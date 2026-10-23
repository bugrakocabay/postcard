//! Integration tests for `http::send` against a local axum server.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::Request as AxRequest;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
use postcard_lib::cookies;
use postcard_lib::http::{self, BodyPayload, HttpError, HttpResponse};
use postcard_lib::model::{Body, FormField, FormFieldKind, KeyValue, Request};
use reqwest_cookie_store::CookieStoreMutex;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

/// Echoes method, uri, headers and body of any request as JSON.
async fn echo(req: AxRequest) -> axum::Json<Value> {
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();
    let mut headers = serde_json::Map::new();
    for (k, v) in parts.headers.iter() {
        headers.insert(k.as_str().to_string(), Value::String(String::from_utf8_lossy(v.as_bytes()).into_owned()));
    }
    axum::Json(json!({
        "method": parts.method.as_str(),
        "uri": parts.uri.to_string(),
        "headers": headers,
        "body": String::from_utf8_lossy(&bytes),
        "bodyLen": bytes.len(),
    }))
}

/// Returns the raw request body with an octet-stream content type.
async fn raw_echo(body: axum::body::Bytes) -> Response {
    ([(header::CONTENT_TYPE, "application/octet-stream")], body).into_response()
}

async fn binary() -> Response {
    ([(header::CONTENT_TYPE, "application/octet-stream")], vec![0xffu8, 0xfe, 0x00, 0x80, 0x81]).into_response()
}

async fn redirect() -> Response {
    (StatusCode::FOUND, [(header::LOCATION, "/get")]).into_response()
}

async fn slow() -> &'static str {
    tokio::time::sleep(Duration::from_secs(3)).await;
    "late"
}

async fn set_cookie() -> Response {
    let mut res = "ok".into_response();
    res.headers_mut().append(header::SET_COOKIE, "sid=abc; Path=/".parse().unwrap());
    res.headers_mut().append(header::SET_COOKIE, "theme=dark; Path=/".parse().unwrap());
    res
}

async fn echo_cookie(headers: HeaderMap) -> String {
    headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string()
}

async fn spawn_server() -> SocketAddr {
    let app = Router::new()
        .route("/get", get(echo))
        .route("/echo", any(echo))
        .route("/raw-echo", any(raw_echo))
        .route("/binary", get(binary))
        .route("/redirect", get(redirect))
        .route("/slow", get(slow))
        .route("/set-cookie", get(set_cookie))
        .route("/echo-cookie", get(echo_cookie));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

fn req(method: &str, addr: SocketAddr, path: &str) -> Request {
    Request { method: method.into(), url: format!("http://{addr}{path}"), ..Default::default() }
}

async fn send(r: &Request) -> Result<HttpResponse, HttpError> {
    http::send(r, None, CancellationToken::new()).await
}

fn json_of(res: &HttpResponse) -> Value {
    serde_json::from_slice(&res.body).expect("response body is JSON")
}

fn header_of<'a>(res: &'a HttpResponse, name: &str) -> Option<&'a str> {
    res.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

#[tokio::test]
async fn get_echo() {
    let addr = spawn_server().await;
    let mut r = req("GET", addr, "/get?a=1&b=two");
    r.headers = vec![KeyValue::new("X-Test", "yes"), KeyValue::new("Dup", "1")];
    let res = send(&r).await.unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.status_text, "OK");
    assert!(res.final_url.ends_with("/get?a=1&b=two"), "{}", res.final_url);
    assert_eq!(header_of(&res, "content-type"), Some("application/json"));
    let j = json_of(&res);
    assert_eq!(j["method"], "GET");
    assert_eq!(j["uri"], "/get?a=1&b=two");
    assert_eq!(j["headers"]["x-test"], "yes");
}

#[tokio::test]
async fn disabled_and_empty_headers_are_skipped() {
    let addr = spawn_server().await;
    let mut r = req("GET", addr, "/get");
    r.headers = vec![
        KeyValue { key: "X-Off".into(), value: "1".into(), enabled: false },
        KeyValue::new("", "orphan"),
        KeyValue::new("X-On", "1"),
    ];
    let j = json_of(&send(&r).await.unwrap());
    assert!(j["headers"].get("x-off").is_none());
    assert_eq!(j["headers"]["x-on"], "1");
}

#[tokio::test]
async fn invalid_header_name_is_an_error() {
    let addr = spawn_server().await;
    let mut r = req("GET", addr, "/get");
    r.headers = vec![KeyValue::new("Bad Header", "x")];
    match send(&r).await {
        Err(HttpError::Other(m)) => assert!(m.contains("Bad Header"), "{m}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn url_without_scheme_defaults_to_http() {
    let addr = spawn_server().await;
    let mut r = req("GET", addr, "/get");
    r.url = format!("{addr}/get");
    assert_eq!(send(&r).await.unwrap().status, 200);
}

#[tokio::test]
async fn custom_method_is_sent() {
    let addr = spawn_server().await;
    let r = req("PROPFIND", addr, "/echo");
    assert_eq!(json_of(&send(&r).await.unwrap())["method"], "PROPFIND");
}

#[tokio::test]
async fn json_body_sets_content_type() {
    let addr = spawn_server().await;
    let mut r = req("POST", addr, "/echo");
    r.body = Body::Json { text: r#"{"a":1}"#.into() };
    let j = json_of(&send(&r).await.unwrap());
    assert_eq!(j["method"], "POST");
    assert_eq!(j["body"], r#"{"a":1}"#);
    assert_eq!(j["headers"]["content-type"], "application/json");
}

#[tokio::test]
async fn explicit_content_type_wins_over_json_default() {
    let addr = spawn_server().await;
    let mut r = req("POST", addr, "/echo");
    r.body = Body::Json { text: "{}".into() };
    r.headers = vec![KeyValue::new("content-type", "application/vnd.api+json")];
    let j = json_of(&send(&r).await.unwrap());
    assert_eq!(j["headers"]["content-type"], "application/vnd.api+json");
}

#[tokio::test]
async fn raw_body_uses_its_content_type() {
    let addr = spawn_server().await;
    let mut r = req("POST", addr, "/echo");
    r.body = Body::Raw { text: "hello".into(), content_type: "text/plain".into() };
    let j = json_of(&send(&r).await.unwrap());
    assert_eq!(j["body"], "hello");
    assert_eq!(j["headers"]["content-type"], "text/plain");
}

#[tokio::test]
async fn urlencoded_body() {
    let addr = spawn_server().await;
    let mut r = req("POST", addr, "/echo");
    r.body = Body::UrlEncoded {
        fields: vec![
            KeyValue::new("a", "1 2"),
            KeyValue::new("b", "x&y"),
            KeyValue { key: "off".into(), value: "z".into(), enabled: false },
        ],
    };
    let j = json_of(&send(&r).await.unwrap());
    assert_eq!(j["body"], "a=1+2&b=x%26y");
    assert_eq!(j["headers"]["content-type"], "application/x-www-form-urlencoded");
}

#[tokio::test]
async fn multipart_with_file() {
    let addr = spawn_server().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "file-content-123").unwrap();
    let mut r = req("POST", addr, "/echo");
    r.body = Body::Multipart {
        fields: vec![
            FormField { key: "title".into(), value: "hello".into(), enabled: true, kind: FormFieldKind::Text },
            FormField {
                key: "upload".into(),
                value: path.to_string_lossy().into_owned(),
                enabled: true,
                kind: FormFieldKind::File,
            },
        ],
    };
    // A stale explicit content-type must not break the multipart boundary.
    r.headers = vec![KeyValue::new("Content-Type", "text/plain")];
    let j = json_of(&send(&r).await.unwrap());
    let ct = j["headers"]["content-type"].as_str().unwrap();
    assert!(ct.starts_with("multipart/form-data; boundary="), "{ct}");
    let body = j["body"].as_str().unwrap();
    assert!(body.contains("name=\"title\""), "{body}");
    assert!(body.contains("hello"), "{body}");
    assert!(body.contains("name=\"upload\""), "{body}");
    assert!(body.contains("filename=\"notes.txt\""), "{body}");
    assert!(body.contains("file-content-123"), "{body}");
}

#[tokio::test]
async fn multipart_missing_file_is_io_error() {
    let addr = spawn_server().await;
    let mut r = req("POST", addr, "/echo");
    r.body = Body::Multipart {
        fields: vec![FormField {
            key: "f".into(),
            value: "/definitely/not/here.bin".into(),
            enabled: true,
            kind: FormFieldKind::File,
        }],
    };
    match send(&r).await {
        Err(HttpError::Io(m)) => assert!(m.contains("cannot read file /definitely/not/here.bin"), "{m}"),
        other => panic!("unexpected {other:?}"),
    }
}

#[tokio::test]
async fn binary_file_body() {
    let addr = spawn_server().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("blob.bin");
    let bytes: Vec<u8> = (0..=255u8).collect();
    std::fs::write(&path, &bytes).unwrap();
    let mut r = req("POST", addr, "/raw-echo");
    r.body = Body::Binary { path: path.to_string_lossy().into_owned() };
    let res = send(&r).await.unwrap();
    assert_eq!(res.body.as_ref(), bytes.as_slice());
}

#[tokio::test]
async fn redirect_followed() {
    let addr = spawn_server().await;
    let res = send(&req("GET", addr, "/redirect")).await.unwrap();
    assert_eq!(res.status, 200);
    assert!(res.final_url.ends_with("/get"), "{}", res.final_url);
}

#[tokio::test]
async fn redirect_not_followed() {
    let addr = spawn_server().await;
    let mut r = req("GET", addr, "/redirect");
    r.settings.follow_redirects = false;
    let res = send(&r).await.unwrap();
    assert_eq!(res.status, 302);
    assert_eq!(header_of(&res, "location"), Some("/get"));
    assert!(res.final_url.ends_with("/redirect"), "{}", res.final_url);
}

#[tokio::test]
async fn timeout_reports_ms() {
    let addr = spawn_server().await;
    let mut r = req("GET", addr, "/slow");
    r.settings.timeout_ms = 200;
    let started = Instant::now();
    let err = send(&r).await.unwrap_err();
    assert!(matches!(err, HttpError::Timeout(200)), "{err:?}");
    assert!(err.to_string().contains("200 ms"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn cancel_aborts_request() {
    let addr = spawn_server().await;
    let r = req("GET", addr, "/slow");
    let token = CancellationToken::new();
    let t2 = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        t2.cancel();
    });
    let started = Instant::now();
    let err = http::send(&r, None, token).await.unwrap_err();
    assert!(matches!(err, HttpError::Cancelled), "{err:?}");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn cookie_set_then_sent() {
    let addr = spawn_server().await;
    let jar = Arc::new(CookieStoreMutex::default());
    let first = http::send(&req("GET", addr, "/set-cookie"), Some(jar.clone()), CancellationToken::new())
        .await
        .unwrap();
    // Both Set-Cookie headers are preserved, in order.
    let set: Vec<&str> = first
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(set, vec!["sid=abc; Path=/", "theme=dark; Path=/"]);

    let second = http::send(&req("GET", addr, "/echo-cookie"), Some(jar.clone()), CancellationToken::new())
        .await
        .unwrap();
    let sent = String::from_utf8_lossy(&second.body).into_owned();
    assert!(sent.contains("sid=abc"), "{sent}");
    assert!(sent.contains("theme=dark"), "{sent}");
    assert_eq!(cookies::list(&jar).len(), 2);

    // Without a jar nothing is stored or sent.
    let third = send(&req("GET", addr, "/echo-cookie")).await.unwrap();
    assert!(third.body.is_empty());
}

#[tokio::test]
async fn connection_refused_message() {
    // Bind then drop to get a port nobody listens on.
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let r = Request { url: format!("http://127.0.0.1:{port}/"), ..Default::default() };
    let err = send(&r).await.unwrap_err();
    assert!(matches!(err, HttpError::Connect(_)), "{err:?}");
    let msg = err.to_string();
    assert!(msg.starts_with("Could not connect to host"), "{msg}");
}

#[tokio::test]
async fn binary_body_is_not_sent_as_text() {
    let addr = spawn_server().await;
    let res = send(&req("GET", addr, "/binary")).await.unwrap();
    assert_eq!(res.body.len(), 5);
    let ct = header_of(&res, "content-type");
    assert_eq!(http::body_payload(ct, &res.body), BodyPayload::Binary);
    let wire = serde_json::to_string(&http::body_payload(ct, &res.body)).unwrap();
    assert_eq!(wire, r#"{"type":"binary"}"#);
}

#[tokio::test]
async fn time_ms_is_measured() {
    let addr = spawn_server().await;
    let res = send(&req("GET", addr, "/get")).await.unwrap();
    assert!(res.time_ms < 2_000);
}

#[tokio::test]
async fn tls_to_plain_http_server_is_a_tls_error() {
    let addr = spawn_server().await;
    let r = Request { url: format!("https://{addr}/get"), ..Default::default() };
    let err = send(&r).await.unwrap_err();
    assert!(matches!(err, HttpError::Tls(_)), "{err:?}");
}

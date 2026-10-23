//! HTTP client: sends a fully resolved [`Request`] and classifies the response body.

use std::error::Error as _;
use std::path::Path;
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use bytes::Bytes;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use reqwest::multipart::{Form, Part};
use reqwest_cookie_store::CookieStoreMutex;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::model::{ApiKeyLocation, Auth, Body, FormFieldKind, KeyValue, Request};

/// Responses larger than this are never decoded or embedded (50 MiB).
pub const TOO_LARGE: usize = 50 * 1024 * 1024;

/// A fully received HTTP response.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    /// Response headers in wire order; duplicates (e.g. `Set-Cookie`) are kept.
    pub headers: Vec<(String, String)>,
    /// Body after transparent decompression.
    pub body: Bytes,
    /// Milliseconds from just before sending until the body was fully read.
    pub time_ms: u64,
    /// URL after following redirects.
    pub final_url: String,
}

/// Why a request failed. `Display` is user-facing text.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HttpError {
    /// Carries the configured timeout in milliseconds.
    #[error("Request timed out after {0} ms")]
    Timeout(u64),
    #[error("{0}")]
    Connect(String),
    #[error("{0}")]
    Dns(String),
    #[error("{0}")]
    Tls(String),
    #[error("Request cancelled")]
    Cancelled,
    #[error("{0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}

/// Installs the process-wide rustls crypto provider (needed by reqwest's `rustls-no-provider`).
/// Safe to call repeatedly.
pub fn ensure_crypto() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // Err means a provider is already installed, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// Sends `req` (variables resolved, auth already applied via [`apply_auth`]).
///
/// Cookies are read from and stored into `jar` when given. Dropping the future or
/// cancelling `cancel` aborts the request with [`HttpError::Cancelled`].
pub async fn send(
    req: &Request,
    jar: Option<Arc<CookieStoreMutex>>,
    cancel: CancellationToken,
) -> Result<HttpResponse, HttpError> {
    ensure_crypto();
    if cancel.is_cancelled() {
        return Err(HttpError::Cancelled);
    }
    let timeout_ms = req.settings.timeout_ms;

    let method = parse_method(&req.method)?;
    let url = parse_url(&req.url)?;
    let client = build_client(req, jar)?;
    let builder = apply_body(client.request(method, url), req).await?;

    let started = Instant::now();
    let work = async {
        let resp = builder.send().await.map_err(|e| map_error(&e, timeout_ms))?;
        let status = resp.status();
        let final_url = resp.url().to_string();
        let headers = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
            .collect();
        let body = resp.bytes().await.map_err(|e| map_error(&e, timeout_ms))?;
        Ok(HttpResponse {
            status: status.as_u16(),
            status_text: status.canonical_reason().unwrap_or("").to_string(),
            headers,
            body,
            time_ms: started.elapsed().as_millis() as u64,
            final_url,
        })
    };
    tokio::select! {
        _ = cancel.cancelled() => Err(HttpError::Cancelled),
        r = work => r,
    }
}

fn parse_method(raw: &str) -> Result<reqwest::Method, HttpError> {
    let m = raw.trim().to_ascii_uppercase();
    let m = if m.is_empty() { "GET".to_string() } else { m };
    reqwest::Method::from_bytes(m.as_bytes()).map_err(|_| HttpError::Other(format!("Invalid HTTP method: {raw}")))
}

fn parse_url(raw: &str) -> Result<reqwest::Url, HttpError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(HttpError::Other("URL is empty".into()));
    }
    let full = if raw.contains("://") { raw.to_string() } else { format!("http://{raw}") };
    reqwest::Url::parse(&full).map_err(|e| HttpError::Other(format!("Invalid URL \"{raw}\": {e}")))
}

fn build_client(req: &Request, jar: Option<Arc<CookieStoreMutex>>) -> Result<reqwest::Client, HttpError> {
    let s = &req.settings;
    let mut b = reqwest::Client::builder()
        .user_agent(concat!("Postcard/", env!("CARGO_PKG_VERSION")))
        .redirect(if s.follow_redirects {
            reqwest::redirect::Policy::limited(10)
        } else {
            reqwest::redirect::Policy::none()
        })
        .tls_danger_accept_invalid_certs(!s.verify_ssl)
        .tls_danger_accept_invalid_hostnames(!s.verify_ssl);
    if s.timeout_ms > 0 {
        b = b.timeout(Duration::from_millis(s.timeout_ms));
    }
    if let Some(jar) = jar {
        b = b.cookie_provider(jar);
    }
    b.build().map_err(|e| HttpError::Other(format!("Could not create HTTP client: {e}")))
}

/// Enabled rows with a non-blank key.
fn enabled_kv(headers: &[KeyValue]) -> Vec<&KeyValue> {
    headers.iter().filter(|h| h.enabled && !h.key.trim().is_empty()).collect()
}

/// Adds headers and body to the builder. Header values and file reads are validated here.
async fn apply_body(mut builder: reqwest::RequestBuilder, req: &Request) -> Result<reqwest::RequestBuilder, HttpError> {
    let is_multipart = matches!(req.body, Body::Multipart { .. });
    let mut map = HeaderMap::new();
    for h in enabled_kv(&req.headers) {
        let name = h.key.trim();
        // The multipart boundary is generated by reqwest; a user header would break it.
        if is_multipart && name.eq_ignore_ascii_case("content-type") {
            continue;
        }
        let hn = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| HttpError::Other(format!("Invalid header name: \"{name}\"")))?;
        let hv = HeaderValue::from_str(&h.value)
            .map_err(|_| HttpError::Other(format!("Invalid value for header \"{name}\"")))?;
        map.append(hn, hv);
    }
    let user_ct = map.contains_key(CONTENT_TYPE);
    let default_ct = |map: &mut HeaderMap, ct: &str| {
        if !user_ct {
            if let Ok(v) = HeaderValue::from_str(ct) {
                map.insert(CONTENT_TYPE, v);
            }
        }
    };

    match &req.body {
        Body::None => {}
        Body::Json { text } => {
            default_ct(&mut map, "application/json");
            builder = builder.body(text.clone());
        }
        Body::Raw { text, content_type } => {
            if !content_type.trim().is_empty() {
                default_ct(&mut map, content_type.trim());
            }
            builder = builder.body(text.clone());
        }
        Body::UrlEncoded { fields } => {
            let mut ser = url::form_urlencoded::Serializer::new(String::new());
            for f in enabled_kv(fields) {
                ser.append_pair(&f.key, &f.value);
            }
            default_ct(&mut map, "application/x-www-form-urlencoded");
            builder = builder.body(ser.finish());
        }
        Body::Multipart { fields } => {
            let mut form = Form::new();
            for f in fields.iter().filter(|f| f.enabled && !f.key.is_empty()) {
                let part = match f.kind {
                    FormFieldKind::Text => Part::text(f.value.clone()),
                    FormFieldKind::File => {
                        let bytes = read_file(&f.value).await?;
                        let name = Path::new(&f.value)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "file".into());
                        Part::bytes(bytes).file_name(name).mime_str(guess_mime(&f.value)).map_err(|e| {
                            HttpError::Other(format!("Invalid MIME type for {}: {e}", f.value))
                        })?
                    }
                };
                form = form.part(f.key.clone(), part);
            }
            builder = builder.multipart(form);
        }
        Body::Binary { path } => {
            if !path.trim().is_empty() {
                builder = builder.body(read_file(path).await?);
            }
        }
    }
    Ok(builder.headers(map))
}

async fn read_file(path: &str) -> Result<Vec<u8>, HttpError> {
    tokio::fs::read(path).await.map_err(|e| HttpError::Io(format!("cannot read file {path}: {e}")))
}

/// Minimal extension -> MIME table for multipart file parts.
fn guess_mime(path: &str) -> &'static str {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "txt" | "log" | "md" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "csv" => "text/csv",
        "js" => "text/javascript",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

/// Text of an error's source chain without reqwest's generic wrapper layers, for display and classification.
fn chain_text(e: &reqwest::Error) -> String {
    let mut parts = vec![e.to_string()];
    let mut cur = e.source();
    while let Some(c) = cur {
        parts.push(c.to_string());
        cur = c.source();
    }
    let useful: Vec<&str> = parts
        .iter()
        .map(String::as_str)
        .filter(|p| !p.starts_with("error sending request") && !p.starts_with("client error"))
        .collect();
    if useful.is_empty() { parts.join(": ") } else { useful.join(": ") }
}

fn map_error(e: &reqwest::Error, timeout_ms: u64) -> HttpError {
    if e.is_timeout() {
        return HttpError::Timeout(timeout_ms);
    }
    let text = chain_text(e);
    let low = text.to_ascii_lowercase();
    if e.is_connect() {
        if ["certificate", "tls", "ssl", "handshake", "invalid peer", "corrupt message"].iter().any(|k| low.contains(k)) {
            return HttpError::Tls(format!("TLS error: {text}"));
        }
        if low.contains("dns") || low.contains("resolve") || low.contains("lookup address") {
            return HttpError::Dns(format!("Could not resolve host: {text}"));
        }
        return HttpError::Connect(format!("Could not connect to host: {text}"));
    }
    if e.is_redirect() {
        return HttpError::Other(format!("Too many redirects: {text}"));
    }
    if e.is_body() || e.is_decode() {
        return HttpError::Io(format!("Error reading response: {text}"));
    }
    HttpError::Other(text)
}

/// Sets `name` to `value`, replacing every existing header of that name (case-insensitive).
fn upsert_header(headers: &mut Vec<KeyValue>, name: &str, value: String) {
    match headers.iter().position(|h| h.key.trim().eq_ignore_ascii_case(name)) {
        Some(first) => {
            headers[first] = KeyValue { key: headers[first].key.clone(), value, enabled: true };
            let mut i = first + 1;
            while i < headers.len() {
                if headers[i].key.trim().eq_ignore_ascii_case(name) {
                    headers.remove(i);
                } else {
                    i += 1;
                }
            }
        }
        None => headers.push(KeyValue::new(name, value)),
    }
}

/// Appends `key=value` (form-encoded) to the URL's query, keeping the rest of the URL verbatim.
fn append_query(url: &str, key: &str, value: &str) -> String {
    let (base, fragment) = match url.find('#') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    };
    let pair = url::form_urlencoded::Serializer::new(String::new()).append_pair(key, value).finish();
    let sep = if !base.contains('?') {
        "?"
    } else if base.ends_with('?') || base.ends_with('&') {
        ""
    } else {
        "&"
    };
    format!("{base}{sep}{pair}{fragment}")
}

/// Applies an already-resolved auth to the request (headers or query). `None` and `Inherit` do nothing.
pub fn apply_auth(req: &mut Request, auth: &Auth) {
    match auth {
        Auth::None | Auth::Inherit => {}
        Auth::Bearer { token } => {
            if !token.is_empty() {
                upsert_header(&mut req.headers, "Authorization", format!("Bearer {token}"));
            }
        }
        Auth::Basic { username, password } => {
            let enc = BASE64.encode(format!("{username}:{password}"));
            upsert_header(&mut req.headers, "Authorization", format!("Basic {enc}"));
        }
        Auth::ApiKey { key, value, location } => {
            if key.is_empty() {
                return;
            }
            match location {
                ApiKeyLocation::Header => upsert_header(&mut req.headers, key, value.clone()),
                ApiKeyLocation::Query => req.url = append_query(&req.url, key, value),
            }
        }
    }
}

/// How a response body is delivered to the UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BodyPayload {
    Text { text: String },
    Image { base64: String, mime: String },
    Binary,
    TooLarge,
}

fn is_textual(mime: &str) -> bool {
    mime.starts_with("text/")
        || ["json", "xml", "javascript", "x-www-form-urlencoded"].iter().any(|k| mime.contains(k))
}

/// Chooses how to present `bytes`: more than [`TOO_LARGE`] -> `TooLarge`; `image/*` -> `Image`;
/// valid UTF-8 or a textual content type -> `Text` (lossy decode; charset parameters are ignored);
/// anything else -> `Binary`.
pub fn body_payload(content_type: Option<&str>, bytes: &[u8]) -> BodyPayload {
    if bytes.len() > TOO_LARGE {
        return BodyPayload::TooLarge;
    }
    if bytes.is_empty() {
        return BodyPayload::Text { text: String::new() };
    }
    let mime = content_type.unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if mime.starts_with("image/") {
        return BodyPayload::Image { base64: BASE64.encode(bytes), mime };
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => BodyPayload::Text { text: s.to_string() },
        Err(_) if is_textual(&mime) => BodyPayload::Text { text: String::from_utf8_lossy(bytes).into_owned() },
        Err(_) => BodyPayload::Binary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header<'a>(r: &'a Request, name: &str) -> Option<&'a str> {
        r.headers.iter().find(|h| h.key.eq_ignore_ascii_case(name)).map(|h| h.value.as_str())
    }

    #[test]
    fn body_payload_thresholds() {
        assert_eq!(body_payload(None, b"hi"), BodyPayload::Text { text: "hi".into() });
        assert_eq!(body_payload(Some("application/json; charset=utf-8"), b"{}"), BodyPayload::Text { text: "{}".into() });
        assert_eq!(body_payload(Some("application/octet-stream"), &[0xff, 0xfe]), BodyPayload::Binary);
        assert_eq!(body_payload(None, &[0xff, 0xfe]), BodyPayload::Binary);
        // Textual type with invalid UTF-8 (e.g. latin-1) is decoded lossily.
        assert_eq!(
            body_payload(Some("text/html; charset=iso-8859-1"), &[b'a', 0xe9]),
            BodyPayload::Text { text: "a\u{fffd}".into() }
        );
        match body_payload(Some("IMAGE/PNG"), &[1, 2, 3]) {
            BodyPayload::Image { base64, mime } => {
                assert_eq!(mime, "image/png");
                assert_eq!(base64, "AQID");
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(body_payload(Some("image/svg+xml"), b"<svg/>"), BodyPayload::Image { .. }));
        assert_eq!(body_payload(Some("image/png"), &[]), BodyPayload::Text { text: String::new() });
        let exactly = vec![b'a'; TOO_LARGE];
        assert!(matches!(body_payload(None, &exactly), BodyPayload::Text { .. }));
        let over = vec![b'a'; TOO_LARGE + 1];
        assert_eq!(body_payload(None, &over), BodyPayload::TooLarge);
        assert_eq!(body_payload(Some("image/png"), &over), BodyPayload::TooLarge);
    }

    #[test]
    fn body_payload_wire_format() {
        let t = serde_json::to_string(&BodyPayload::Text { text: "x".into() }).unwrap();
        assert_eq!(t, r#"{"type":"text","text":"x"}"#);
        let i = serde_json::to_string(&BodyPayload::Image { base64: "AA".into(), mime: "image/png".into() }).unwrap();
        assert_eq!(i, r#"{"type":"image","base64":"AA","mime":"image/png"}"#);
        assert_eq!(serde_json::to_string(&BodyPayload::TooLarge).unwrap(), r#"{"type":"tooLarge"}"#);
    }

    #[test]
    fn bearer_upserts_case_insensitively() {
        let mut r = Request { headers: vec![KeyValue::new("authorization", "old")], ..Default::default() };
        apply_auth(&mut r, &Auth::Bearer { token: "abc".into() });
        assert_eq!(r.headers.len(), 1);
        assert_eq!(header(&r, "Authorization"), Some("Bearer abc"));
    }

    #[test]
    fn empty_bearer_adds_nothing() {
        let mut r = Request::default();
        apply_auth(&mut r, &Auth::Bearer { token: String::new() });
        assert!(r.headers.is_empty());
    }

    #[test]
    fn basic_is_base64() {
        let mut r = Request::default();
        apply_auth(&mut r, &Auth::Basic { username: "user".into(), password: "pass".into() });
        assert_eq!(header(&r, "authorization"), Some("Basic dXNlcjpwYXNz"));
    }

    #[test]
    fn api_key_header_and_query() {
        let mut r = Request { url: "http://h/p".into(), ..Default::default() };
        apply_auth(&mut r, &Auth::ApiKey { key: "X-Key".into(), value: "v".into(), location: ApiKeyLocation::Header });
        assert_eq!(header(&r, "x-key"), Some("v"));

        apply_auth(&mut r, &Auth::ApiKey { key: "api key".into(), value: "a&b".into(), location: ApiKeyLocation::Query });
        assert_eq!(r.url, "http://h/p?api+key=a%26b");
        apply_auth(&mut r, &Auth::ApiKey { key: "k".into(), value: "2".into(), location: ApiKeyLocation::Query });
        assert_eq!(r.url, "http://h/p?api+key=a%26b&k=2");
    }

    #[test]
    fn query_append_keeps_fragment_and_trailing_separators() {
        assert_eq!(append_query("http://h/p#frag", "k", "v"), "http://h/p?k=v#frag");
        assert_eq!(append_query("http://h/p?", "k", "v"), "http://h/p?k=v");
        assert_eq!(append_query("http://h/p?a=1&", "k", "v"), "http://h/p?a=1&k=v");
        assert_eq!(append_query("localhost:3000", "k", "v"), "localhost:3000?k=v");
    }

    #[test]
    fn none_and_inherit_do_nothing() {
        let mut r = Request::default();
        apply_auth(&mut r, &Auth::None);
        apply_auth(&mut r, &Auth::Inherit);
        assert!(r.headers.is_empty());
        assert!(r.url.is_empty());
    }

    #[test]
    fn mime_guess() {
        assert_eq!(guess_mime("a/b.PNG"), "image/png");
        assert_eq!(guess_mime("noext"), "application/octet-stream");
    }

    #[test]
    fn url_and_method_parsing() {
        assert_eq!(parse_url("localhost:3000/x").unwrap().as_str(), "http://localhost:3000/x");
        assert!(parse_url("  ").is_err());
        assert_eq!(parse_method("get").unwrap(), reqwest::Method::GET);
        assert_eq!(parse_method("").unwrap(), reqwest::Method::GET);
        assert!(parse_method("BAD METHOD").is_err());
    }
}

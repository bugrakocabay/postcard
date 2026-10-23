//! End-to-end tests of the send pipeline (scripts → variables → auth → HTTP → history).

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::{Request as AxRequest, State};
use axum::routing::{any, post};
use postcard_lib::model::{Auth, Body, Environment, EnvironmentsFile, KeyValue, Request, Scripts, Variable};
use postcard_lib::pipeline::{Ancestor, PipelineCtx, SendInput, lock, run};
use postcard_lib::script::LogLevel;
use postcard_lib::storage::Store;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
struct Hits(Arc<AtomicUsize>);

async fn echo(State(hits): State<Hits>, req: AxRequest) -> axum::Json<Value> {
    hits.0.fetch_add(1, Ordering::SeqCst);
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap_or_default();
    let headers: serde_json::Map<String, Value> = parts
        .headers
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), Value::String(String::from_utf8_lossy(v.as_bytes()).into())))
        .collect();
    axum::Json(json!({ "method": parts.method.as_str(), "uri": parts.uri.to_string(), "headers": headers, "body": String::from_utf8_lossy(&bytes) }))
}

async fn login() -> axum::Json<Value> {
    axum::Json(json!({ "access_token": "tok-123" }))
}

async fn server() -> (SocketAddr, Hits) {
    let hits = Hits::default();
    let app = Router::new().route("/echo", any(echo)).route("/login", post(login)).with_state(hits.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (addr, hits)
}

fn ctx_with_env(dir: &std::path::Path, vars: Vec<(&str, &str)>) -> PipelineCtx {
    let store = Store::new(dir.to_path_buf());
    let env = Environment {
        id: "e1".into(),
        name: "local".into(),
        variables: vars
            .into_iter()
            .map(|(k, v)| Variable { key: k.into(), value: v.into(), enabled: true, secret: false })
            .collect(),
    };
    store
        .save_environments(&EnvironmentsFile { environments: vec![env], active_id: Some("e1".into()), ..Default::default() })
        .unwrap();
    PipelineCtx::load(store).0
}

fn input(req: Request, ancestors: Vec<Ancestor>) -> SendInput {
    SendInput { send_id: "s".into(), request: req, ancestors }
}

fn body_json(res: &postcard_lib::pipeline::SendResult) -> Value {
    match res.body.as_ref().unwrap() {
        postcard_lib::http::BodyPayload::Text { text } => serde_json::from_str(text).unwrap(),
        other => panic!("unexpected body {other:?}"),
    }
}

#[tokio::test]
async fn login_script_sets_token_used_by_next_request() {
    let (addr, _) = server().await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_with_env(dir.path(), vec![("baseUrl", &format!("http://{addr}"))]);

    let login = Request {
        method: "POST".into(),
        url: "{{baseUrl}}/login".into(),
        scripts: Scripts { pre: String::new(), post: "pm.environment.set('token', pm.response.json().access_token)".into() },
        ..Default::default()
    };
    let r1 = run(input(login, vec![]), &ctx, CancellationToken::new()).await;
    assert_eq!(r1.error, None, "{:?}", r1.logs);
    let envs = r1.environments.expect("env changed");
    assert!(envs.environments[0].variables.iter().any(|v| v.key == "token" && v.value == "tok-123"));

    // Saved to disk too.
    let on_disk: EnvironmentsFile =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("environments.json")).unwrap()).unwrap();
    assert!(on_disk.environments[0].variables.iter().any(|v| v.key == "token"));

    let next = Request {
        url: "{{baseUrl}}/echo".into(),
        headers: vec![KeyValue::new("Authorization", "Bearer {{token}}")],
        ..Default::default()
    };
    let r2 = run(input(next, vec![]), &ctx, CancellationToken::new()).await;
    assert_eq!(body_json(&r2)["headers"]["authorization"], "Bearer tok-123");
    assert_eq!(r2.environments, None);
}

#[tokio::test]
async fn pre_script_error_does_not_send() {
    let (addr, hits) = server().await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_with_env(dir.path(), vec![]);
    let req = Request {
        url: format!("http://{addr}/echo"),
        scripts: Scripts { pre: "throw new Error('boom')".into(), post: String::new() },
        ..Default::default()
    };
    let r = run(input(req, vec![]), &ctx, CancellationToken::new()).await;
    assert!(r.error.as_deref().unwrap().contains("boom"), "{:?}", r.error);
    assert!(r.response.is_none());
    assert_eq!(hits.0.load(Ordering::SeqCst), 0);
    assert_eq!(lock(&ctx.history).len(), 1, "failed sends are still in history");
}

#[tokio::test]
async fn unresolved_variable_warns() {
    let (addr, _) = server().await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_with_env(dir.path(), vec![]);
    let req = Request { url: format!("http://{addr}/echo?x={{{{missing}}}}"), ..Default::default() };
    let r = run(input(req, vec![]), &ctx, CancellationToken::new()).await;
    assert!(r.logs.iter().any(|l| l.level == LogLevel::Warn && l.message.contains("missing")), "{:?}", r.logs);
}

#[tokio::test]
async fn auth_inherited_from_folder_and_scripts_run_outermost_first() {
    let (addr, _) = server().await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_with_env(dir.path(), vec![("t", "secret")]);
    let ancestors = vec![
        Ancestor {
            auth: Auth::None,
            scripts: Scripts { pre: "pm.variables.set('order', 'c')".into(), post: String::new() },
        },
        Ancestor {
            auth: Auth::Bearer { token: "{{t}}".into() },
            scripts: Scripts { pre: "pm.variables.set('order', pm.variables.get('order') + 'f')".into(), post: String::new() },
        },
    ];
    let req = Request {
        method: "POST".into(),
        url: format!("http://{addr}/echo"),
        body: Body::Json { text: "{\"order\":\"{{order}}\"}".into() },
        scripts: Scripts { pre: "pm.variables.set('order', pm.variables.get('order') + 'r')".into(), post: String::new() },
        ..Default::default()
    };
    let r = run(input(req, ancestors), &ctx, CancellationToken::new()).await;
    let b = body_json(&r);
    assert_eq!(b["headers"]["authorization"], "Bearer secret");
    assert_eq!(b["body"], "{\"order\":\"cfr\"}");
}

#[tokio::test]
async fn pre_script_can_modify_request() {
    let (addr, _) = server().await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_with_env(dir.path(), vec![]);
    let req = Request {
        url: format!("http://{addr}/echo"),
        scripts: Scripts { pre: "pm.request.headers.upsert({key: 'X-Trace', value: 'abc'})".into(), post: String::new() },
        ..Default::default()
    };
    let r = run(input(req, vec![]), &ctx, CancellationToken::new()).await;
    assert_eq!(body_json(&r)["headers"]["x-trace"], "abc");
}

#[tokio::test]
async fn history_is_capped_and_saved() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = ctx_with_env(dir.path(), vec![]);
    {
        let mut h = lock(&ctx.history);
        for _ in 0..500 {
            h.push(Default::default());
        }
    }
    // Connection refused: still recorded.
    let req = Request { url: "http://127.0.0.1:1/x".into(), ..Default::default() };
    let r = run(input(req, vec![]), &ctx, CancellationToken::new()).await;
    assert!(r.error.is_some());
    assert_eq!(lock(&ctx.history).len(), 500);
    assert_eq!(lock(&ctx.history)[0].id, r.history_entry.id);
    assert!(dir.path().join("history.json").exists());
}

#[tokio::test]
async fn env_set_without_active_env_is_runtime_only() {
    let (addr, _) = server().await;
    let dir = tempfile::tempdir().unwrap();
    let ctx = PipelineCtx::load(Store::new(dir.path().to_path_buf())).0;
    let req = Request {
        url: format!("http://{addr}/echo?v={{{{x}}}}"),
        scripts: Scripts { pre: "pm.environment.set('x', '42')".into(), post: String::new() },
        ..Default::default()
    };
    let r = run(input(req, vec![]), &ctx, CancellationToken::new()).await;
    assert_eq!(body_json(&r)["uri"], "/echo?v=42");
    assert!(r.logs.iter().any(|l| l.level == LogLevel::Warn && l.message.contains("no active environment")));
    assert_eq!(r.environments, None);
}

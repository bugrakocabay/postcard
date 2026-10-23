//! The full send flow: pre-request scripts → variables → auth → HTTP → post-response scripts → history.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use reqwest_cookie_store::CookieStoreMutex;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::cookies;
use crate::http::{self, BodyPayload};
use crate::model::{
    Auth, Body, EnvironmentsFile, HistoryEntry, KeyValue, Request, Scripts, Settings, Variable, Workspace,
};
use crate::script::{self, LogLevel, LogLine, ScriptRequest, ScriptResponse, ScriptState};
use crate::storage::Store;
use crate::vars::{self, VarScope};

pub const MAX_HISTORY: usize = 500;
const KEEP_BODIES: usize = 20;

/// Auth and scripts of a collection or folder above the request (outermost first).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Ancestor {
    pub auth: Auth,
    pub scripts: Scripts,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SendInput {
    pub send_id: String,
    pub request: Request,
    #[serde(default)]
    pub ancestors: Vec<Ancestor>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ResponseMeta {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub size_bytes: u64,
    pub time_ms: u64,
    pub content_type: Option<String>,
    pub set_cookies: Vec<String>,
    pub final_url: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub response_id: String,
    pub response: Option<ResponseMeta>,
    pub body: Option<BodyPayload>,
    /// Network error, pre-request script error, or (with a response) a post-response script error.
    pub error: Option<String>,
    pub logs: Vec<LogLine>,
    /// Present when scripts changed environment or global variables.
    pub environments: Option<EnvironmentsFile>,
    pub history_entry: HistoryEntry,
}

/// Shared backend state used by the send pipeline and commands.
pub struct PipelineCtx {
    pub store: Store,
    pub envs: Mutex<EnvironmentsFile>,
    pub settings: Mutex<Settings>,
    pub history: Mutex<Vec<HistoryEntry>>,
    pub jar: Arc<CookieStoreMutex>,
    bodies: Mutex<VecDeque<(String, Bytes)>>,
}

impl PipelineCtx {
    /// Loads everything from disk. Returns the context, the workspace for the UI, and load warnings.
    pub fn load(store: Store) -> (Self, Workspace, Vec<String>) {
        let (ws, warnings) = store.load_workspace();
        let jar = Arc::new(cookies::load_jar(&store.cookies_path()));
        let ctx = Self {
            envs: Mutex::new(ws.environments.clone()),
            settings: Mutex::new(ws.settings.clone()),
            history: Mutex::new(ws.history.clone()),
            jar,
            bodies: Mutex::new(VecDeque::new()),
            store,
        };
        (ctx, ws, warnings)
    }

    pub fn body(&self, response_id: &str) -> Option<Bytes> {
        lock(&self.bodies).iter().find(|(id, _)| id == response_id).map(|(_, b)| b.clone())
    }

    fn keep_body(&self, id: String, bytes: Bytes) {
        let mut b = lock(&self.bodies);
        b.push_back((id, bytes));
        while b.len() > KEEP_BODIES {
            b.pop_front();
        }
    }

    pub fn save_cookies(&self) {
        let _ = cookies::save_jar(&self.jar, &self.store.cookies_path());
    }
}

/// Locks a mutex, recovering the data if a previous holder panicked.
pub fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The auth that applies: the request's own, or the nearest ancestor's that is not Inherit.
pub fn effective_auth(req: &Request, ancestors: &[Ancestor]) -> Auth {
    if req.auth != Auth::Inherit {
        return req.auth.clone();
    }
    ancestors
        .iter()
        .rev()
        .map(|a| &a.auth)
        .find(|a| **a != Auth::Inherit)
        .cloned()
        .unwrap_or(Auth::None)
}

fn enabled_map(vars: &[Variable]) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for v in vars.iter().filter(|v| v.enabled && !v.key.is_empty()) {
        m.entry(v.key.clone()).or_insert_with(|| v.value.clone());
    }
    m
}

/// Writes script changes (`after` vs `before`) into a variable list. Returns true if anything changed.
fn apply_var_changes(list: &mut Vec<Variable>, before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> bool {
    let mut changed = false;
    for (k, v) in after {
        if before.get(k) == Some(v) {
            continue;
        }
        changed = true;
        if let Some(existing) = list.iter_mut().find(|x| &x.key == k && x.enabled) {
            existing.value = v.clone();
        } else if let Some(existing) = list.iter_mut().find(|x| &x.key == k) {
            existing.value = v.clone();
            existing.enabled = true;
        } else {
            list.push(Variable { key: k.clone(), value: v.clone(), enabled: true, secret: false });
        }
    }
    for k in before.keys().filter(|k| !after.contains_key(*k)) {
        changed = true;
        list.retain(|x| &x.key != k);
    }
    changed
}

fn script_request(req: &Request) -> ScriptRequest {
    ScriptRequest {
        method: req.method.clone(),
        url: req.url.clone(),
        headers: req.headers.iter().filter(|h| h.enabled).map(|h| (h.key.clone(), h.value.clone())).collect(),
        body: match &req.body {
            Body::Json { text } | Body::Raw { text, .. } => Some(text.clone()),
            _ => None,
        },
    }
}

/// Applies changes a pre-request script made through `pm.request`.
fn apply_script_request(req: &mut Request, before: &ScriptRequest, after: &ScriptRequest) {
    if after.method != before.method {
        req.method = after.method.clone();
    }
    if after.url != before.url {
        req.url = after.url.clone();
    }
    if after.headers != before.headers {
        req.headers = after.headers.iter().map(|(k, v)| KeyValue::new(k.clone(), v.clone())).collect();
    }
    if after.body != before.body {
        let text = after.body.clone().unwrap_or_default();
        match &mut req.body {
            Body::Json { text: t } | Body::Raw { text: t, .. } => *t = text,
            _ => req.body = Body::Raw { text, content_type: String::new() },
        }
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

struct ScriptRun {
    state: ScriptState,
    logs: Vec<LogLine>,
    error: Option<String>,
    /// The runner thread panicked; `state` is the unchanged input and must not be persisted as a diff.
    crashed: bool,
}

/// Runs scripts in order on a blocking thread, stopping at the first error.
async fn run_scripts(
    scripts: Vec<(String, String)>,
    state: ScriptState,
    settings: &Settings,
) -> ScriptRun {
    let shell = settings.shell.clone();
    let fallback = state.clone();
    let job = tokio::task::spawn_blocking(move || {
        let mut state = state;
        let mut logs = Vec::new();
        for (name, source) in &scripts {
            if let Err(e) = script::run_script(source, name, &mut state, &shell, &mut logs) {
                return ScriptRun { state, logs, error: Some(e.message), crashed: false };
            }
        }
        ScriptRun { state, logs, error: None, crashed: false }
    });
    match job.await {
        Ok(run) => run,
        Err(e) => ScriptRun {
            state: fallback,
            logs: vec![],
            error: Some(format!("script runner crashed: {e}")),
            crashed: true,
        },
    }
}

fn script_list(ancestors: &[Ancestor], req: &Request, post: bool) -> Vec<(String, String)> {
    let phase = if post { "post-response" } else { "pre-request" };
    let mut out = Vec::new();
    for (i, a) in ancestors.iter().enumerate() {
        let src = if post { &a.scripts.post } else { &a.scripts.pre };
        if !src.trim().is_empty() {
            let kind = if i == 0 { "Collection" } else { "Folder" };
            out.push((format!("{kind} {phase}"), src.clone()));
        }
    }
    let src = if post { &req.scripts.post } else { &req.scripts.pre };
    if !src.trim().is_empty() {
        out.push((format!("Request {phase}"), src.clone()));
    }
    out
}

/// Runs the whole send flow. Never fails: errors are reported inside the result.
pub async fn run(input: SendInput, ctx: &PipelineCtx, cancel: CancellationToken) -> SendResult {
    let response_id = uuid::Uuid::new_v4().to_string();
    let original = input.request.clone();
    let settings = lock(&ctx.settings).clone();
    let envs_before = lock(&ctx.envs).clone();
    let env_before = envs_before.active().map(|e| enabled_map(&e.variables)).unwrap_or_default();
    let globals_before = enabled_map(&envs_before.globals);
    // Script changes go to the environment that was active when the send started.
    let env_id = envs_before.active().map(|e| e.id.clone());

    let mut logs: Vec<LogLine> = Vec::new();
    let mut req = input.request.clone();
    let started = chrono::Utc::now().timestamp_millis();

    // 1. Pre-request scripts.
    let initial_sr = script_request(&req);
    let state = ScriptState {
        env: env_before.clone(),
        globals: globals_before.clone(),
        runtime: BTreeMap::new(),
        request: initial_sr.clone(),
        response: None,
    };
    let pre = run_scripts(script_list(&input.ancestors, &req, false), state, &settings).await;
    logs.extend(pre.logs);
    let mut state = pre.state;
    let mut crashed = pre.crashed;
    let mut error: Option<String> = None;
    let mut meta = None;
    let mut payload = None;
    let mut resolved_url = req.url.clone();
    let mut status = None;
    let mut time_ms = 0;

    if let Some(e) = pre.error {
        error = Some(format!("Pre-request script error: {e}"));
        logs.push(LogLine { level: LogLevel::Error, message: format!("{e} — request not sent") });
    } else if cancel.is_cancelled() {
        error = Some("Request cancelled".into());
    } else {
        apply_script_request(&mut req, &initial_sr, &state.request);

        // 2. Variables and auth.
        req.auth = effective_auth(&req, &input.ancestors);
        let mut env_now = envs_before.clone();
        let mut runtime = state.runtime.clone();
        sync_envs(&mut env_now, &env_before, &state.env, &globals_before, &state.globals, env_id.as_deref(), &mut runtime, &mut logs);
        let scope = VarScope::from(env_now.active(), &env_now.globals).with_runtime(runtime);
        let (mut resolved, unresolved) = vars::resolve_request(&req, &scope);
        for name in unresolved {
            logs.push(LogLine { level: LogLevel::Warn, message: format!("Unresolved variable {{{{{name}}}}} was sent as written") });
        }
        let auth = resolved.auth.clone();
        http::apply_auth(&mut resolved, &auth);
        resolved_url = resolved.url.clone();

        // 3. HTTP.
        let jar = settings.cookie_jar_enabled.then(|| ctx.jar.clone());
        match http::send(&resolved, jar, cancel).await {
            Ok(resp) => {
                let content_type = header(&resp.headers, "content-type").map(str::to_string);
                let set_cookies = resp
                    .headers
                    .iter()
                    .filter(|(k, _)| k.eq_ignore_ascii_case("set-cookie"))
                    .map(|(_, v)| v.clone())
                    .collect();
                payload = Some(http::body_payload(content_type.as_deref(), &resp.body));
                status = Some(resp.status);
                time_ms = resp.time_ms;
                resolved_url = resp.final_url.clone();
                state.response = Some(ScriptResponse {
                    code: resp.status,
                    status: resp.status_text.clone(),
                    headers: resp.headers.clone(),
                    body: String::from_utf8_lossy(&resp.body).into_owned(),
                    time_ms: resp.time_ms,
                });
                meta = Some(ResponseMeta {
                    status: resp.status,
                    status_text: resp.status_text,
                    headers: resp.headers,
                    size_bytes: resp.body.len() as u64,
                    time_ms: resp.time_ms,
                    content_type,
                    set_cookies,
                    final_url: resp.final_url,
                });
                ctx.keep_body(response_id.clone(), resp.body);
                if settings.cookie_jar_enabled {
                    ctx.save_cookies();
                }

                // 4. Post-response scripts.
                let post = run_scripts(script_list(&input.ancestors, &req, true), state, &settings).await;
                logs.extend(post.logs);
                state = post.state;
                crashed |= post.crashed;
                if let Some(e) = post.error {
                    logs.push(LogLine { level: LogLevel::Error, message: e.clone() });
                    error = Some(format!("Post-response script error: {e}"));
                }
            }
            Err(e) => {
                error = Some(e.to_string());
            }
        }
    }

    // 5. Persist variable changes made by scripts.
    let environments = {
        let mut envs = lock(&ctx.envs);
        let mut runtime = BTreeMap::new();
        let mut sink = Vec::new();
        let changed = !crashed
            && sync_envs(&mut envs, &env_before, &state.env, &globals_before, &state.globals, env_id.as_deref(), &mut runtime, &mut sink);
        // The first sync already logged some of these warnings; add only new ones.
        for line in sink {
            if !logs.iter().any(|l| l.message == line.message) {
                logs.push(line);
            }
        }
        if changed {
            if let Err(e) = ctx.store.save_environments(&envs) {
                logs.push(LogLine { level: LogLevel::Error, message: format!("Could not save environments: {e}") });
            }
            Some(envs.clone())
        } else {
            None
        }
    };

    // 6. History.
    let history_entry = HistoryEntry {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: started,
        method: req.method.to_uppercase(),
        url: resolved_url,
        status,
        time_ms,
        request: original,
    };
    {
        let mut h = lock(&ctx.history);
        h.insert(0, history_entry.clone());
        h.truncate(MAX_HISTORY);
        if let Err(e) = ctx.store.save_history(&h) {
            logs.push(LogLine { level: LogLevel::Error, message: format!("Could not save history: {e}") });
        }
    }

    SendResult { response_id, response: meta, body: payload, error, logs, environments, history_entry }
}

/// Applies script variable changes to an environments file. Environment changes with no active
/// environment become runtime variables and a warning. Returns true if the file changed.
#[allow(clippy::too_many_arguments)]
fn sync_envs(
    file: &mut EnvironmentsFile,
    env_before: &BTreeMap<String, String>,
    env_after: &BTreeMap<String, String>,
    globals_before: &BTreeMap<String, String>,
    globals_after: &BTreeMap<String, String>,
    env_id: Option<&str>,
    runtime: &mut BTreeMap<String, String>,
    logs: &mut Vec<LogLine>,
) -> bool {
    let mut changed = apply_var_changes(&mut file.globals, globals_before, globals_after);
    if let Some(id) = env_id {
        if let Some(env) = file.environments.iter_mut().find(|e| e.id == id) {
            changed |= apply_var_changes(&mut env.variables, env_before, env_after);
        }
    } else {
        let set: BTreeSet<&String> = env_after.iter().filter(|(k, v)| env_before.get(*k) != Some(v)).map(|(k, _)| k).collect();
        for k in set {
            runtime.insert(k.clone(), env_after[k].clone());
            logs.push(LogLine {
                level: LogLevel::Warn,
                message: format!("pm.environment.set(\"{k}\") with no active environment: kept for this send only"),
            });
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_auth_nearest_non_inherit() {
        let req = Request::default();
        let anc = vec![
            Ancestor { auth: Auth::Bearer { token: "coll".into() }, ..Default::default() },
            Ancestor { auth: Auth::Basic { username: "f".into(), password: "p".into() }, ..Default::default() },
            Ancestor { auth: Auth::Inherit, ..Default::default() },
        ];
        assert_eq!(effective_auth(&req, &anc), Auth::Basic { username: "f".into(), password: "p".into() });
        assert_eq!(effective_auth(&req, &[]), Auth::None);
        let own = Request { auth: Auth::None, ..Default::default() };
        assert_eq!(effective_auth(&own, &anc), Auth::None);
    }

    #[test]
    fn var_changes_update_add_remove() {
        let mut list = vec![
            Variable { key: "a".into(), value: "1".into(), enabled: true, secret: true },
            Variable { key: "gone".into(), value: "x".into(), enabled: true, secret: false },
        ];
        let before = enabled_map(&list);
        let mut after = before.clone();
        after.insert("a".into(), "2".into());
        after.insert("new".into(), "n".into());
        after.remove("gone");
        assert!(apply_var_changes(&mut list, &before, &after));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].value, "2");
        assert!(list[0].secret, "secret flag kept");
        assert_eq!(list[1].key, "new");
        assert!(!apply_var_changes(&mut list.clone(), &after, &after));
    }

    #[test]
    fn script_env_changes_go_to_the_env_active_at_start() {
        let mut file = EnvironmentsFile {
            environments: vec![
                crate::model::Environment { id: "a".into(), name: "a".into(), variables: vec![] },
                crate::model::Environment { id: "b".into(), name: "b".into(), variables: vec![] },
            ],
            active_id: Some("b".into()), // user switched during the send
            ..Default::default()
        };
        let before = BTreeMap::new();
        let after = BTreeMap::from([("token".to_string(), "t".to_string())]);
        let mut rt = BTreeMap::new();
        let mut logs = vec![];
        assert!(sync_envs(&mut file, &before, &after, &before, &before, Some("a"), &mut rt, &mut logs));
        assert_eq!(file.environments[0].variables.len(), 1);
        assert!(file.environments[1].variables.is_empty());
    }

    #[test]
    fn script_request_changes_apply() {
        let mut req = Request { url: "http://a".into(), body: Body::Json { text: "{}".into() }, ..Default::default() };
        let before = script_request(&req);
        let mut after = before.clone();
        after.url = "http://b".into();
        after.headers.push(("X".into(), "1".into()));
        after.body = Some("{\"a\":1}".into());
        apply_script_request(&mut req, &before, &after);
        assert_eq!(req.url, "http://b");
        assert_eq!(req.headers, vec![KeyValue::new("X", "1")]);
        assert_eq!(req.body, Body::Json { text: "{\"a\":1}".into() });
    }
}

//! Tauri commands called from the frontend (see src/lib/api.ts).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::State;
use tokio_util::sync::CancellationToken;

use crate::cookies::{self, CookieInfo};
use crate::curl::{self, ParsedCurl};
use crate::model::{Collection, EnvironmentsFile, Request, Settings, Workspace};
use crate::pipeline::{self, lock, Ancestor, PipelineCtx, SendInput, SendResult};
use crate::vars::{self, VarScope};

pub struct AppState {
    pub ctx: Arc<PipelineCtx>,
    pub cancels: Mutex<HashMap<String, CancellationToken>>,
    /// Warnings from the first load (e.g. corrupt files), shown once.
    pub load_warnings: Mutex<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadResult {
    workspace: Workspace,
    warnings: Vec<String>,
}

type CmdResult<T> = Result<T, String>;

#[tauri::command]
pub fn load_workspace(state: State<'_, AppState>) -> LoadResult {
    let ctx = &state.ctx;
    let (mut ws, more) = ctx.store.load_workspace();
    // Backend memory is the source of truth for these.
    ws.environments = lock(&ctx.envs).clone();
    ws.settings = lock(&ctx.settings).clone();
    ws.history = lock(&ctx.history).clone();
    let mut warnings = std::mem::take(&mut *lock(&state.load_warnings));
    warnings.extend(more);
    LoadResult { workspace: ws, warnings }
}

#[tauri::command]
pub fn save_collection(state: State<'_, AppState>, collection: Collection) -> CmdResult<()> {
    state.ctx.store.save_collection(&collection).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.ctx.store.delete_collection(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_environments(state: State<'_, AppState>, file: EnvironmentsFile) -> CmdResult<()> {
    let mut envs = lock(&state.ctx.envs);
    *envs = file;
    state.ctx.store.save_environments(&envs).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    let mut s = lock(&state.ctx.settings);
    *s = settings;
    state.ctx.store.save_settings(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_session(state: State<'_, AppState>, session: serde_json::Value) -> CmdResult<()> {
    state.ctx.store.save_session(&session).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> CmdResult<()> {
    let mut h = lock(&state.ctx.history);
    h.clear();
    state.ctx.store.save_history(&h).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn send_request(state: State<'_, AppState>, input: SendInput) -> CmdResult<SendResult> {
    let token = CancellationToken::new();
    let send_id = input.send_id.clone();
    lock(&state.cancels).insert(send_id.clone(), token.clone());
    let ctx = state.ctx.clone();
    let result = pipeline::run(input, &ctx, token).await;
    lock(&state.cancels).remove(&send_id);
    Ok(result)
}

#[tauri::command]
pub fn cancel_request(state: State<'_, AppState>, send_id: String) {
    if let Some(t) = lock(&state.cancels).get(&send_id) {
        t.cancel();
    }
}

#[tauri::command]
pub fn parse_curl(text: String) -> CmdResult<ParsedCurl> {
    curl::parse_curl(&text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn to_curl(state: State<'_, AppState>, request: Request, ancestors: Vec<Ancestor>, resolved: bool) -> String {
    let mut req = request;
    req.auth = pipeline::effective_auth(&req, &ancestors);
    if resolved {
        let envs = lock(&state.ctx.envs);
        let scope = VarScope::from(envs.active(), &envs.globals);
        req = vars::resolve_request(&req, &scope).0;
    }
    curl::to_curl(&req)
}

#[tauri::command]
pub fn save_response_body(state: State<'_, AppState>, response_id: String, path: String) -> CmdResult<()> {
    let body = state
        .ctx
        .body(&response_id)
        .ok_or_else(|| "This response is no longer in memory; send the request again.".to_string())?;
    std::fs::write(&path, &body).map_err(|e| format!("Could not write {path}: {e}"))
}

#[tauri::command]
pub fn list_cookies(state: State<'_, AppState>) -> Vec<CookieInfo> {
    cookies::list(&state.ctx.jar)
}

#[tauri::command]
pub fn delete_cookie(state: State<'_, AppState>, domain: String, path: String, name: String) {
    cookies::delete(&state.ctx.jar, &domain, &path, &name);
    state.ctx.save_cookies();
}

#[tauri::command]
pub fn clear_cookies(state: State<'_, AppState>) {
    cookies::clear(&state.ctx.jar);
    state.ctx.save_cookies();
}

pub mod commands;
pub mod cookies;
pub mod curl;
pub mod http;
pub mod model;
pub mod pipeline;
pub mod script;
pub mod storage;
pub mod vars;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::Manager;
use tauri::menu::{Menu, PredefinedMenuItem, Submenu};

/// macOS menu bar. It keeps Copy/Paste working in the web view but leaves out the native
/// "Close Window" (Cmd+W closes a tab here) and Undo/Redo (Cmd+Z goes to the editors).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn mac_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<Menu<R>> {
    let app_menu = Submenu::with_items(
        app,
        "Postcard",
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[&PredefinedMenuItem::minimize(app, None)?, &PredefinedMenuItem::fullscreen(app, None)?],
    )?;
    Menu::with_items(app, &[&app_menu, &edit, &window])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    http::ensure_crypto();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.handle().set_menu(mac_menu(app.handle())?)?;
            let dir = app.path().app_data_dir()?;
            let (ctx, _ws, warnings) = pipeline::PipelineCtx::load(storage::Store::new(dir));
            app.manage(commands::AppState {
                ctx: Arc::new(ctx),
                cancels: Mutex::new(HashMap::new()),
                load_warnings: Mutex::new(warnings),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_workspace,
            commands::save_collection,
            commands::delete_collection,
            commands::save_environments,
            commands::save_settings,
            commands::save_session,
            commands::clear_history,
            commands::send_request,
            commands::cancel_request,
            commands::parse_curl,
            commands::to_curl,
            commands::save_response_body,
            commands::list_cookies,
            commands::delete_cookie,
            commands::clear_cookies,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Postcard");
}

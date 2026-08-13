//! dsh-desktop: a Tauri shell that hosts the DeepSeek Harness web server
//! (`dsh web`) and points an embedded WebView2 at it.
//!
//! Security model: the harness page (http://127.0.0.1:<port>) is a plain
//! remote page and is never granted Tauri IPC access — `dangerousRemoteDomainIpcAccess`
//! is not enabled. All shell actions go through the native menu/tray and the
//! local boot page.

mod menu;
mod server;

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_opener::OpenerExt;

use server::{DshServer, ServerStatus};

pub struct AppState {
    pub server: Arc<Mutex<DshServer>>,
}

// ── commands (called only from the local boot page) ─────────────────────────

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> ServerStatus {
    state.server.lock().unwrap().status.clone()
}

#[tauri::command]
fn get_info(app: AppHandle, state: State<'_, AppState>) -> serde_json::Value {
    server::info(&app, &state.server)
}

#[tauri::command]
fn start_server(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let srv = state.server.clone();
    thread::spawn(move || {
        let _ = server::start(&app, &srv);
    });
    Ok(())
}

#[tauri::command]
fn restart_server(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let srv = state.server.clone();
    thread::spawn(move || {
        server::stop(&srv);
        thread::sleep(Duration::from_millis(600));
        let _ = server::start(&app, &srv);
    });
    Ok(())
}

#[tauri::command]
fn stop_server(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    server::stop(&state.server);
    server::navigate_boot(&app, &state.server);
    Ok(())
}

#[tauri::command]
fn get_log_tail(state: State<'_, AppState>, n: Option<usize>) -> Vec<String> {
    server::log_tail(&state.server, n.unwrap_or(100))
}

#[tauri::command]
fn open_in_browser(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let url = server::running_url(&state.server)
        .unwrap_or_else(|| format!("http://127.0.0.1:{}", server::default_port()));
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_data_dir(app: AppHandle) -> Result<(), String> {
    let home = server::dsh_home_dir(&app);
    let _ = std::fs::create_dir_all(&home);
    app.opener().reveal_item_in_dir(&home).map_err(|e| e.to_string())
}

// ── menu / tray actions ──────────────────────────────────────────────────────

fn handle_menu_action(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    match id {
        menu::MENU_OPEN_BROWSER => {
            let url = server::running_url(&state.server)
                .unwrap_or_else(|| format!("http://127.0.0.1:{}", server::default_port()));
            let _ = app.opener().open_url(url, None::<&str>);
        }
        menu::MENU_RESTART => {
            let srv = state.server.clone();
            let app2 = app.clone();
            thread::spawn(move || {
                server::stop(&srv);
                thread::sleep(Duration::from_millis(600));
                let _ = server::start(&app2, &srv);
            });
        }
        menu::MENU_OPEN_DATA_DIR => {
            let home = server::dsh_home_dir(app);
            let _ = std::fs::create_dir_all(&home);
            let _ = app.opener().reveal_item_in_dir(&home);
        }
        menu::MENU_SHOW_WINDOW => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }
        menu::MENU_QUIT => {
            app.exit(0);
        }
        _ => {}
    }
}

// ── app entry ────────────────────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            server: Arc::new(Mutex::new(DshServer::default())),
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_info,
            start_server,
            restart_server,
            stop_server,
            get_log_tail,
            open_in_browser,
            open_data_dir
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // Persistent log file for the desktop shell itself.
            if let Ok(log_dir) = app.path().app_log_dir() {
                server::init_log_file(log_dir.join("desktop.log"));
            }

            // Remember the local boot page URL so we can navigate back to it
            // when the server stops unexpectedly.
            if let Some(win) = app.get_webview_window("main") {
                if let Ok(url) = win.url() {
                    app.state::<AppState>().server.lock().unwrap().boot_url = Some(url.to_string());
                }
            }

            app.set_menu(menu::build_menu(&handle)?)?;
            menu::build_tray(&handle, handle_menu_action)?;

            // Auto-start the harness server shortly after the window appears.
            let srv = app.state::<AppState>().server.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(300));
                let _ = server::start(&handle, &srv);
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                // Ensure the dsh process tree is gone when the window closes.
                let state = window.app_handle().state::<AppState>();
                server::stop(&state.server);
            }
        })
        .on_menu_event(|app, event| handle_menu_action(app, event.id.as_ref()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

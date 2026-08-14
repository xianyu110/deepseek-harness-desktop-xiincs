//! dsh-desktop: a Tauri shell that hosts the DeepSeek Harness web server
//! (`dsh web`) and points an embedded WebView2 at it.
//!
//! Security model: the harness page (http://127.0.0.1:<port>) is a plain
//! remote page and is never granted Tauri IPC access — `dangerousRemoteDomainIpcAccess`
//! is not enabled. All shell actions go through the native menu/tray and the
//! local boot page.

mod menu;
mod server;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

use server::{DshServer, ServerStatus};

pub struct AppState {
    pub server: Arc<Mutex<DshServer>>,
    /// Whether the one-time "minimized to tray" notice has fired this run.
    hide_notice_shown: AtomicBool,
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
    thread::spawn(move || server::restart(&app, &srv));
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

#[derive(serde::Serialize)]
struct UpdateInfo {
    version: String,
    body: Option<String>,
}

/// Checks the configured updater endpoint for a newer shell release. Returns
/// `None` if the current version is already the latest (or the check
/// failed — network errors here shouldn't block the app from starting).
#[tauri::command]
async fn check_for_update(app: AppHandle) -> Option<UpdateInfo> {
    let update = app.updater().ok()?.check().await.ok()??;
    Some(UpdateInfo {
        version: update.version,
        body: update.body,
    })
}

/// Re-checks for an update and, if one is still available, downloads,
/// verifies (against the pinned pubkey) and installs it, then relaunches.
/// Re-checking here (rather than trusting a version string round-tripped
/// from `check_for_update`) avoids installing a stale/unverified `Update`.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "没有可用的更新".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

// ── window helpers ───────────────────────────────────────────────────────────

/// Un-hide, un-minimize, and focus the main window. Used by both the tray's
/// "显示窗口" action and the single-instance relaunch callback below, so a
/// second launch attempt (desktop icon, Start menu, ...) surfaces the
/// existing window instead of spawning a second process/window/tray icon.
fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Disables WebView2's built-in right-click context menu (Back / Forward /
/// Reload / Inspect / …). The harness page is a plain remote page with no
/// Tauri-side navigation UI of its own, so that default menu was the only
/// way a user could reach browser-style back/forward — and "back" lands on
/// the local boot page with no way back to the harness UI short of a
/// restart (there is no in-app forward affordance, and the boot page's own
/// readiness check does not re-run on a history navigation). Removing the
/// menu removes the discoverable path into that dead end. `Settings` lives
/// on the `CoreWebView2` instance, not the page, so this only needs to run
/// once — it stays in effect across every later `navigate()` call (both to
/// the harness URL and back to the boot page on stop/restart).
#[cfg(windows)]
fn disable_context_menu(win: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
    let _ = win.with_webview(|webview| {
        let controller: ICoreWebView2Controller = webview.controller();
        let result: windows::core::Result<()> = (|| unsafe {
            let core = controller.CoreWebView2()?;
            let settings = core.Settings()?;
            settings.SetAreDefaultContextMenusEnabled(false)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("[dsh-desktop] failed to disable WebView2 context menu: {e}");
        }
    });
}
#[cfg(not(windows))]
fn disable_context_menu(_win: &tauri::WebviewWindow) {}

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
            thread::spawn(move || server::restart(&app2, &srv));
        }
        menu::MENU_OPEN_DATA_DIR => {
            let home = server::dsh_home_dir(app);
            let _ = std::fs::create_dir_all(&home);
            let _ = app.opener().reveal_item_in_dir(&home);
        }
        menu::MENU_SHOW_WINDOW => show_main_window(app),
        menu::MENU_QUIT => {
            // stop() is a safe no-op in attach mode (pid stays None there),
            // so this only tears down a server we actually spawned.
            server::stop(&state.server);
            app.exit(0);
        }
        _ => {}
    }
}

// ── app entry ────────────────────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin registered: it needs to claim the
        // single-instance lock before anything else in the builder chain
        // runs. A second launch (desktop icon, Start menu, ...) hits this
        // callback in the *first* process and exits immediately instead of
        // creating its own window/tray icon — see `show_main_window`.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            server: Arc::new(Mutex::new(DshServer::default())),
            hide_notice_shown: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_info,
            start_server,
            restart_server,
            stop_server,
            get_log_tail,
            open_in_browser,
            open_data_dir,
            check_for_update,
            install_update
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
                disable_context_menu(&win);
            }

            // A window/app menu set via `set_menu()` becomes the global
            // top-of-screen menu bar on macOS (platform convention, doesn't
            // cost window space) but a classic in-window Win32-style menu
            // strip on Windows/Linux — stacked right under the native title
            // bar, i.e. two layers of chrome for one "文件" entry. Every
            // action it offered already lives in the tray menu below, so
            // only set it on macOS.
            #[cfg(target_os = "macos")]
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
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Tray-resident mode: closing the window hides it but leaves
                // the dsh server running in the background. Only the
                // menu/tray "退出" action (MENU_QUIT, app.exit) stops the
                // server and actually exits the process.
                api.prevent_close();
                let _ = window.hide();

                let app = window.app_handle();
                let state = app.state::<AppState>();
                if state.hide_notice_shown.swap(true, Ordering::Relaxed) {
                    return;
                }
                let _ = app
                    .notification()
                    .builder()
                    .title("DeepSeek Harness 已转入后台")
                    .body("服务仍在运行；从系统托盘图标可重新打开窗口或退出。")
                    .show();
            }
        })
        .on_menu_event(|app, event| handle_menu_action(app, event.id.as_ref()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

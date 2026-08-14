//! Native menu and tray for the shell.
//!
//! Menu/tray item ids are handled in `lib.rs` (`handle_menu_action`) so the
//! actions can reach the shared app state; `build_tray` takes the handler as a
//! callback to avoid a circular dependency.

use std::sync::Arc;

use tauri::menu::{CheckMenuItem, Menu, MenuItem};
#[cfg(target_os = "macos")]
use tauri::menu::Submenu;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};
use tauri_plugin_autostart::ManagerExt;

pub const MENU_OPEN_BROWSER: &str = "open_browser";
pub const MENU_RESTART: &str = "restart";
pub const MENU_OPEN_DATA_DIR: &str = "open_data_dir";
pub const MENU_SHOW_WINDOW: &str = "show_window";
pub const MENU_TOGGLE_AUTOSTART: &str = "toggle_autostart";
pub const MENU_QUIT: &str = "quit";

/// macOS-only — see the `set_menu()` callsite in lib.rs for why.
#[cfg(target_os = "macos")]
pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let open_browser = MenuItem::with_id(app, MENU_OPEN_BROWSER, "在浏览器中打开", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, MENU_RESTART, "重启服务", true, None::<&str>)?;
    let open_data_dir = MenuItem::with_id(app, MENU_OPEN_DATA_DIR, "打开数据目录 (~/.dsh)", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let file = Submenu::with_items(app, "文件", true, &[&open_browser, &restart, &open_data_dir, &quit])?;
    Ok(Menu::with_items(app, &[&file])?)
}

/// Builds the tray icon/menu and returns the autostart toggle's
/// `CheckMenuItem` handle so the caller can keep its checked state in sync
/// after every toggle (see `MENU_TOGGLE_AUTOSTART` in lib.rs's
/// `handle_menu_action` — clicking a `CheckMenuItem` does not flip its own
/// visual state automatically).
pub fn build_tray(
    app: &AppHandle,
    on_action: impl Fn(&AppHandle, &str) + Send + Sync + 'static,
) -> tauri::Result<CheckMenuItem<Wry>> {
    let show = MenuItem::with_id(app, MENU_SHOW_WINDOW, "显示窗口", true, None::<&str>)?;
    let open_browser = MenuItem::with_id(app, MENU_OPEN_BROWSER, "在浏览器中打开", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, MENU_RESTART, "重启服务", true, None::<&str>)?;
    // On Windows/Linux this tray is the *only* menu (see the `set_menu()`
    // callsite in lib.rs) — include everything the removed window menu
    // offered, not just what macOS's menu bar leaves uncovered.
    let open_data_dir = MenuItem::with_id(app, MENU_OPEN_DATA_DIR, "打开数据目录 (~/.dsh)", true, None::<&str>)?;
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);
    let autostart = CheckMenuItem::with_id(
        app,
        MENU_TOGGLE_AUTOSTART,
        "开机自动启动",
        true,
        autostart_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &open_browser, &restart, &open_data_dir, &autostart, &quit])?;

    // Shared between on_menu_event and on_tray_icon_event below (a left
    // click routes through the same MENU_SHOW_WINDOW id/handler as the
    // "显示窗口" item, so both paths stay in sync by construction).
    let on_action = Arc::new(on_action);
    let on_action_click = on_action.clone();

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("DeepSeek Harness")
        .menu(&menu)
        // Left click shows the window directly; only right click opens this
        // menu (the platform default once this is off).
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| on_action(app, event.id.as_ref()))
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                on_action_click(tray.app_handle(), MENU_SHOW_WINDOW);
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(autostart)
}

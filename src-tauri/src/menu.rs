//! Native menu and tray for the shell.
//!
//! Menu/tray item ids are handled in `lib.rs` (`handle_menu_action`) so the
//! actions can reach the shared app state; `build_tray` takes the handler as a
//! callback to avoid a circular dependency.

use tauri::menu::{Menu, MenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Wry};

pub const MENU_OPEN_BROWSER: &str = "open_browser";
pub const MENU_RESTART: &str = "restart";
pub const MENU_OPEN_DATA_DIR: &str = "open_data_dir";
pub const MENU_SHOW_WINDOW: &str = "show_window";
pub const MENU_QUIT: &str = "quit";

pub fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let open_browser = MenuItem::with_id(app, MENU_OPEN_BROWSER, "在浏览器中打开", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, MENU_RESTART, "重启服务", true, None::<&str>)?;
    let open_data_dir = MenuItem::with_id(app, MENU_OPEN_DATA_DIR, "打开数据目录 (~/.dsh)", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let file = Submenu::with_items(app, "文件", true, &[&open_browser, &restart, &open_data_dir, &quit])?;
    Ok(Menu::with_items(app, &[&file])?)
}

pub fn build_tray(
    app: &AppHandle,
    on_action: impl Fn(&AppHandle, &str) + Send + Sync + 'static,
) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW_WINDOW, "显示窗口", true, None::<&str>)?;
    let open_browser = MenuItem::with_id(app, MENU_OPEN_BROWSER, "在浏览器中打开", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, MENU_RESTART, "重启服务", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &open_browser, &restart, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("DeepSeek Harness")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| on_action(app, event.id.as_ref()));
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    App, Manager,
};

/// 点击穿透开关状态（`set_ignore_cursor_events` 无 getter，需自行记录）。
static CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

/// 创建系统托盘图标与菜单。
pub fn create_tray(app: &App) -> tauri::Result<()> {
    let toggle_window = MenuItem::with_id(
        app,
        "toggle-window",
        "显示/隐藏主窗口",
        true,
        None::<&str>,
    )?;
    let toggle_click_through = CheckMenuItem::with_id(
        app,
        "toggle-click-through",
        "点击穿透",
        true,
        false,
        None::<&str>,
    )?;
    let click_through_item = toggle_click_through.clone();
    let test_sound = MenuItem::with_id(app, "test-sound", "测试提示音", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle_window,
            &toggle_click_through,
            &test_sound,
            &separator,
            &quit,
        ],
    )?;

    // 托盘图标直接使用 src-tauri/icons/icon.png（编译期内嵌）
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;

    let _tray = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("DSH 桌宠")
        .menu(&menu)
        // macOS/Windows 左键也弹出菜单（Linux 不支持该开关）
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "toggle-window" => {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            "toggle-click-through" => {
                let next = !CLICK_THROUGH.load(Ordering::SeqCst);
                CLICK_THROUGH.store(next, Ordering::SeqCst);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_ignore_cursor_events(next);
                }
                let _ = click_through_item.set_checked(next);
            }
            "test-sound" => {
                crate::audio::play_sound(app.clone(), "done".to_string());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    App, Emitter, Manager,
};

/// 点击穿透开关状态（`set_ignore_cursor_events` 无 getter，需自行记录）。
static CLICK_THROUGH: AtomicBool = AtomicBool::new(false);

/// 音效开关。
static SOUND_ENABLED: AtomicBool = AtomicBool::new(true);
/// 点击彩蛋开关。
static EASTER_EGG_ENABLED: AtomicBool = AtomicBool::new(true);
/// 位置记忆开关。
static REMEMBER_POSITION: AtomicBool = AtomicBool::new(true);

pub fn is_sound_enabled() -> bool {
    SOUND_ENABLED.load(Ordering::SeqCst)
}
pub fn is_easter_egg_enabled() -> bool {
    EASTER_EGG_ENABLED.load(Ordering::SeqCst)
}
pub fn is_remember_position() -> bool {
    REMEMBER_POSITION.load(Ordering::SeqCst)
}

/// 免打扰模式开关。
static DND_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn is_dnd_enabled() -> bool {
    DND_ENABLED.load(Ordering::SeqCst)
}

/// 创建系统托盘图标与菜单。
pub fn create_tray(app: &App) -> tauri::Result<()> {
    let toggle_window =
        MenuItem::with_id(app, "toggle-window", "显示/隐藏主窗口", true, None::<&str>)?;
    let toggle_click_through = CheckMenuItem::with_id(
        app,
        "toggle-click-through",
        "点击穿透",
        true,
        false,
        None::<&str>,
    )?;
    let click_through_item = toggle_click_through.clone();

    // 设置开关：音效 / 彩蛋 / 位置记忆，初始值从配置恢复
    let cfg = crate::config::load(app.handle());
    SOUND_ENABLED.store(cfg.sound_enabled, Ordering::SeqCst);
    EASTER_EGG_ENABLED.store(cfg.easter_egg_enabled, Ordering::SeqCst);
    REMEMBER_POSITION.store(cfg.remember_position, Ordering::SeqCst);
    let toggle_sound = CheckMenuItem::with_id(
        app,
        "toggle-sound",
        "音效",
        true,
        cfg.sound_enabled,
        None::<&str>,
    )?;
    let sound_item = toggle_sound.clone();
    let toggle_easter = CheckMenuItem::with_id(
        app,
        "toggle-easter",
        "点击彩蛋",
        true,
        cfg.easter_egg_enabled,
        None::<&str>,
    )?;
    let easter_item = toggle_easter.clone();
    let toggle_remember = CheckMenuItem::with_id(
        app,
        "toggle-remember",
        "位置记忆",
        true,
        cfg.remember_position,
        None::<&str>,
    )?;
    let remember_item = toggle_remember.clone();
    let autostart_initial = crate::autostart::is_enabled();
    let toggle_autostart = CheckMenuItem::with_id(
        app,
        "toggle-autostart",
        "开机自启",
        true,
        autostart_initial,
        None::<&str>,
    )?;
    let autostart_item = toggle_autostart.clone();
    let dnd_initial = crate::config::load(app.handle()).dnd_enabled;
    DND_ENABLED.store(dnd_initial, Ordering::SeqCst);
    let toggle_dnd =
        CheckMenuItem::with_id(app, "toggle-dnd", "免打扰", true, dnd_initial, None::<&str>)?;
    let dnd_item = toggle_dnd.clone();

    let test_sound = MenuItem::with_id(app, "test-sound", "测试提示音", true, None::<&str>)?;

    // 皮肤子菜单：内置默认 + 用户自定义皮肤
    let mut owned_theme_items: Vec<tauri::menu::MenuItem<tauri::Wry>> = Vec::new();
    let builtin_theme = MenuItem::with_id(app, "theme:", "内置鲸鱼娘", true, None::<&str>)?;
    owned_theme_items.push(builtin_theme);
    let new_template = MenuItem::with_id(
        app,
        "new-theme-template",
        "新建皮肤模板…",
        true,
        None::<&str>,
    )?;
    owned_theme_items.push(new_template);
    let edit_theme = MenuItem::with_id(app, "edit-theme", "打开皮肤编辑器…", true, None::<&str>)?;
    owned_theme_items.push(edit_theme.clone());
    for t in crate::theme::scan_themes(app.handle()) {
        let id = format!("theme:{}", t.id);
        if let Ok(item) = MenuItem::with_id(app, &id, &t.name, true, None::<&str>) {
            owned_theme_items.push(item);
        }
    }
    let theme_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = owned_theme_items
        .iter()
        .map(|i| i as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let theme_submenu = Submenu::with_items(app, "皮肤", true, &theme_refs)?;
    let open_themes = MenuItem::with_id(app, "open-themes", "打开皮肤目录…", true, None::<&str>)?;

    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &toggle_window,
            &toggle_click_through,
            &toggle_sound,
            &toggle_easter,
            &toggle_remember,
            &toggle_autostart,
            &toggle_dnd,
            &theme_submenu,
            &open_themes,
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
        .on_menu_event(move |app, event| {
            let id = event.id().0.clone();
            if let Some(theme_id) = id.strip_prefix("theme:") {
                let mut cfg = crate::config::load(app);
                cfg.theme = theme_id.to_string();
                crate::config::save(app, &cfg);
                let _ = app.emit("theme-changed", theme_id.to_string());
                return;
            }
            match id.as_str() {
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
                "toggle-sound" => {
                    let next = !SOUND_ENABLED.load(Ordering::SeqCst);
                    SOUND_ENABLED.store(next, Ordering::SeqCst);
                    let mut c = crate::config::load(app);
                    c.sound_enabled = next;
                    crate::config::save(app, &c);
                    let _ = sound_item.set_checked(next);
                }
                "toggle-easter" => {
                    let next = !EASTER_EGG_ENABLED.load(Ordering::SeqCst);
                    EASTER_EGG_ENABLED.store(next, Ordering::SeqCst);
                    let mut c = crate::config::load(app);
                    c.easter_egg_enabled = next;
                    crate::config::save(app, &c);
                    let _ = easter_item.set_checked(next);
                }
                "toggle-dnd" => {
                    let next = !DND_ENABLED.load(Ordering::SeqCst);
                    DND_ENABLED.store(next, Ordering::SeqCst);
                    let mut c = crate::config::load(app);
                    c.dnd_enabled = next;
                    crate::config::save(app, &c);
                    let _ = app.emit("pet-dnd", next);
                    let _ = dnd_item.set_checked(next);
                }
                "toggle-autostart" => {
                    let next = !crate::autostart::is_enabled();
                    crate::autostart::set_enabled(next);
                    let _ = autostart_item.set_checked(next);
                }
                "toggle-remember" => {
                    let next = !REMEMBER_POSITION.load(Ordering::SeqCst);
                    REMEMBER_POSITION.store(next, Ordering::SeqCst);
                    let mut c = crate::config::load(app);
                    c.remember_position = next;
                    crate::config::save(app, &c);
                    let _ = remember_item.set_checked(next);
                }
                "new-theme-template" => {
                    if let Some(t) = crate::theme::create_template_theme(app) {
                        log::info!("[theme] 已创建皮肤模板: {}", t.id);
                    }
                    crate::theme::open_themes_dir(app);
                }
                "edit-theme" => {
                    crate::theme::open_editor(app);
                }
                "open-themes" => {
                    crate::theme::open_themes_dir(app);
                }
                "test-sound" => {
                    crate::audio::play_sound(app.clone(), "done".to_string());
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

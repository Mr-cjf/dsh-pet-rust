//! Tauri command：供前端 invoke 查询状态快照与移动窗口。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tauri::{Emitter, LogicalPosition, Manager, State};

use crate::engine::{self, Snapshot};
use crate::hit_test::{self, Rect};

/// 返回当前 DSH 状态快照（五态 + attention/running/done 标题列表）。
#[tauri::command]
pub fn get_state(state: State<'_, Arc<Mutex<engine::EngineState>>>) -> Snapshot {
    let st = state.lock().unwrap();
    engine::build_snapshot(&st)
}

/// 计算桌宠显示尺寸：基础 400 按显示器缩放因子等比缩放，竖屏加成 20%。
#[tauri::command]
pub fn get_pet_size(app: tauri::AppHandle) -> f64 {
    let base = 400.0;
    let Some(window) = app.get_webview_window("main") else {
        return base;
    };
    let Some(monitor) = window
        .primary_monitor()
        .ok()
        .flatten()
        .or(window.current_monitor().ok().flatten())
    else {
        return base;
    };
    let scale = monitor.scale_factor().max(1.0);
    let sz = monitor.size();
    let w = sz.width as f64;
    let h = sz.height as f64;
    let mut s = base * scale;
    // 竖屏显示器（高远大于宽）时宠物加成，避免过小
    if h > w * 1.2 {
        s *= 1.2;
    }
    s
}

/// 返回所有可用皮肤列表。
#[tauri::command]
pub fn get_themes(app: tauri::AppHandle) -> Vec<crate::theme::ThemeInfo> {
    crate::theme::scan_themes(&app)
}

/// 返回当前皮肤：{ theme_id, animations: {事件名: 绝对路径数组} }。
/// 每个事件值都是数组（归一化后），即使主题里只填了单字符串，也返回单元素数组。
/// 文件名不存在的元素会被丢弃，前端据此在多动画池里随机选择。
#[tauri::command]
pub fn get_theme(app: tauri::AppHandle) -> serde_json::Value {
    let cfg = crate::config::load(&app);
    let id = cfg.theme;
    if id.is_empty() {
        return serde_json::json!({ "theme_id": "", "animations": null });
    }
    let Some(theme) = crate::theme::load_theme(&app, &id) else {
        return serde_json::json!({ "theme_id": id, "animations": null });
    };
    let mut anims = serde_json::Map::new();
    for (k, files) in crate::theme::normalized_animations(&theme) {
        let mut arr = Vec::new();
        for f in files {
            if let Some(path) = crate::theme::theme_animation_path(&app, &id, &f) {
                arr.push(serde_json::Value::String(
                    path.to_string_lossy().to_string(),
                ));
            }
        }
        if !arr.is_empty() {
            anims.insert(k, serde_json::Value::Array(arr));
        }
    }
    serde_json::json!({ "theme_id": id, "animations": anims })
}

#[tauri::command]
pub fn get_theme_definition(
    app: tauri::AppHandle,
    id: String,
) -> Result<serde_json::Value, String> {
    let theme = crate::theme::load_theme(&app, &id).ok_or_else(|| "皮肤不存在".to_string())?;
    let animations = crate::theme::normalized_animations(&theme)
        .into_iter()
        .map(|(key, files)| {
            let paths = files
                .into_iter()
                .filter_map(|file| crate::theme::theme_animation_path(&app, &id, &file))
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>();
            (key, paths)
        })
        .filter(|(_, files)| !files.is_empty())
        .collect::<HashMap<_, _>>();
    Ok(serde_json::json!({ "id": id, "name": theme.name, "animations": animations }))
}

/// 使用桌面原生文件对话框选择一个或多个 WebM 动画文件。
/// 返回绝对路径，避免依赖 WebView2 不保证提供的 `File.path` 属性。
#[tauri::command]
pub fn pick_webm_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let files = rfd::FileDialog::new()
        .add_filter("WebM 动画", &["webm"])
        .set_title("选择 WebM 动画（可多选）")
        .pick_files()
        .unwrap_or_default();
    let staging = crate::theme::themes_dir(&app).join(".editor-staging");
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let batch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let mut paths = Vec::new();
    for (index, source) in files.into_iter().enumerate() {
        let name = source
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or_else(|| "动画文件名无效".to_string())?;
        if !name.to_ascii_lowercase().ends_with(".webm") {
            return Err(format!("仅支持 webm：{name}"));
        }
        let target = staging.join(format!("{batch}-{index}-{name}"));
        std::fs::copy(&source, &target).map_err(|e| e.to_string())?;
        paths.push(target.to_string_lossy().to_string());
    }
    Ok(paths)
}

#[tauri::command]
pub fn save_theme(
    app: tauri::AppHandle,
    id: Option<String>,
    name: String,
    animations: HashMap<String, Vec<String>>,
) -> Result<crate::theme::ThemeInfo, String> {
    let clean_name = name.trim();
    if clean_name.is_empty() {
        return Err("请输入皮肤名称".to_string());
    }
    let theme_id = id.filter(|v| !v.trim().is_empty()).unwrap_or_else(|| {
        let slug: String = clean_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        format!("custom-{}-{}", slug.trim_matches('-'), std::process::id())
    });
    if theme_id == "" || theme_id.contains("..") || theme_id.contains(['/', '\\']) {
        return Err("皮肤标识无效".to_string());
    }
    let dir = crate::theme::themes_dir(&app).join(&theme_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut stored = HashMap::new();
    for (key, files) in animations {
        let mut out = Vec::new();
        for (index, file) in files.into_iter().enumerate() {
            let trimmed = file.trim();
            if trimmed.is_empty() {
                continue;
            }
            let source = std::path::PathBuf::from(trimmed);
            let target_name = source
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or_else(|| "动画文件名无效".to_string())?;
            if !target_name.to_ascii_lowercase().ends_with(".webm") {
                return Err(format!("仅支持 webm：{}", target_name));
            }
            let target = dir.join(target_name);
            if source.is_absolute() && source.exists() {
                if source == target {
                    out.push(target_name.to_string());
                    continue;
                }
                let final_target = if target.exists() {
                    dir.join(format!(
                        "{}-{}.webm",
                        target_name.trim_end_matches(".webm"),
                        index
                    ))
                } else {
                    target
                };
                std::fs::copy(&source, &final_target).map_err(|e| e.to_string())?;
                out.push(
                    final_target
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );
            } else if !source.is_absolute() && dir.join(&source).exists() {
                out.push(source.to_string_lossy().to_string());
            } else {
                return Err(format!("动画文件不存在：{trimmed}"));
            }
        }
        if !out.is_empty() {
            stored.insert(key, out);
        }
    }
    let json = serde_json::json!({ "name": clean_name, "animations": stored });
    std::fs::write(
        dir.join("theme.json"),
        serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut cfg = crate::config::load(&app);
    cfg.theme = theme_id.clone();
    crate::config::save(&app, &cfg);
    let info = crate::theme::ThemeInfo {
        id: theme_id.clone(),
        name: clean_name.to_string(),
    };
    let _ = app.emit("theme-changed", theme_id);
    Ok(info)
}

/// 返回主窗口当前位置（逻辑坐标，CSS 像素，与前端 e.screenX/e.screenY 一致）。
#[tauri::command]
pub fn get_window_position(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main 窗口不存在".to_string())?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?.max(1.0);
    let x = pos.x as f64 / scale;
    let y = pos.y as f64 / scale;
    Ok(serde_json::json!({ "x": x, "y": y }))
}

/// 把窗口位置约束在所有显示器的虚拟桌面范围内（支持跨屏拖拽）。
fn clamp_to_desktop(
    window: &tauri::WebviewWindow<tauri::Wry>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    scale: f64,
) -> (f64, f64) {
    let monitors = window.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        return (x, y);
    }
    let min_x = monitors
        .iter()
        .map(|m| m.position().x as f64 / scale)
        .fold(f64::INFINITY, f64::min);
    let min_y = monitors
        .iter()
        .map(|m| m.position().y as f64 / scale)
        .fold(f64::INFINITY, f64::min);
    let max_x = monitors
        .iter()
        .map(|m| (m.position().x + m.size().width as i32) as f64 / scale)
        .fold(f64::NEG_INFINITY, f64::max);
    let max_y = monitors
        .iter()
        .map(|m| (m.position().y + m.size().height as i32) as f64 / scale)
        .fold(f64::NEG_INFINITY, f64::max);
    (
        x.clamp(min_x, (max_x - w).max(min_x)),
        y.clamp(min_y, (max_y - h).max(min_y)),
    )
}

/// 移动主窗口到指定逻辑坐标，并约束在所有显示器的虚拟桌面内。
/// 移动主窗口到指定逻辑坐标，并约束在主显示器工作区内。
#[tauri::command]
pub fn move_window(app: tauri::AppHandle, x: f64, y: f64) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main 窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?.max(1.0);
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;

    let (cx, cy) = clamp_to_desktop(&window, x, y, w, h, scale);
    window
        .set_position(LogicalPosition::new(cx, cy))
        .map_err(|e| e.to_string())
}

/// 拖拽快照：按下时的全局光标屏幕坐标 + 窗口逻辑位置。
/// 拖拽期间窗口位置 = 按下窗口位置 + (当前光标 - 按下光标)，用屏幕坐标计算，
/// 彻底避免前端 clientX 随窗口移动导致的漂移/来回跳。
static DRAG_SNAPSHOT: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);

#[cfg(target_os = "windows")]
fn cursor_screen_point() -> Option<(f64, f64)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut pt) }.ok()?;
    Some((pt.x as f64, pt.y as f64))
}

#[cfg(not(target_os = "windows"))]
fn cursor_screen_point() -> Option<(f64, f64)> {
    None
}

/// 记录拖拽快照（按下时调用）。
#[tauri::command]
pub fn drag_start(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main 窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?.max(1.0);
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let Some((cx, cy)) = cursor_screen_point() else {
        return Err("无法获取全局光标位置".to_string());
    };
    let wx = pos.x as f64 / scale;
    let wy = pos.y as f64 / scale;
    *DRAG_SNAPSHOT.lock().unwrap() = Some((cx, cy, wx, wy));
    Ok(())
}

/// 拖拽移动：按快照 + 当前光标屏幕坐标计算窗口绝对位置。
#[tauri::command]
pub fn drag_move(app: tauri::AppHandle) -> Result<(), String> {
    let snap = *DRAG_SNAPSHOT.lock().unwrap();
    let Some((cx0, cy0, wx0, wy0)) = snap else {
        return Ok(());
    };
    let Some((cx, cy)) = cursor_screen_point() else {
        return Ok(());
    };
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main 窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?.max(1.0);
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let x = wx0 + (cx - cx0);
    let y = wy0 + (cy - cy0);
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;

    let (cx2, cy2) = clamp_to_desktop(&window, x, y, w, h, scale);
    window
        .set_position(LogicalPosition::new(cx2, cy2))
        .map_err(|e| e.to_string())
}

/// 结束拖拽，清除快照并记录窗口位置。
#[tauri::command]
pub fn drag_end(app: tauri::AppHandle) {
    *DRAG_SNAPSHOT.lock().unwrap() = None;
    if crate::tray::is_remember_position() {
        crate::config::remember_window_position(&app);
    }
}

/// 当前是否启用点击彩蛋。
#[tauri::command]
pub fn get_easter_egg_enabled() -> bool {
    crate::tray::is_easter_egg_enabled()
}

/// 按增量移动主窗口（帧间增量方式，避免 clientX 基准随窗口移动而漂移）。
#[tauri::command]
pub fn move_window_by(app: tauri::AppHandle, dx: f64, dy: f64) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main 窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?.max(1.0);
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let x = pos.x as f64 / scale + dx;
    let y = pos.y as f64 / scale + dy;
    let w = size.width as f64 / scale;
    let h = size.height as f64 / scale;

    let (cx, cy) = clamp_to_desktop(&window, x, y, w, h, scale);
    window
        .set_position(LogicalPosition::new(cx, cy))
        .map_err(|e| e.to_string())
}

/// 更新鼠标命中区（窗口逻辑坐标，CSS 像素）。`None` 表示整窗穿透。
#[tauri::command]
pub fn set_hit_region(rect: Option<Rect>) {
    hit_test::set_hit_region(rect);
}

/// 前端日志透传：把鲸鱼娘渲染/视频加载等诊断信息打到 Rust 日志。
#[tauri::command]
pub fn frontend_log(msg: String) {
    log::info!("[frontend] {msg}");
}

//! Tauri command：供前端 invoke 查询状态快照与移动窗口。

use std::sync::{Arc, Mutex};

use tauri::{LogicalPosition, Manager, State};

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

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

/// 移动主窗口到指定逻辑坐标（CSS 像素，与前端 e.screenX/e.screenY 一致）。
#[tauri::command]
pub fn move_window(app: tauri::AppHandle, x: f64, y: f64) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main 窗口不存在".to_string())?;
    window
        .set_position(LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}

/// 更新鼠标命中区（窗口逻辑坐标，CSS 像素）。`None` 表示整窗穿透。
#[tauri::command]
pub fn set_hit_region(rect: Option<Rect>) {
    hit_test::set_hit_region(rect);
}

//! 区域点击穿透：光标穿透轮询层。
//!
//! 核心机制：前端把鲸鱼娘矩形（窗口逻辑坐标）通过 `set_hit_region` 上报，
//! Rust 后台线程每 20ms 读取全局光标位置，判断光标是否落在鲸鱼娘矩形内：
//! - 在矩形内：`set_ignore_cursor_events(false)`，窗口接收鼠标（可拖拽/点击鲸鱼娘）；
//! - 在矩形外：`set_ignore_cursor_events(true)`，整窗穿透，不挡下层窗口操作。
//!
//! 说明：原先的 Windows WM_NCHITTEST 子类化方案在 Tauri v2 + WebView2 下
//! 子类化不生效（自定义窗口过程不会被调用），导致透明置顶窗口劫持鼠标，
//! 已改为官方 `set_ignore_cursor_events` 方案。

use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 命中矩形（窗口逻辑坐标，CSS 像素）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    /// 点是否落在矩形内（含左/上边界，不含右/下边界）。
    #[inline]
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// 共享命中区：`None` 表示无命中区（整窗穿透）。
static HIT_REGION: Mutex<Option<Rect>> = Mutex::new(None);

/// 设置命中区（窗口逻辑坐标，CSS 像素）。
pub fn set_hit_region(rect: Option<Rect>) {
    *HIT_REGION.lock().unwrap_or_else(|p| p.into_inner()) = rect;
}

/// 读取当前命中区。
pub fn get_hit_region() -> Option<Rect> {
    *HIT_REGION.lock().unwrap_or_else(|p| p.into_inner())
}

/// 启动“光标穿透”轮询：根据全局光标是否落在命中区内，动态切换整窗点击穿透。
pub fn start_cursor_through(app: tauri::AppHandle) {
    #[cfg(target_os = "windows")]
    {
        win::start_cursor_through(app);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        log::warn!("[hit_test] 当前平台的光标穿透轮询尚未实现");
    }
}

#[cfg(target_os = "windows")]
mod win {
    use tauri::Manager;
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    /// 后台轮询全局光标，动态切换整窗点击穿透。
    pub fn start_cursor_through(app: tauri::AppHandle) {
        std::thread::spawn(move || {
            let mut last: Option<bool> = None;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(20));
                let Some(window) = app.get_webview_window("main") else {
                    continue;
                };
                let mut pt = POINT { x: 0, y: 0 };
                if unsafe { GetCursorPos(&mut pt) }.is_err() {
                    continue;
                }
                let Ok(pos) = window.outer_position() else { continue };
                let Ok(scale) = window.scale_factor() else { continue };
                let scale = scale.max(1.0);
                let region = super::get_hit_region();
                let inside = region.map_or(false, |r| {
                    let lx = (pt.x as f64 - pos.x as f64) / scale;
                    let ly = (pt.y as f64 - pos.y as f64) / scale;
                    r.contains(lx, ly)
                });
                let ignore = !inside;
                if last == Some(ignore) {
                    continue;
                }
                last = Some(ignore);
                let _ = window.set_ignore_cursor_events(ignore);
            }
        });
    }
}

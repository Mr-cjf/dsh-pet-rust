//! 桌宠本地配置：窗口位置记忆、极简模式开关等。
//! 保存到 Tauri 的 app_config_dir 下的 config.json。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetConfig {
    /// 上次窗口逻辑位置 x（未保存过则为 None）
    pub window_x: Option<f64>,
    /// 上次窗口逻辑位置 y
    pub window_y: Option<f64>,
    /// 音效开关
    pub sound_enabled: bool,
    /// 点击彩蛋开关
    pub easter_egg_enabled: bool,
    /// 位置记忆开关
    pub remember_position: bool,
}

impl Default for PetConfig {
    fn default() -> Self {
        Self {
            window_x: None,
            window_y: None,
            sound_enabled: true,
            easter_egg_enabled: true,
            remember_position: true,
        }
    }
}

fn config_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("config.json")
}

pub fn load(app: &AppHandle) -> PetConfig {
    let path = config_path(app);
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => PetConfig::default(),
    }
}

pub fn save(app: &AppHandle, config: &PetConfig) {
    let path = config_path(app);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(s) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(&path, s);
    }
}

/// 记录当前主窗口逻辑位置到配置（拖拽/移动后调用）。
pub fn remember_window_position(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let Ok(pos) = window.outer_position() else { return };
    let Ok(scale) = window.scale_factor() else { return };
    let scale = scale.max(1.0);
    let mut config = load(app);
    config.window_x = Some(pos.x as f64 / scale);
    config.window_y = Some(pos.y as f64 / scale);
    save(app, &config);
}

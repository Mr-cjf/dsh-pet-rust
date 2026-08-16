use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// 解析音效文件路径：优先打包资源目录，开发态回退到 `CARGO_MANIFEST_DIR/resources/`。
fn resolve_sound_path(app: &AppHandle, name: &str) -> Option<PathBuf> {
    let file = format!("{name}.m4a");

    if let Ok(dir) = app.path().resource_dir() {
        let bundled = dir.join("resources").join(&file);
        if bundled.exists() {
            return Some(bundled);
        }
        let flat = dir.join(&file);
        if flat.exists() {
            return Some(flat);
        }
    }

    let dev = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join(&file);
    if dev.exists() {
        return Some(dev);
    }

    None
}

/// 实际播放逻辑：仅接受 attention / done，解析失败或播放器缺失时静默。
fn play(name: &str, app: &AppHandle) {
    if name != "attention" && name != "done" {
        return;
    }

    let Some(path) = resolve_sound_path(app, name) else {
        eprintln!("[audio] 未找到音效资源: {name}.m4a");
        return;
    };

    #[cfg(target_os = "macos")]
    {
        if let Err(e) = std::process::Command::new("afplay").arg(&path).spawn() {
            eprintln!("[audio] afplay 启动失败: {e}");
        }
    }

    #[cfg(target_os = "windows")]
    {
        // TODO: Windows 无 afplay；.m4a(AAC) 无法由 System.Media.SoundPlayer(仅 WAV) 播放。
        // 当前以系统默认播放器打开作为占位实现；后续可换 rodio/symphonia 或 Media Foundation 直解。
        let script = format!("Start-Process -FilePath '{}'", path.display());
        if let Err(e) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(script)
            .spawn()
        {
            eprintln!("[audio] powershell 启动失败: {e}");
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = &path; // TODO: Linux 可用 paplay/aplay（需先转 wav 或换解码方案）
    }
}

/// 播放提示音，供前端 `invoke('play_sound', { name: 'done' })` 与托盘菜单调用。
#[tauri::command]
pub fn play_sound(app: tauri::AppHandle, name: String) {
    play(&name, &app);
}

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

/// 解析音效文件路径：优先打包资源目录，开发态回退到 `CARGO_MANIFEST_DIR/resources/`。
fn resolve_sound_path(app: &AppHandle, name: &str) -> Option<PathBuf> {
    let file = format!("{name}.wav");

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
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
        use windows::core::PCWSTR;
        // PlaySoundW 直接后台播放 WAV，不弹任何窗口
        let wide: Vec<u16> = path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            let _ = PlaySoundW(
                PCWSTR(wide.as_ptr()),
                None,
                SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = &path; // TODO: Linux 可用 paplay/aplay（需先转 wav 或换解码方案）
    }
}

/// 自动音效节流：同一种音效 10 秒内只播一次，避免状态反复触发刷屏。
static LAST_SOUND_MS: AtomicI64 = AtomicI64::new(0);
const SOUND_COOLDOWN_MS: i64 = 10_000;

/// 状态变化时的自动提示音：done -> 完成音，attention -> 审批音；带 10s 冷却。
pub fn notify_state_change(app: &AppHandle, state: &str) {
    if !crate::tray::is_sound_enabled() {
        return;
    }
    let name = match state {
        "done" => "done",
        "attention" => "attention",
        _ => return,
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let last = LAST_SOUND_MS.load(Ordering::Relaxed);
    if now - last < SOUND_COOLDOWN_MS {
        return;
    }
    LAST_SOUND_MS.store(now, Ordering::Relaxed);
    play(name, app);
}

/// 播放提示音，供前端 `invoke('play_sound', { name: 'done' })` 与托盘菜单调用。
#[tauri::command]
pub fn play_sound(app: tauri::AppHandle, name: String) {
    play(&name, &app);
}

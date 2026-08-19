//! 开机自启：Windows 注册表 HKCU\Software\Microsoft\Windows\CurrentVersion\Run。
//! 写入当前可执行文件路径，通过 reg.exe 命令行实现（避免额外依赖）。

use std::process::Command;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "dsh-pet";

/// 当前是否已注册开机自启。
pub fn is_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        Command::new("reg")
            .args(["query", RUN_KEY, "/v", VALUE_NAME])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// 设置/取消开机自启。
pub fn set_enabled(enabled: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            let exe = std::env::current_exe().unwrap_or_default();
            let path = exe.to_string_lossy().to_string();
            Command::new("reg")
                .args([
                    "add", RUN_KEY, "/v", VALUE_NAME, "/t", "REG_SZ", "/d", &path, "/f",
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            Command::new("reg")
                .args(["delete", RUN_KEY, "/v", VALUE_NAME, "/f"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        false
    }
}

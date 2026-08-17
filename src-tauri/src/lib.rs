mod audio;
mod commands;
mod config;
mod discover;
mod engine;
mod hit_test;
mod rpc;
mod sse;
mod tray;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 已有实例运行时，聚焦主窗口
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![audio::play_sound, commands::get_state, commands::move_window,
            commands::set_hit_region, commands::frontend_log, commands::get_window_position, commands::move_window_by,
            commands::drag_start, commands::drag_move, commands::drag_end, commands::get_pet_size, commands::get_easter_egg_enabled])
        .setup(|app| {
            // 初始化日志，使 engine/sse 里的 log::info!/warn! 生效
            let _ = env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or("info"),
            )
            .try_init();

            // 不再把窗口设置为全屏 overlay，避免透明置顶窗口劫持整个屏幕的鼠标操作。
            // 仅把窗口定位到主显示器工作区右下角（桌宠习惯位置），并启动“光标穿透”轮询：
            // 仅当全局光标落在鲸鱼娘命中区内时才接收鼠标，其余区域整窗穿透。
            if let Some(window) = app.get_webview_window("main") {
                // 位置记忆：优先恢复上次位置，否则默认右下角
                let cfg = config::load(app.handle());
                let mut restored = false;
                if cfg.remember_position {
                    if let (Some(x), Some(y)) = (cfg.window_x, cfg.window_y) {
                        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                        restored = true;
                    }
                }
                if !restored {
                    if let Some(monitor) = window
                    .primary_monitor()
                    .ok()
                    .flatten()
                    .or(window.current_monitor().ok().flatten())
                {
                    let scale = monitor.scale_factor();
                    let wa = monitor.work_area();
                    let wa_pos = wa.position.to_logical::<f64>(scale);
                    let wa_size = wa.size.to_logical::<f64>(scale);
                    let size = window
                        .inner_size()
                        .unwrap_or(tauri::PhysicalSize::new(420, 460))
                        .to_logical::<f64>(scale);
                    let x = (wa_pos.x + wa_size.width - size.width - 24.0).max(0.0);
                    let y = (wa_pos.y + wa_size.height - size.height - 24.0).max(0.0);
                    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                    }
                }

                // 关闭窗口 = 隐藏到托盘（不退出）；真正退出请用托盘菜单“退出”
                let close_window = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = close_window.hide();
                    }
                });
            }
            hit_test::start_cursor_through(app.handle().clone());

            // macOS：隐藏 Dock 图标（桌面宠物的辅助型激活策略）
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // 创建系统托盘与菜单
            tray::create_tray(app)?;

            // DSH 状态引擎：共享状态 + SSE/轮询引擎
            let state: Arc<Mutex<engine::EngineState>> =
                Arc::new(Mutex::new(engine::EngineState::default()));
            app.manage(state.clone());

            let client = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(10))
                .build()
                .expect("构建 reqwest 客户端失败");
            let handle = app.handle().clone();
            let sound_handle = handle.clone();
            // 记录上次状态：只在状态变化时触发音效，避免 done/attention 持续期间反复播放
            let last_sound_state: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
            let sink: engine::SnapshotSink = Arc::new(move |snapshot| {
                if let Err(e) = handle.emit("dsh-state", &snapshot) {
                    log::warn!("[pet] emit dsh-state 失败: {e}");
                }
                let mut last = last_sound_state.lock().unwrap();
                if *last != snapshot.state {
                    *last = snapshot.state.clone();
                    audio::notify_state_change(&sound_handle, &snapshot.state);
                }
            });

            tauri::async_runtime::spawn(async move {
                engine::run(state, client, sink).await;
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // 点击穿透由 set_ignore_cursor_events 控制，进程退出后系统自动清理，无需额外处理
        });
}

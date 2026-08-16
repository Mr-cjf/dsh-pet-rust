mod audio;
mod commands;
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
            commands::set_hit_region])
        .setup(|app| {
            // 初始化日志，使 engine/sse 里的 log::info!/warn! 生效
            let _ = env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or("info"),
            )
            .try_init();

            // 主窗口全屏透明 overlay：覆盖主显示器工作区（排除任务栏等系统保留区域）
            if let Some(window) = app.get_webview_window("main") {
                // 优先系统主显示器，回退到窗口当前所在显示器
                let monitor = window
                    .primary_monitor()?
                    .or(window.current_monitor()?);
                if let Some(monitor) = monitor {
                    let scale_factor = monitor.scale_factor();
                    // work_area 为物理像素（相对虚拟桌面的矩形，已排除任务栏）
                    let work_area = monitor.work_area();
                    // 物理 -> 逻辑：除以 scale_factor，得到 CSS 像素（与前端 e.screenX/Y 一致）
                    let logical_size = work_area.size.to_logical::<f64>(scale_factor);
                    let logical_position = work_area.position.to_logical::<f64>(scale_factor);
                    log::info!(
                        "[pet] 全屏 overlay：monitor={:?} scale_factor={:.2} work_area物理={:?} -> 逻辑尺寸={:.1}x{:.1} 逻辑位置=({:.1}, {:.1})",
                        monitor.name(),
                        scale_factor,
                        work_area,
                        logical_size.width,
                        logical_size.height,
                        logical_position.x,
                        logical_position.y
                    );
                    window.set_size(logical_size)?;
                    window.set_position(logical_position)?;
                } else {
                    log::warn!("[pet] 未找到可用显示器，跳过全屏 overlay 尺寸设置");
                }

                // 命中测试子类化：矩形内接收鼠标事件，矩形外穿透（Windows WM_NCHITTEST）
                if let Err(e) = hit_test::init_hit_test(&window) {
                    log::warn!("[pet] 命中测试初始化失败: {e}");
                }
            }

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
            let sink: engine::SnapshotSink = Arc::new(move |snapshot| {
                if let Err(e) = handle.emit("dsh-state", &snapshot) {
                    log::warn!("[pet] emit dsh-state 失败: {e}");
                }
            });

            tauri::async_runtime::spawn(async move {
                engine::run(state, client, sink).await;
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // 退出时恢复窗口过程，避免子类化残留
            if let tauri::RunEvent::Exit = event {
                #[cfg(target_os = "windows")]
                hit_test::cleanup_hit_test();
            }
        });
}

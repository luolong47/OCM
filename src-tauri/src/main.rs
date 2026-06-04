#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::{Manager, menu::{Menu, MenuItem}, tray::TrayIconBuilder};
use tauri_plugin_opener::OpenerExt;
use tracing_subscriber::EnvFilter;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("ocm_backend=debug,info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![]),
        ))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let config = Arc::new(ocm_backend::config::Config::from_env());
            let frontend_url = config.frontend_url.clone();
            
            // 保存配置到应用状态
            app.manage(AppConfig {
                frontend_url: frontend_url.clone(),
            });

            let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel(1);

            // 启动 HTTP 服务器
            tauri::async_runtime::spawn(async move {
                let server_config = ocm_backend::ServerConfig {
                    config,
                    shutdown_rx,
                };

                if let Err(e) = ocm_backend::run_server(server_config).await {
                    tracing::error!("Server error: {}", e);
                }
            });

            // 保存 shutdown 句柄
            app.manage(ShutdownHandle { tx: shutdown_tx });

            // 创建托盘菜单
            let open = MenuItem::with_id(app, "open", "打开页面", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;

            // 创建托盘图标
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "open" => {
                            let config = app.state::<AppConfig>();
                            let url = config.frontend_url.clone();
                            if let Err(e) = app.opener().open_url(&url, None::<&str>) {
                                tracing::error!("Failed to open browser: {}", e);
                            }
                        }
                        "quit" => {
                            let handle = app.state::<ShutdownHandle>();
                            let _ = handle.tx.try_send(());
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            tracing::info!("OCM Tauri application started");
            tracing::info!("Frontend URL: {}", frontend_url);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[derive(Clone)]
struct AppConfig {
    frontend_url: String,
}

struct ShutdownHandle {
    tx: tokio::sync::mpsc::Sender<()>,
}

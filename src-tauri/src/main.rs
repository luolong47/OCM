#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::{CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu};
use tracing_subscriber::EnvFilter;

fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("ocm_backend=debug,info")),
        )
        .init();

    // 系统托盘菜单
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("open", "打开页面"))
        .add_item(CustomMenuItem::new("quit", "退出"));

    let tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--flag1", "--flag2"]),
        ))
        .system_tray(tray)
        .on_system_tray_event(|app, event| {
            if let SystemTrayEvent::MenuItemClick { id, .. } = event {
                match id.as_str() {
                    "open" => {
                        let config = app.state::<AppConfig>();
                        let url = config.frontend_url.clone();
                        if let Err(e) = tauri::api::shell::open(&app.shell_scope(), &url, None) {
                            tracing::error!("Failed to open browser: {}", e);
                        }
                    }
                    "quit" => {
                        // 尝试优雅关闭 HTTP 服务器
                        let handle = app.state::<ShutdownHandle>();
                        let _ = handle.tx.try_send(());
                        app.exit(0);
                    }
                    _ => {}
                }
            }
        })
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

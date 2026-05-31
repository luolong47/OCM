//! OCM backend entrypoint.

mod autostart;
mod cache;
mod clients;
mod config;
mod db;
mod domain;
mod error;
mod routes;
mod services;
mod state;
mod tray;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("ocm_backend=debug,tower_http=info,info")),
        )
        .init();

    let config = Arc::new(config::Config::from_env());
    ensure_sqlite_parent_dir(&config.database_url);

    let db = db::connect(&config.database_url).await?;
    let http = reqwest::Client::builder()
        .user_agent(concat!("ocm-backend/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(30))
        .build()?;
    let catalog = cache::Catalog::new(http, config.clone());

    let state = state::AppState {
        db,
        config: config.clone(),
        catalog,
    };

    let app = routes::router(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::very_permissive());

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("OCM backend listening on http://{}", config.bind);
    tracing::info!("OCM frontend URL: {}", config.frontend_url);
    tracing::info!(
        "opencode.json target: {}",
        config.opencode_config_path.display()
    );

    let (tray_tx, mut tray_rx) = mpsc::unbounded_channel();
    let tray_handle = match tray::spawn(tray_tx).await {
        Ok(handle) => {
            tracing::info!("OCM system tray started");
            Some(handle)
        }
        Err(err) => {
            tracing::warn!(%err, "OCM system tray unavailable; backend continues without tray");
            None
        }
    };

    let frontend_url = config.frontend_url.clone();
    let shutdown = async move {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("shutdown requested by Ctrl+C");
                    break;
                }
                Some(command) = tray_rx.recv() => {
                    match command {
                        tray::TrayCommand::OpenPage => tray::open_page(&frontend_url),
                        tray::TrayCommand::Exit => {
                            tracing::info!("shutdown requested from system tray");
                            break;
                        }
                    }
                }
            }
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    if let Some(handle) = tray_handle {
        handle.shutdown().await;
    }
    Ok(())
}

/// `sqlite:...?mode=rwc` creates the db file but not its parent directory.
fn ensure_sqlite_parent_dir(database_url: &str) {
    let Some(rest) = database_url.strip_prefix("sqlite:") else {
        return;
    };
    let rest = rest.trim_start_matches("//");
    let path = rest.split('?').next().unwrap_or(rest);
    if path.is_empty() || path == ":memory:" {
        return;
    }
    if let Some(parent) = std::path::Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}

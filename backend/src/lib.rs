//! OCM backend library — exposes modules for use in Tauri application.

pub mod autostart;
pub mod cache;
pub mod clients;
pub mod config;
pub mod db;
pub mod domain;
pub mod embedded;
pub mod error;
pub mod routes;
pub mod services;
pub mod state;

use std::sync::Arc;
use std::time::Duration;

use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub struct ServerConfig {
    pub config: Arc<config::Config>,
    pub shutdown_rx: tokio::sync::mpsc::Receiver<()>,
}

/// Start the Axum HTTP server with the given configuration.
pub async fn run_server(mut server_config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let config = server_config.config;
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

    let shutdown = async move {
        let _ = server_config.shutdown_rx.recv().await;
        tracing::info!("shutdown requested");
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

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

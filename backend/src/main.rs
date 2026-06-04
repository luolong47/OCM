//! OCM backend entrypoint for standalone mode.

use std::sync::Arc;
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

    let config = Arc::new(ocm_backend::config::Config::from_env());
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel(1);

    let server_config = ocm_backend::ServerConfig {
        config,
        shutdown_rx,
    };

    tokio::select! {
        result = ocm_backend::run_server(server_config) => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Ctrl+C received, shutting down");
            let _ = shutdown_tx.send(()).await;
        }
    }

    Ok(())
}

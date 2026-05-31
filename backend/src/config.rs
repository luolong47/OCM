//! Runtime configuration, loaded from environment with sane defaults.

use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub frontend_url: String,
    pub models_dev_url: String,
    pub models_dev_ttl: Duration,
    pub provider_list_ttl: Duration,
    pub opencode_config_path: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env_or("DATABASE_URL", "sqlite:data/ocm.db?mode=rwc");
        let bind = env_or("OCM_BIND", "127.0.0.1:8787");
        let frontend_url = env_or("OCM_FRONTEND_URL", "http://localhost:5174/");
        let models_dev_url = env_or("OCM_MODELS_DEV_URL", "https://models.dev/api.json");
        let models_dev_ttl = Duration::from_secs(env_u64("OCM_MODELS_DEV_TTL_SECS", 86_400));
        let provider_list_ttl = Duration::from_secs(env_u64("OCM_PROVIDER_LIST_TTL_SECS", 300));
        let opencode_config_path = std::env::var("OCM_OPENCODE_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_opencode_path());

        Config {
            database_url,
            bind,
            frontend_url,
            models_dev_url,
            models_dev_ttl,
            provider_list_ttl,
            opencode_config_path,
        }
    }
}

fn default_opencode_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("opencode")
        .join("opencode.json")
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

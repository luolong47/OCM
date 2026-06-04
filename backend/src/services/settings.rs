use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettingsFile {
    #[serde(default)]
    pub nutstore: NutstoreSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NutstoreSettings {
    #[serde(default = "default_server_url")]
    pub server_url: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub remote_dir: String,
}

impl Default for NutstoreSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: default_server_url(),
            username: String::new(),
            password: String::new(),
            remote_dir: String::new(),
        }
    }
}

fn default_server_url() -> String {
    "https://dav.jianguoyun.com/dav/".to_string()
}

pub fn load(state: &AppState) -> Result<SettingsFile, AppError> {
    let path = &state.config.settings_path;
    if !path.exists() {
        return Ok(SettingsFile::default());
    }

    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("读取 {} 失败: {e}", path.display())))?;
    if text.trim().is_empty() {
        return Ok(SettingsFile::default());
    }

    serde_json::from_str(&text)
        .map_err(|e| AppError::Internal(format!("解析 {} 失败: {e}", path.display())))
}

pub fn save(state: &AppState, settings: &SettingsFile) -> Result<(), AppError> {
    let path = &state.config.settings_path;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("创建 {} 失败: {e}", parent.display())))?;
    }

    let text = format!("{}\n", serde_json::to_string_pretty(settings)?);
    std::fs::write(path, text)
        .map_err(|e| AppError::Internal(format!("写入 {} 失败: {e}", path.display())))
}

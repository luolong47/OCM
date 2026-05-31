//! Import configuration service: parses the existing opencode.json and imports
//! its provider and model configuration into OCM's SQLite database.

use serde::Serialize;
use serde_json::Value;

use crate::db;
use crate::error::AppError;
use crate::services::selection::{self, SelectedUpdate};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub providers_imported: usize,
    pub models_imported: usize,
}

pub async fn import_from_config(state: &AppState) -> Result<ImportReport, AppError> {
    let path = &state.config.opencode_config_path;
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "配置路径 {} 不存在",
            path.display()
        )));
    }

    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::Internal(format!("读取 {} 失败: {e}", path.display())))?;

    if text.trim().is_empty() {
        return Ok(ImportReport {
            providers_imported: 0,
            models_imported: 0,
        });
    }

    let root: Value = serde_json::from_str(&text).map_err(|e| {
        AppError::BadRequest(format!("配置文件 {} 不是有效的 JSON ({e})", path.display()))
    })?;

    let root_obj = root
        .as_object()
        .ok_or_else(|| AppError::BadRequest("opencode.json 根节点不是一个 JSON 对象".into()))?;

    let provider_map = match root_obj.get("provider") {
        Some(p) => p
            .as_object()
            .ok_or_else(|| AppError::BadRequest("`provider` 部分不是一个 JSON 对象".into()))?,
        None => {
            return Ok(ImportReport {
                providers_imported: 0,
                models_imported: 0,
            })
        }
    };

    let mut providers_imported = 0;
    let mut models_imported = 0;

    for (provider_id, val) in provider_map {
        let p_obj = val.as_object().ok_or_else(|| {
            AppError::BadRequest(format!("服务商 '{provider_id}' 的配置不是一个 JSON 对象"))
        })?;

        let name = p_obj
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(provider_id)
            .to_string();

        let npm = p_obj
            .get("npm")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                if let Some(api) = p_obj.get("api").and_then(|v| v.as_str()) {
                    match api {
                        "anthropic" => "@ai-sdk/anthropic".to_string(),
                        "openai" => "@ai-sdk/openai-compatible".to_string(),
                        "google" => "@ai-sdk/google".to_string(),
                        "groq" => "@ai-sdk/groq".to_string(),
                        "mistral" => "@ai-sdk/mistral".to_string(),
                        "ollama" => "@ai-sdk/ollama".to_string(),
                        _ => "@ai-sdk/openai-compatible".to_string(),
                    }
                } else {
                    "@ai-sdk/openai-compatible".to_string()
                }
            });

        let mut base_url = None;
        let mut api_key_env = None;
        let mut api_key = None;
        let mut headers = None;
        let mut extra_options = serde_json::Map::new();

        if let Some(options_val) = p_obj.get("options") {
            if let Some(options_obj) = options_val.as_object() {
                for (k, v) in options_obj {
                    match k.as_str() {
                        "baseURL" => {
                            if let Some(s) = v.as_str() {
                                base_url = Some(s.to_string());
                            }
                        }
                        "apiKey" => {
                            if let Some(key_str) = v.as_str() {
                                if key_str.starts_with("{env:") && key_str.ends_with('}') {
                                    api_key_env = Some(key_str[5..key_str.len() - 1].to_string());
                                } else {
                                    api_key = Some(key_str.to_string());
                                }
                            }
                        }
                        "headers" => {
                            headers = Some(v.clone());
                        }
                        _ => {
                            extra_options.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
        }

        let options_json = if extra_options.is_empty() {
            None
        } else {
            Some(Value::Object(extra_options))
        };

        let input = db::ProviderInput {
            id: provider_id.clone(),
            name,
            npm,
            base_url,
            api_key_env,
            api_key,
            models_dev_key: None,
            headers,
            options: options_json,
            enabled: true,
        };

        // Check if provider already exists in DB
        let exists = db::get_provider(&state.db, provider_id).await?.is_some();
        if exists {
            db::update_provider(&state.db, provider_id, &input).await?;
        } else {
            db::insert_provider(&state.db, &input).await?;
        }

        // Set as applied
        db::set_applied(&state.db, provider_id, true).await?;
        providers_imported += 1;

        // Process models if any
        if let Some(models_val) = p_obj.get("models") {
            if let Some(models_obj) = models_val.as_object() {
                for (model_id, model_val) in models_obj {
                    // Select model
                    selection::select(state, provider_id, &[model_id.clone()]).await?;

                    let mut display_name = None;
                    let mut override_patch = serde_json::Map::new();

                    if let Some(m_obj) = model_val.as_object() {
                        for (k, v) in m_obj {
                            if k == "name" {
                                if let Some(s) = v.as_str() {
                                    display_name = Some(s.to_string());
                                }
                            } else {
                                override_patch.insert(k.clone(), v.clone());
                            }
                        }
                    }

                    let patch_json = if override_patch.is_empty() {
                        None
                    } else {
                        Some(Value::Object(override_patch))
                    };

                    selection::update_selected(
                        &state.db,
                        provider_id,
                        model_id,
                        &SelectedUpdate {
                            display_name,
                            is_enabled: Some(true),
                            override_patch: patch_json,
                        },
                    )
                    .await?;

                    models_imported += 1;
                }
            }
        }
    }

    Ok(ImportReport {
        providers_imported,
        models_imported,
    })
}

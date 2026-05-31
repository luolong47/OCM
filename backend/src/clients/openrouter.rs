//! OpenRouter API models client.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelArchitecture {
    #[serde(default)]
    pub input_modalities: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModelPricing {
    pub prompt: String,
    pub completion: String,
    pub input_cache_read: Option<String>,
    pub input_cache_write: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenRouterModel {
    pub id: String,
    pub name: String,
    pub context_length: u64,
    pub architecture: OpenRouterModelArchitecture,
    pub pricing: OpenRouterModelPricing,
    #[serde(default)]
    pub supported_parameters: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    pub data: Vec<OpenRouterModel>,
}

pub async fn fetch_openrouter_models(
    http: &reqwest::Client,
) -> Result<Vec<OpenRouterModel>, crate::error::AppError> {
    let url = "https://openrouter.ai/api/v1/models";
    let resp = http.get(url).send().await.map_err(|e| {
        crate::error::AppError::Upstream(format!("OpenRouter models fetch failed: {e}"))
    })?;
    if !resp.status().is_success() {
        return Err(crate::error::AppError::Upstream(format!(
            "OpenRouter models returned HTTP {}",
            resp.status()
        )));
    }
    let parsed: OpenRouterResponse = resp.json().await.map_err(|e| {
        crate::error::AppError::Upstream(format!("OpenRouter models parse failed: {e}"))
    })?;
    Ok(parsed.data)
}

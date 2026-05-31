//! Provider client.
//!
//! Calls a provider's OpenAI-compatible `GET {base_url}/models`, which returns only
//! the ids the key can actually access (no rich metadata — that comes from models.dev).
//! Handles both `{ "data": [{ "id": ... }] }` and a bare `[{ "id": ... }]` shape.

use crate::error::AppError;

pub async fn fetch_model_ids(
    http: &reqwest::Client,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>, AppError> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let mut req = http.get(&url);
    if let Some(key) = api_key {
        req = req.bearer_auth(key);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| AppError::Upstream(format!("GET {url}: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::Upstream(format!(
            "{} returned {}",
            url,
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Upstream(format!("parse {url}: {e}")))?;

    let array = body
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| body.as_array())
        .ok_or_else(|| AppError::Upstream(format!("{url}: unexpected /models shape")))?;

    let ids = array
        .iter()
        .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(str::to_string))
        .collect();
    Ok(ids)
}

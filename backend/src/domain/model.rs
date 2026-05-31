//! Model schema.
//!
//! `ModelEntry` mirrors a single entry under `provider.models` in both the
//! models.dev dataset and opencode's own config schema — deliberately the *same*
//! shape so metadata flows source → snapshot → opencode.json with zero field
//! translation. Every field is optional so a sparse provider-only id still parses.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Modalities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Limit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cost {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_audio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_audio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_over_200k: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiers: Option<serde_json::Value>,
}

/// A single model entry. Same shape as a models.dev / opencode `models[id]` object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachment: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_weights: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_output: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interleaved: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<serde_json::Value>,
    #[serde(default)]
    pub modalities: Modalities,
    #[serde(default)]
    pub limit: Limit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
}

impl ModelEntry {
    /// A minimal entry for an id with no models.dev metadata.
    pub fn bare(id: impl Into<String>) -> Self {
        ModelEntry {
            id: id.into(),
            ..Default::default()
        }
    }

    pub fn supports(&self, modality: &str) -> bool {
        self.modalities.input.iter().any(|m| m == modality)
    }

    pub fn context(&self) -> Option<u64> {
        self.limit.context
    }
}

/// Per-row metadata layered on top of a `ModelEntry` for the catalog listing.
#[derive(Debug, Clone, Serialize)]
pub struct ModelMeta {
    pub is_selected: bool,
    pub has_custom_config: bool,
    /// false when the id came only from the provider and was not found in models.dev.
    pub metadata_known: bool,
    pub source: &'static str,
}

/// A catalog row: the model entry, flattened, plus its OCM metadata.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogModel {
    #[serde(flatten)]
    pub entry: ModelEntry,
    #[serde(rename = "_meta")]
    pub meta: ModelMeta,
}

/// Apply an RFC 7386 JSON merge-patch of `patch` onto `target`, in place.
pub fn merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    use serde_json::Value;
    match patch {
        Value::Object(patch_map) => {
            if !target.is_object() {
                *target = Value::Object(serde_json::Map::new());
            }
            let target_map = target.as_object_mut().expect("ensured object above");
            for (key, patch_val) in patch_map {
                if patch_val.is_null() {
                    target_map.remove(key);
                } else {
                    merge_patch(
                        target_map.entry(key.clone()).or_insert(Value::Null),
                        patch_val,
                    );
                }
            }
        }
        _ => *target = patch.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_patch_overrides_and_deletes() {
        let mut base = serde_json::json!({
            "name": "old", "limit": { "context": 128000, "output": 8192 }
        });
        let patch = serde_json::json!({
            "name": "new", "limit": { "output": null }, "tool_call": true
        });
        merge_patch(&mut base, &patch);
        assert_eq!(base["name"], "new");
        assert_eq!(base["limit"]["context"], 128000);
        assert!(base["limit"].get("output").is_none());
        assert_eq!(base["tool_call"], true);
    }

    #[test]
    fn parses_sparse_modelsdev_entry() {
        let raw = serde_json::json!({
            "id": "solar-pro3", "name": "solar-pro3", "reasoning": true,
            "tool_call": true, "modalities": { "input": ["text"], "output": ["text"] },
            "limit": { "context": 131072, "output": 8192 },
            "cost": { "input": 0.25, "output": 0.25 }
        });
        let m: ModelEntry = serde_json::from_value(raw).unwrap();
        assert_eq!(m.context(), Some(131072));
        assert_eq!(m.reasoning, Some(true));
        assert!(!m.supports("image"));
    }
}

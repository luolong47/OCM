//! Apply service: turn the selected models + overrides into opencode.json provider
//! blocks and merge them into the user's config, preserving everything else.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use crate::db::{self, ProviderRow};
use crate::error::AppError;
use crate::services::selection::{self, SelectedRow};
use crate::state::AppState;

/// Resolve which config file to operate on.
/// Prefers `<stem>.jsonc` if it already exists next to the configured path;
/// falls back to the configured path (typically `opencode.json`) otherwise.
fn resolve_config_path(configured: &Path) -> PathBuf {
    let jsonc = configured.with_extension("jsonc");
    if jsonc.exists() {
        jsonc
    } else {
        configured.to_path_buf()
    }
}

/// Strip `// line` and `/* block */` comments from a JSONC string so that
/// `serde_json` can parse it as plain JSON.
fn strip_jsonc_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(ch) = chars.next() {
        if escape_next {
            out.push(ch);
            escape_next = false;
            continue;
        }
        if in_string {
            if ch == '\\' {
                escape_next = true;
                out.push(ch);
            } else {
                if ch == '"' {
                    in_string = false;
                }
                out.push(ch);
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                out.push(ch);
            }
            '/' if chars.peek() == Some(&'/') => {
                // Line comment — skip until newline.
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                // Block comment — skip until `*/`.
                chars.next(); // consume '*'
                let mut prev = ' ';
                for c in chars.by_ref() {
                    if prev == '*' && c == '/' {
                        break;
                    }
                    if c == '\n' {
                        out.push('\n');
                    } // preserve line numbers
                    prev = c;
                }
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Fields present in models.dev / our snapshot that are not copied verbatim into
/// opencode's per-model schema. `name` is normalized to the model id below.
const NON_SCHEMA_FIELDS: &[&str] = &[
    "id",
    "knowledge",
    "last_updated",
    "open_weights",
    "structured_output",
    "experimental",
    "interleaved",
    "provider",
    "name",
    "_meta",
];
const NON_SCHEMA_COST_FIELDS: &[&str] = &[
    "context_over_200k",
    "tiers",
    "input_audio",
    "output_audio",
    "reasoning",
];
const DEFAULT_CONTEXT_LIMIT: i64 = 128_000;
const DEFAULT_OUTPUT_LIMIT: i64 = 4096;

#[derive(Debug, Serialize)]
pub struct ApplyReport {
    pub path: String,
    pub providers_written: Vec<String>,
    pub models_written: usize,
    pub backup: Option<String>,
}

fn opencode_model_object(row: &SelectedRow) -> Value {
    let mut eff = row.effective();
    if let Some(obj) = eff.as_object_mut() {
        for key in NON_SCHEMA_FIELDS {
            obj.remove(*key);
        }
        obj.insert("name".into(), Value::String(row.model_id.clone()));
        if let Some(cost) = obj.get_mut("cost").and_then(|v| v.as_object_mut()) {
            for key in NON_SCHEMA_COST_FIELDS {
                cost.remove(*key);
            }
        }
        ensure_required_limit(obj, row.context);
    }
    prune_empty_values(&mut eff);
    eff
}

fn ensure_required_limit(obj: &mut serde_json::Map<String, Value>, context_hint: Option<i64>) {
    let limit = obj.entry("limit").or_insert_with(|| json!({}));
    let Some(limit_obj) = limit.as_object_mut() else {
        *limit = json!({
            "context": context_hint.unwrap_or(DEFAULT_CONTEXT_LIMIT),
            "output": DEFAULT_OUTPUT_LIMIT,
        });
        return;
    };
    if !limit_obj.get("context").is_some_and(Value::is_number) {
        let context = context_hint
            .or_else(|| limit_obj.get("input").and_then(Value::as_i64))
            .unwrap_or(DEFAULT_CONTEXT_LIMIT);
        limit_obj.insert("context".into(), Value::Number(context.into()));
    }
    if !limit_obj.get("output").is_some_and(Value::is_number) {
        limit_obj.insert("output".into(), Value::Number(DEFAULT_OUTPUT_LIMIT.into()));
    }
}

fn is_empty_json_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(items) => items.is_empty(),
        Value::Object(map) => map.is_empty(),
        _ => false,
    }
}

fn prune_empty_values(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items.iter_mut() {
                prune_empty_values(item);
            }
            items.retain(|item| !is_empty_json_value(item));
        }
        Value::Object(map) => {
            for child in map.values_mut() {
                prune_empty_values(child);
            }
            map.retain(|_, child| !is_empty_json_value(child));
        }
        _ => {}
    }
}

/// Build the `provider.<id>` block for one provider.
pub fn build_provider_block(provider: &ProviderRow, selected: &[SelectedRow]) -> (Value, usize) {
    let mut models = serde_json::Map::new();
    for row in selected.iter().filter(|r| r.is_enabled) {
        models.insert(row.model_id.clone(), opencode_model_object(row));
    }
    let model_count = models.len();

    let mut options = serde_json::Map::new();
    if let Some(base) = &provider.base_url {
        options.insert("baseURL".into(), Value::String(base.clone()));
    }
    if let Some(env) = provider.api_key_env.as_ref().filter(|e| !e.is_empty()) {
        options.insert("apiKey".into(), Value::String(format!("{{env:{env}}}")));
    } else if let Some(key) = provider.api_key.as_ref().filter(|k| !k.is_empty()) {
        options.insert("apiKey".into(), Value::String(key.clone()));
    }
    if let Some(extra) = provider
        .options
        .as_ref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
    {
        if let Some(map) = extra.as_object() {
            for (k, v) in map {
                options.insert(k.clone(), v.clone());
            }
        }
    }
    if let Some(headers) = provider
        .headers
        .as_ref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
    {
        options.insert("headers".into(), headers);
    }

    let npm_lower = provider.npm.to_lowercase();
    let api_type = if npm_lower.contains("openai") {
        "openai"
    } else if npm_lower.contains("anthropic") {
        "anthropic"
    } else if npm_lower.contains("google") {
        "google"
    } else if npm_lower.contains("groq") {
        "groq"
    } else if npm_lower.contains("mistral") {
        "mistral"
    } else if npm_lower.contains("ollama") {
        "ollama"
    } else {
        "custom"
    };

    let mut block = serde_json::Map::new();
    block.insert("id".into(), Value::String(provider.id.clone()));
    block.insert("name".into(), Value::String(provider.name.clone()));
    block.insert("api".into(), Value::String(api_type.to_string()));
    block.insert("npm".into(), Value::String(provider.npm.clone()));
    if let Some(env) = provider.api_key_env.as_ref().filter(|e| !e.is_empty()) {
        block.insert("env".into(), Value::Array(vec![Value::String(env.clone())]));
    }
    block.insert("options".into(), Value::Object(options));
    block.insert("models".into(), Value::Object(models));

    let mut block = Value::Object(block);
    prune_empty_values(&mut block);
    (block, model_count)
}

/// Preview the block(s) for one provider without writing to disk.
pub async fn preview(state: &AppState, provider_id: &str) -> Result<Value, AppError> {
    let provider = db::get_provider(&state.db, provider_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("provider '{provider_id}' not found")))?;
    let selected = selection::list_selected(&state.db, provider_id).await?;
    let (block, _) = build_provider_block(&provider, &selected);
    Ok(json!({ "provider": { provider.id: block } }))
}

/// Merge OCM-managed providers into opencode.json. `only` limits to specific ids.
pub async fn apply(state: &AppState, only: Option<&[String]>) -> Result<ApplyReport, AppError> {
    let path = resolve_config_path(&state.config.opencode_config_path);

    // Read existing config (tolerate missing/empty), refuse to clobber invalid JSON/JSONC.
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
        if text.trim().is_empty() {
            json!({})
        } else {
            let stripped = strip_jsonc_comments(&text);
            serde_json::from_str(&stripped).map_err(|e| {
                AppError::BadRequest(format!(
                    "existing {} is not valid JSON/JSONC ({e}); refusing to overwrite",
                    path.display()
                ))
            })?
        }
    } else {
        json!({})
    };
    if !root.is_object() {
        return Err(AppError::BadRequest(
            "opencode.json root is not a JSON object".into(),
        ));
    }

    // Gather blocks (all awaits happen here, before any borrow of `root`).
    let providers = db::list_providers(&state.db).await?;
    let mut blocks: Vec<(String, Value)> = Vec::new();
    let mut models_written = 0usize;
    for p in providers.iter().filter(|p| p.enabled && p.source == "ocm") {
        if let Some(ids) = only {
            if !ids.iter().any(|x| x == &p.id) {
                continue;
            }
        }
        let selected = selection::list_selected(&state.db, &p.id).await?;
        let (block, count) = build_provider_block(p, &selected);
        models_written += count;
        blocks.push((p.id.clone(), block));
    }

    // Synchronous merge — no awaits while `root` is borrowed.
    {
        let root_obj = root.as_object_mut().expect("checked object above");
        root_obj
            .entry("$schema")
            .or_insert_with(|| Value::String("https://opencode.ai/config.json".into()));
        let provider_map = root_obj
            .entry("provider")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| AppError::BadRequest("`provider` section is not an object".into()))?;
        for (id, block) in &blocks {
            provider_map.insert(id.clone(), block.clone());
        }
    }
    prune_empty_values(&mut root);

    // Back up existing file, then write.
    let backup = if path.exists() {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let bak_name = format!(
            "{}.{}.bak",
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("opencode"),
            ts
        );
        let bak = path.with_file_name(bak_name);
        std::fs::copy(&path, &bak)
            .map_err(|e| AppError::Internal(format!("backup failed: {e}")))?;
        Some(bak.display().to_string())
    } else {
        None
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("create {}: {e}", parent.display())))?;
    }
    let pretty = format!("{}\n", serde_json::to_string_pretty(&root)?);
    let collapsed = collapse_json_models(&pretty);
    std::fs::write(&path, collapsed)
        .map_err(|e| AppError::Internal(format!("write {}: {e}", path.display())))?;

    let written: Vec<String> = blocks.into_iter().map(|(id, _)| id).collect();
    for id in &written {
        db::set_applied(&state.db, id, true).await?;
        let _ = db::set_needs_reapply(&state.db, id, false).await;
    }

    Ok(ApplyReport {
        path: path.display().to_string(),
        providers_written: written,
        models_written,
        backup,
    })
}

/// Remove an OCM-managed provider from opencode.json and set is_applied=false.
pub async fn unapply(state: &AppState, provider_id: &str) -> Result<ApplyReport, AppError> {
    let path = resolve_config_path(&state.config.opencode_config_path);

    // Read existing config.
    let mut root: Value = if path.exists() {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| AppError::Internal(format!("read {}: {e}", path.display())))?;
        if text.trim().is_empty() {
            json!({})
        } else {
            let stripped = strip_jsonc_comments(&text);
            serde_json::from_str(&stripped).map_err(|e| {
                AppError::BadRequest(format!(
                    "existing {} is not valid JSON/JSONC ({e}); refusing to overwrite",
                    path.display()
                ))
            })?
        }
    } else {
        json!({})
    };

    let mut removed = false;
    if let Some(root_obj) = root.as_object_mut() {
        if let Some(provider_map) = root_obj.get_mut("provider").and_then(|p| p.as_object_mut()) {
            if provider_map.remove(provider_id).is_some() {
                removed = true;
            }
        }
    }

    let backup = if removed && path.exists() {
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let bak_name = format!(
            "{}.{}.bak",
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("opencode"),
            ts
        );
        let bak = path.with_file_name(bak_name);
        std::fs::copy(&path, &bak)
            .map_err(|e| AppError::Internal(format!("backup failed: {e}")))?;

        let pretty = format!("{}\n", serde_json::to_string_pretty(&root)?);
        let collapsed = collapse_json_models(&pretty);
        std::fs::write(&path, collapsed)
            .map_err(|e| AppError::Internal(format!("write {}: {e}", path.display())))?;
        Some(bak.display().to_string())
    } else {
        None
    };

    db::set_applied(&state.db, provider_id, false).await?;
    let _ = db::set_needs_reapply(&state.db, provider_id, false).await;

    Ok(ApplyReport {
        path: path.display().to_string(),
        providers_written: vec![provider_id.to_string()],
        models_written: 0,
        backup,
    })
}

fn collapse_json_models(pretty: &str) -> String {
    let mut out = Vec::new();
    let lines: Vec<&str> = pretty.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.starts_with("\"models\": {") {
            out.push(line.to_string());

            let models_indent = line.len() - trimmed.len();
            i += 1;

            while i < lines.len() {
                let inner_line = lines[i];
                let inner_trimmed = inner_line.trim();
                let inner_indent = inner_line.len() - inner_trimmed.len();

                if inner_indent == models_indent && (inner_trimmed == "}" || inner_trimmed == "},")
                {
                    out.push(inner_line.to_string());
                    i += 1;
                    break;
                }

                if inner_indent > models_indent && inner_trimmed.ends_with('{') {
                    let mut accumulated = Vec::new();
                    let model_indent_str = &inner_line[..inner_indent];

                    accumulated.push(inner_trimmed.to_string());
                    i += 1;

                    while i < lines.len() {
                        let model_line = lines[i];
                        let model_trimmed = model_line.trim();
                        let model_indent = model_line.len() - model_trimmed.len();

                        if model_indent == inner_indent
                            && (model_trimmed == "}" || model_trimmed == "},")
                        {
                            accumulated.push(model_trimmed.to_string());
                            i += 1;
                            break;
                        }

                        accumulated.push(model_trimmed.to_string());
                        i += 1;
                    }

                    let mut collapsed = String::new();
                    for (idx, part) in accumulated.iter().enumerate() {
                        if idx > 0 {
                            collapsed.push(' ');
                        }
                        collapsed.push_str(part);
                    }

                    let mut clean_collapsed = String::new();
                    let mut prev_is_space = false;
                    for c in collapsed.chars() {
                        if c == ' ' {
                            if !prev_is_space {
                                clean_collapsed.push(c);
                                prev_is_space = true;
                            }
                        } else {
                            clean_collapsed.push(c);
                            prev_is_space = false;
                        }
                    }

                    out.push(format!("{}{}", model_indent_str, clean_collapsed));
                } else {
                    out.push(inner_line.to_string());
                    i += 1;
                }
            }
        } else {
            out.push(line.to_string());
            i += 1;
        }
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_row(api_key_env: Option<&str>) -> ProviderRow {
        ProviderRow {
            id: "grok2api".into(),
            name: "grok2api".into(),
            npm: "@ai-sdk/openai".into(),
            base_url: Some("http://localhost:8000/v1".into()),
            api_key_env: api_key_env.map(str::to_string),
            api_key: None,
            models_dev_key: None,
            headers: None,
            options: None,
            enabled: true,
            is_applied: false,
            needs_reapply: false,
            source: "ocm".into(),
            created_at: "2026-05-31 00:00:00".into(),
            updated_at: "2026-05-31 00:00:00".into(),
        }
    }

    #[test]
    fn provider_block_omits_unset_env_and_filters() {
        let (block, count) = build_provider_block(&provider_row(None), &[]);
        let obj = block.as_object().unwrap();

        assert_eq!(count, 0);
        assert!(obj.get("env").is_none());
        assert!(obj.get("whitelist").is_none());
        assert!(obj.get("blacklist").is_none());
        assert_eq!(obj["options"]["baseURL"], "http://localhost:8000/v1");
    }

    #[test]
    fn provider_block_writes_env_when_api_key_env_is_set() {
        let (block, _) = build_provider_block(&provider_row(Some("GROK2API_API_KEY")), &[]);
        let obj = block.as_object().unwrap();

        assert_eq!(obj["env"], json!(["GROK2API_API_KEY"]));
        assert_eq!(obj["options"]["apiKey"], "{env:GROK2API_API_KEY}");
        assert!(obj.get("whitelist").is_none());
        assert!(obj.get("blacklist").is_none());
    }

    #[test]
    fn provider_block_writes_model_name_as_id() {
        let selected = SelectedRow {
            provider_id: "p".into(),
            model_id: "deepseek-chat".into(),
            display_name: Some("DeepSeek".into()),
            is_enabled: true,
            snapshot: json!({
                "id": "deepseek-chat",
                "name": "DeepSeek",
                "family": "deepseek",
                "limit": { "context": 128000, "output": 8192 }
            })
            .to_string(),
            metadata_known: true,
            override_patch: None,
            context: Some(128000),
            has_image: false,
            tool_call: true,
            selected_at: "2026-05-31 00:00:00".into(),
            updated_at: "2026-05-31 00:00:00".into(),
            api_snapshot_at: None,
        };

        let (block, count) = build_provider_block(&provider_row(None), &[selected]);
        let model = &block["models"]["deepseek-chat"];

        assert_eq!(count, 1);
        assert_eq!(model["name"], "deepseek-chat");
        assert_eq!(model["family"], "deepseek");
    }

    #[test]
    fn prune_empty_values_removes_empty_arrays_objects_and_nulls() {
        let mut value = json!({
            "env": [],
            "options": {
                "headers": {},
                "apiKey": "{env:KEY}"
            },
            "models": {
                "m": {
                    "limit": {},
                    "modalities": { "input": [], "output": ["text"] },
                    "reasoning": false,
                    "cost": null
                }
            }
        });

        prune_empty_values(&mut value);

        assert!(value.get("env").is_none());
        assert!(value["options"].get("headers").is_none());
        assert!(value["models"]["m"].get("limit").is_none());
        assert!(value["models"]["m"]["modalities"].get("input").is_none());
        assert!(value["models"]["m"].get("cost").is_none());
        assert_eq!(value["models"]["m"]["reasoning"], false);
        assert_eq!(value["options"]["apiKey"], "{env:KEY}");
    }

    #[test]
    fn opencode_model_object_strips_non_schema_modelsdev_extensions() {
        let row = SelectedRow {
            provider_id: "p".into(),
            model_id: "m".into(),
            display_name: None,
            is_enabled: true,
            snapshot: json!({
                "id": "m",
                "name": "M",
                "experimental": { "modes": { "fast": true } },
                "structured_output": true,
                "interleaved": { "field": "reasoning_content" },
                "provider": { "body": { "x": true } },
                "cost": {
                    "input": 1,
                    "output": 2,
                    "context_over_200k": { "input": 3 },
                    "tiers": [{ "input": 4 }],
                    "reasoning": 5
                }
            })
            .to_string(),
            metadata_known: true,
            override_patch: None,
            context: Some(1),
            has_image: false,
            tool_call: true,
            selected_at: "2026-05-31 00:00:00".into(),
            updated_at: "2026-05-31 00:00:00".into(),
            api_snapshot_at: None,
        };

        let value = opencode_model_object(&row);

        assert!(value.get("experimental").is_none());
        assert!(value.get("structured_output").is_none());
        assert!(value.get("interleaved").is_none());
        assert!(value.get("provider").is_none());
        assert_eq!(value["name"], "m");
        assert_eq!(value["cost"], json!({ "input": 1, "output": 2 }));
    }

    #[test]
    fn opencode_model_object_fills_required_limit_output() {
        let row = SelectedRow {
            provider_id: "modelscope".into(),
            model_id: "ZhipuAI/GLM-5.1".into(),
            display_name: None,
            is_enabled: true,
            snapshot: json!({
                "name": "Z.ai: GLM 5.1",
                "limit": { "context": 202752 },
                "tool_call": true
            })
            .to_string(),
            metadata_known: true,
            override_patch: None,
            context: Some(202752),
            has_image: false,
            tool_call: true,
            selected_at: "2026-05-31 00:00:00".into(),
            updated_at: "2026-05-31 00:00:00".into(),
            api_snapshot_at: None,
        };

        let value = opencode_model_object(&row);

        assert_eq!(value["limit"]["context"], 202752);
        assert_eq!(value["limit"]["output"], DEFAULT_OUTPUT_LIMIT);
    }

    #[test]
    fn test_collapse_json_models() {
        let input = r#"{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "grok2api": {
      "npm": "ocm-provider",
      "name": "grok2api",
      "options": {},
      "models": {
        "grok-build-0-1": {
          "name": "Grok Build 0.1",
          "family": "grok-build",
          "attachment": true
        },
        "other": {
          "name": "Other"
        }
      }
    }
  }
}"#;

        let expected = r#"{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "grok2api": {
      "npm": "ocm-provider",
      "name": "grok2api",
      "options": {},
      "models": {
        "grok-build-0-1": { "name": "Grok Build 0.1", "family": "grok-build", "attachment": true },
        "other": { "name": "Other" }
      }
    }
  }
}"#;

        assert_eq!(collapse_json_models(input), expected);
    }

    #[test]
    fn strip_line_comment() {
        let src = "{ \"a\": 1 // comment\n}";
        let stripped = strip_jsonc_comments(src);
        let v: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn strip_block_comment() {
        let src = "{ /* block */ \"b\": 2 }";
        let stripped = strip_jsonc_comments(src);
        let v: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["b"], 2);
    }

    #[test]
    fn preserve_url_in_string() {
        // "//" inside a string value must NOT be treated as a comment.
        let src = r#"{ "url": "https://example.com/path" }"#;
        let stripped = strip_jsonc_comments(src);
        let v: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(v["url"], "https://example.com/path");
    }
}

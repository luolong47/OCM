use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::AppError;
use crate::services::settings;
use crate::state::AppState;

const NUTSTORE_DEFAULT_URL: &str = "https://dav.jianguoyun.com/dav/";
const LATEST_BACKUP_FILE: &str = "ocm-latest.sqlite3";

#[derive(Debug, Serialize)]
pub struct NutstoreBackupReport {
    pub database_path: String,
    pub remote_archive_url: String,
    pub remote_latest_url: String,
    pub bytes_uploaded: usize,
}

#[derive(Debug, Serialize)]
pub struct NutstoreRestoreReport {
    pub remote_url: String,
    pub providers_restored: usize,
    pub selected_models_restored: usize,
}

pub async fn backup_to_nutstore(state: &AppState) -> Result<NutstoreBackupReport, AppError> {
    let config = settings::load(state)?.nutstore;
    if !config.enabled {
        return Err(AppError::BadRequest("坚果云备份未启用".into()));
    }
    if config.username.trim().is_empty() {
        return Err(AppError::BadRequest("请先填写坚果云用户名".into()));
    }
    if config.password.is_empty() {
        return Err(AppError::BadRequest("请先填写坚果云应用密码".into()));
    }

    let server_url = effective_server_url(&config.server_url);
    let database_path = sqlite_path_from_url(&state.config.database_url)?;
    let bytes = snapshot_database(&state).await?;

    let remote_archive_name = build_archive_file_name(&database_path);
    let remote_archive_url = build_remote_url(server_url, &config.remote_dir, &remote_archive_name);
    let remote_latest_url = build_remote_url(server_url, &config.remote_dir, LATEST_BACKUP_FILE);

    let client = reqwest::Client::new();
    ensure_remote_dirs(
        &client,
        server_url,
        &config.remote_dir,
        &config.username,
        &config.password,
    )
    .await?;

    upload_bytes(
        &client,
        &remote_archive_url,
        &config.username,
        &config.password,
        &bytes,
    )
    .await?;
    upload_bytes(
        &client,
        &remote_latest_url,
        &config.username,
        &config.password,
        &bytes,
    )
    .await?;

    Ok(NutstoreBackupReport {
        database_path: database_path.display().to_string(),
        remote_archive_url,
        remote_latest_url,
        bytes_uploaded: bytes.len(),
    })
}

pub async fn restore_from_nutstore(state: &AppState) -> Result<NutstoreRestoreReport, AppError> {
    let config = settings::load(state)?.nutstore;
    if !config.enabled {
        return Err(AppError::BadRequest("坚果云备份未启用".into()));
    }
    if config.username.trim().is_empty() {
        return Err(AppError::BadRequest("请先填写坚果云用户名".into()));
    }
    if config.password.is_empty() {
        return Err(AppError::BadRequest("请先填写坚果云应用密码".into()));
    }

    let server_url = effective_server_url(&config.server_url);
    let remote_url = build_remote_url(server_url, &config.remote_dir, LATEST_BACKUP_FILE);
    let client = reqwest::Client::new();
    let bytes = download_bytes(&client, &remote_url, &config.username, &config.password).await?;

    let temp_path = temp_backup_path("restore");
    std::fs::write(&temp_path, &bytes).map_err(|e| {
        AppError::Internal(format!(
            "写入临时恢复文件 {} 失败: {e}",
            temp_path.display()
        ))
    })?;

    let restore_result = restore_database(&state, &temp_path).await;
    let _ = std::fs::remove_file(&temp_path);

    let (providers_restored, selected_models_restored) = restore_result?;
    Ok(NutstoreRestoreReport {
        remote_url,
        providers_restored,
        selected_models_restored,
    })
}

async fn snapshot_database(state: &AppState) -> Result<Vec<u8>, AppError> {
    let temp_path = temp_backup_path("backup");
    let escaped = sqlite_string(&temp_path.display().to_string());
    let sql = format!("VACUUM main INTO '{escaped}'");
    sqlx::query(&sql).execute(&state.db).await?;

    let bytes = std::fs::read(&temp_path).map_err(|e| {
        AppError::Internal(format!(
            "读取临时备份文件 {} 失败: {e}",
            temp_path.display()
        ))
    })?;
    let _ = std::fs::remove_file(&temp_path);
    Ok(bytes)
}

async fn restore_database(
    state: &AppState,
    backup_path: &Path,
) -> Result<(usize, usize), AppError> {
    let mut conn = state.db.acquire().await?;
    let backup_path = sqlite_string(&backup_path.display().to_string());
    let attach_sql = format!("ATTACH DATABASE '{backup_path}' AS restore_db");
    let mut attached = false;

    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await?;

    let result = async {
        sqlx::query(&attach_sql).execute(&mut *conn).await?;
        attached = true;
        sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

        let providers_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM restore_db.sqlite_master WHERE type = 'table' AND name = 'providers'",
        )
        .fetch_one(&mut *conn)
        .await?;
        if providers_exists == 0 {
            return Err(AppError::BadRequest("远程备份不是有效的 OCM 数据库".into()));
        }

        for sql in [
            "DELETE FROM models_dev_model_modalities",
            "DELETE FROM models_dev_models",
            "DELETE FROM models_dev_providers",
            "DELETE FROM models_dev_refresh",
            "DELETE FROM provider_model_catalog_cache",
            "DELETE FROM selected_models",
            "DELETE FROM providers",
            "DELETE FROM model_library",
            "DELETE FROM sqlite_sequence WHERE name IN ('selected_models', 'model_library')",
        ] {
            sqlx::query(sql).execute(&mut *conn).await?;
        }

        for sql in [
            "INSERT INTO model_library SELECT * FROM restore_db.model_library",
            "INSERT INTO providers SELECT * FROM restore_db.providers",
            "INSERT INTO selected_models SELECT * FROM restore_db.selected_models",
            "INSERT INTO provider_model_catalog_cache SELECT * FROM restore_db.provider_model_catalog_cache",
            "INSERT INTO models_dev_refresh SELECT * FROM restore_db.models_dev_refresh",
            "INSERT INTO models_dev_providers SELECT * FROM restore_db.models_dev_providers",
            "INSERT INTO models_dev_models SELECT * FROM restore_db.models_dev_models",
            "INSERT INTO models_dev_model_modalities SELECT * FROM restore_db.models_dev_model_modalities",
            "INSERT INTO sqlite_sequence(name, seq) SELECT name, seq FROM restore_db.sqlite_sequence WHERE name IN ('selected_models', 'model_library')",
        ] {
            sqlx::query(sql).execute(&mut *conn).await?;
        }

        sqlx::query(
            "UPDATE providers SET is_applied = 0, needs_reapply = 0, updated_at = datetime('now')",
        )
        .execute(&mut *conn)
        .await?;

        let providers_restored: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM providers")
                .fetch_one(&mut *conn)
                .await?;
        let selected_models_restored: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM selected_models")
                .fetch_one(&mut *conn)
                .await?;

        sqlx::query("COMMIT").execute(&mut *conn).await?;
        sqlx::query("DETACH DATABASE restore_db")
            .execute(&mut *conn)
            .await?;
        attached = false;

        Ok((providers_restored as usize, selected_models_restored as usize))
    }
    .await;

    if result.is_err() {
        if attached {
            let _ = sqlx::query("DETACH DATABASE restore_db")
                .execute(&mut *conn)
                .await;
        }
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    }
    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await;

    result
}

async fn upload_bytes(
    client: &reqwest::Client,
    remote_url: &str,
    username: &str,
    password: &str,
    bytes: &[u8],
) -> Result<(), AppError> {
    let response = client
        .put(remote_url)
        .basic_auth(username, Some(password))
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(bytes.to_vec())
        .send()
        .await
        .map_err(|e| AppError::Upstream(format!("上传到坚果云失败: {e}")))?;

    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Err(AppError::Upstream(format!(
        "坚果云返回错误 {}: {}",
        status,
        trim_error_body(&body)
    )))
}

async fn download_bytes(
    client: &reqwest::Client,
    remote_url: &str,
    username: &str,
    password: &str,
) -> Result<Vec<u8>, AppError> {
    let response = client
        .get(remote_url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|e| AppError::Upstream(format!("下载坚果云备份失败: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Upstream(format!(
            "下载坚果云备份失败 {}: {}",
            status,
            trim_error_body(&body)
        )));
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|e| AppError::Upstream(format!("读取坚果云备份内容失败: {e}")))
}

async fn ensure_remote_dirs(
    client: &reqwest::Client,
    server_url: &str,
    remote_dir: &str,
    username: &str,
    password: &str,
) -> Result<(), AppError> {
    let dir = remote_dir.trim().trim_matches('/');
    if dir.is_empty() {
        return Ok(());
    }

    let base = server_url.trim().trim_end_matches('/');
    let segments: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
    let mut accumulated = String::new();
    let mkcol = reqwest::Method::from_bytes(b"MKCOL")
        .map_err(|e| AppError::Internal(format!("MKCOL 方法创建失败: {e}")))?;

    for segment in &segments {
        accumulated.push('/');
        accumulated.push_str(segment);
        let dir_url = format!("{base}{accumulated}/");

        let resp = client
            .request(mkcol.clone(), &dir_url)
            .basic_auth(username, Some(password))
            .send()
            .await
            .map_err(|e| AppError::Upstream(format!("创建远程目录失败: {e}")))?;

        let status = resp.status().as_u16();
        if status == 201 || status == 301 || status == 405 {
            continue;
        }
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "创建远程目录 {} 失败 ({}): {}",
                dir_url,
                status,
                trim_error_body(&body)
            )));
        }
    }

    Ok(())
}

fn effective_server_url(server_url: &str) -> &str {
    let trimmed = server_url.trim();
    if trimmed.is_empty() {
        NUTSTORE_DEFAULT_URL
    } else {
        trimmed
    }
}

fn sqlite_path_from_url(database_url: &str) -> Result<PathBuf, AppError> {
    let rest = database_url
        .strip_prefix("sqlite:")
        .ok_or_else(|| AppError::Internal(format!("不支持的数据库地址: {database_url}")))?;
    let rest = rest.trim_start_matches("//");
    let path = rest.split('?').next().unwrap_or(rest);
    if path.is_empty() || path == ":memory:" {
        return Err(AppError::Internal(
            "当前数据库不是可备份的文件数据库".into(),
        ));
    }
    Ok(PathBuf::from(path))
}

fn temp_backup_path(prefix: &str) -> PathBuf {
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
    std::env::temp_dir().join(format!("ocm-{prefix}-{ts}.sqlite3"))
}

fn sqlite_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn build_archive_file_name(database_path: &Path) -> String {
    let stem = database_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("ocm");
    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    format!("{ts}.{stem}.sqlite3")
}

fn build_remote_url(server_url: &str, remote_dir: &str, file_name: &str) -> String {
    let base = server_url.trim().trim_end_matches('/');
    let dir = remote_dir.trim().trim_matches('/');
    if dir.is_empty() {
        format!("{base}/{file_name}")
    } else {
        format!("{base}/{dir}/{file_name}")
    }
}

fn trim_error_body(body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return "无响应内容".into();
    }
    body.chars().take(200).collect()
}

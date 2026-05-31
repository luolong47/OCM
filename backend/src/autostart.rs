//! XDG autostart integration for Linux desktops.

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

use serde::Serialize;

#[cfg(target_os = "linux")]
use crate::error::AppError;

#[cfg(target_os = "linux")]
const DESKTOP_FILE: &str = "ocm-backend.desktop";
#[cfg(target_os = "linux")]
const SCRIPT_FILE: &str = "ocm-autostart.sh";

#[derive(Debug, Clone, Serialize)]
pub struct AutostartStatus {
    pub enabled: bool,
    pub desktop_file: String,
    pub script_file: String,
    pub executable: String,
    pub working_dir: String,
}

#[cfg(target_os = "linux")]
pub fn status() -> Result<AutostartStatus, AppError> {
    let paths = autostart_paths()?;
    Ok(AutostartStatus {
        enabled: paths.desktop_file.exists() && paths.script_file.exists(),
        desktop_file: paths.desktop_file.display().to_string(),
        script_file: paths.script_file.display().to_string(),
        executable: paths.executable.display().to_string(),
        working_dir: paths.working_dir.display().to_string(),
    })
}

#[cfg(target_os = "linux")]
pub fn set_enabled(enabled: bool) -> Result<AutostartStatus, AppError> {
    if enabled {
        enable()?;
    } else {
        disable()?;
    }
    status()
}

#[cfg(target_os = "linux")]
fn enable() -> Result<(), AppError> {
    let paths = autostart_paths()?;
    if let Some(parent) = paths.desktop_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("create {}: {e}", parent.display())))?;
    }
    if let Some(parent) = paths.script_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::Internal(format!("create {}: {e}", parent.display())))?;
    }

    let script = format!(
        "#!/bin/sh\ncd {} || exit 1\nexec {}\n",
        shell_quote(&paths.working_dir.display().to_string()),
        shell_quote(&paths.executable.display().to_string()),
    );
    fs::write(&paths.script_file, script)
        .map_err(|e| AppError::Internal(format!("write {}: {e}", paths.script_file.display())))?;
    make_executable(&paths.script_file)?;

    let desktop = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=OCM\n\
         Comment=OpenCode Config Manager backend\n\
         Exec=/bin/sh {}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n\
         X-OCM-Autostart=true\n",
        desktop_quote(&paths.script_file.display().to_string()),
    );
    fs::write(&paths.desktop_file, desktop)
        .map_err(|e| AppError::Internal(format!("write {}: {e}", paths.desktop_file.display())))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn make_executable(path: &PathBuf) -> Result<(), AppError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .map_err(|e| AppError::Internal(format!("chmod {}: {e}", path.display())))
}

#[cfg(target_os = "linux")]
fn disable() -> Result<(), AppError> {
    let paths = autostart_paths()?;
    remove_if_exists(paths.desktop_file)?;
    remove_if_exists(paths.script_file)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn remove_if_exists(path: PathBuf) -> Result<(), AppError> {
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AppError::Internal(format!(
            "remove {}: {e}",
            path.display()
        ))),
    }
}

#[cfg(target_os = "linux")]
struct AutostartPaths {
    desktop_file: PathBuf,
    script_file: PathBuf,
    executable: PathBuf,
    working_dir: PathBuf,
}

#[cfg(target_os = "linux")]
fn autostart_paths() -> Result<AutostartPaths, AppError> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| AppError::Internal("cannot determine user config directory".into()))?;
    let executable = std::env::current_exe()
        .map_err(|e| AppError::Internal(format!("resolve current executable: {e}")))?;
    let working_dir = std::env::current_dir()
        .map_err(|e| AppError::Internal(format!("resolve current working directory: {e}")))?;
    Ok(AutostartPaths {
        desktop_file: config_dir.join("autostart").join(DESKTOP_FILE),
        script_file: config_dir.join("ocm").join(SCRIPT_FILE),
        executable,
        working_dir,
    })
}

#[cfg(target_os = "linux")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "linux")]
fn desktop_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

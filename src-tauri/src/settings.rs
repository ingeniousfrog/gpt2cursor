use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};
use tauri::Manager;

pub const DEFAULT_PORT: u16 = 8787;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AppSettings {
    pub port: u16,
    pub api_key: String,
    pub model: String,
    pub codex_command: String,
    #[serde(default)]
    pub codex_workdir: String,
    pub codex_model: String,
    pub codex_profile: String,
    pub codex_sandbox: String,
    pub codex_approval: String,
    pub codex_timeout_ms: u64,
    #[serde(default = "default_codex_max_messages")]
    pub codex_max_messages: usize,
    pub launch_at_login: bool,
    #[serde(default)]
    pub ngrok_enabled: bool,
    #[serde(default)]
    pub ngrok_authtoken: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            api_key: String::new(),
            model: "gpt2cursor-local".to_string(),
            codex_command: "codex".to_string(),
            codex_workdir: String::new(),
            codex_model: "gpt-5.5".to_string(),
            codex_profile: String::new(),
            codex_sandbox: "read-only".to_string(),
            codex_approval: "never".to_string(),
            codex_timeout_ms: 300_000,
            codex_max_messages: 12,
            launch_at_login: false,
            ngrok_enabled: false,
            ngrok_authtoken: String::new(),
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.api_key.trim().is_empty() {
            return Err("API key is required".to_string());
        }
        validate_port(self.port)?;
        validate_option(
            &self.codex_sandbox,
            &["read-only", "workspace-write", "danger-full-access"],
            "Codex sandbox",
        )?;
        validate_option(
            &self.codex_approval,
            &["untrusted", "on-request", "never"],
            "Codex approval",
        )?;
        if self.codex_timeout_ms < 1_000 {
            return Err("Codex timeout must be at least 1000 ms".to_string());
        }
        if self.codex_max_messages == 0 {
            return Err("Codex context must include at least 1 message".to_string());
        }
        Ok(())
    }
}

fn default_codex_max_messages() -> usize {
    12
}

pub fn validate_port(port: u16) -> Result<(), String> {
    if port == 0 {
        return Err("Port must be between 1 and 65535".to_string());
    }
    Ok(())
}

pub fn load_settings(path: &PathBuf) -> AppSettings {
    let Ok(raw) = fs::read_to_string(path) else {
        return AppSettings::default();
    };
    let mut settings = serde_json::from_str::<AppSettings>(&raw).unwrap_or_default();
    if settings.model != "gpt2cursor-local" {
        settings.model = "gpt2cursor-local".to_string();
    }
    if settings.codex_timeout_ms <= 120_000 {
        settings.codex_timeout_ms = 300_000;
    }
    if settings.codex_max_messages == 0 {
        settings.codex_max_messages = default_codex_max_messages();
    }
    settings
}

pub fn save_settings(path: &PathBuf, settings: &AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("Unable to create settings dir: {err}"))?;
    }
    let raw = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("Unable to encode settings: {err}"))?;
    fs::write(path, raw).map_err(|err| format!("Unable to save settings: {err}"))?;
    restrict_settings_permissions(path)
}

#[cfg(unix)]
fn restrict_settings_permissions(path: &PathBuf) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|err| format!("Unable to secure settings file permissions: {err}"))
}

#[cfg(not(unix))]
fn restrict_settings_permissions(_path: &PathBuf) -> Result<(), String> {
    Ok(())
}

pub fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|err| format!("Unable to resolve settings path: {err}"))
}

fn validate_option(value: &str, allowed: &[&str], label: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("{label} must be one of: {}", allowed.join(", ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_is_8787() {
        assert_eq!(AppSettings::default().port, 8787);
    }

    #[test]
    fn rejects_empty_api_key() {
        let settings = AppSettings::default();
        assert!(settings.validate().unwrap_err().contains("API key"));
    }

    #[test]
    fn rejects_unsupported_sandbox() {
        let settings = AppSettings {
            api_key: "local".to_string(),
            codex_sandbox: "full".to_string(),
            ..AppSettings::default()
        };
        assert!(settings.validate().unwrap_err().contains("sandbox"));
    }
}

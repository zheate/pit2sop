use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub vault_path: PathBuf,
    pub language: String,
    pub ai: AiConfig,
    #[serde(skip, default)]
    pub db_path_override: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Secrets {
    pub deepseek_api_key: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            vault_path: default_test_vault_path(),
            language: "zh-CN".to_string(),
            db_path_override: None,
            ai: AiConfig {
                provider: "deepseek".to_string(),
                model: "deepseek-v4-pro".to_string(),
                base_url: "https://api.deepseek.com".to_string(),
            },
        }
    }
}

pub fn pit2sop_home() -> PathBuf {
    if let Ok(path) = std::env::var("PIT2SOP_HOME") {
        return PathBuf::from(path);
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pit2sop")
}

pub fn config_path() -> PathBuf {
    pit2sop_home().join("config.toml")
}

pub fn secrets_path() -> PathBuf {
    pit2sop_home().join("secrets.toml")
}

pub fn default_test_vault_path() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("test")
        .join("pit2sop")
        .join("test-vault")
}

pub fn db_path_for_vault(vault_path: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(vault_path.to_string_lossy().as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    pit2sop_home()
        .join("cache")
        .join(&hash[..12])
        .join("pit2sop.sqlite")
}

impl AppConfig {
    pub fn load_or_default() -> Result<Self> {
        let path = config_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("failed to parse config {}", path.display()))
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn db_path(&self) -> PathBuf {
        if let Some(path) = &self.db_path_override {
            return path.clone();
        }
        db_path_for_vault(&self.vault_path)
    }
}

impl Secrets {
    pub fn load_or_default() -> Result<Self> {
        let path = secrets_path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read secrets {}", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("failed to parse secrets {}", path.display()))
    }

    pub fn save_if_missing(&self) -> Result<()> {
        let path = secrets_path();
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = match &self.deepseek_api_key {
            Some(value) => format!("deepseek_api_key = {:?}\n", value),
            None => "deepseek_api_key = \"\"\n".to_string(),
        };
        fs::write(&path, content)?;
        restrict_file_permissions(&path)?;
        Ok(())
    }

    pub fn has_deepseek_key(&self) -> bool {
        self.deepseek_api_key
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || std::env::var("DEEPSEEK_API_KEY")
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
    }
}

fn restrict_file_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    #[cfg(not(unix))]
    {
        let _ = path;
    }

    Ok(())
}

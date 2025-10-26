use dialoguer::{Input, Password};
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

const CONFIG_SUBDIR: &str = ".config/telltales";
const CONFIG_FILE: &str = "credentials.yaml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("unable to locate the home directory")]
    MissingHomeDir,
    #[error("failed to create configuration directory {0}: {1}")]
    CreateDirFailed(String, #[source] io::Error),
    #[error("failed to read configuration file {0}: {1}")]
    ReadFailed(String, #[source] io::Error),
    #[error("failed to parse configuration file {0}: {1}")]
    ParseFailed(String, #[source] serde_yaml::Error),
    #[error("failed to serialize configuration: {0}")]
    SerializeFailed(#[source] serde_yaml::Error),
    #[error("failed to write configuration file {0}: {1}")]
    WriteFailed(String, #[source] io::Error),
    #[error(transparent)]
    PromptFailed(#[from] dialoguer::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TelldusCredentials {
    pub public_key: String,
    pub private_key: String,
    pub token: String,
    pub token_secret: String,
}

impl TelldusCredentials {
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.public_key.trim().is_empty() {
            missing.push("public_key");
        }
        if self.private_key.trim().is_empty() {
            missing.push("private_key");
        }
        missing
    }

    pub fn is_complete(&self) -> bool {
        self.missing_fields().is_empty()
    }
}

pub fn ensure_credentials() -> Result<TelldusCredentials, ConfigError> {
    let mut creds = load_credentials()?.unwrap_or_default();
    if !creds.is_complete() {
        prompt_for_missing(&mut creds)?;
        save_credentials(&creds)?;
    }
    Ok(creds)
}

fn load_credentials() -> Result<Option<TelldusCredentials>, ConfigError> {
    let path = credentials_path_internal()?;
    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)
        .map_err(|err| ConfigError::ReadFailed(display_path(&path), err))?;
    let parsed = serde_yaml::from_str(&contents)
        .map_err(|err| ConfigError::ParseFailed(display_path(&path), err))?;
    Ok(Some(parsed))
}

fn save_credentials(credentials: &TelldusCredentials) -> Result<(), ConfigError> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir)
        .map_err(|err| ConfigError::CreateDirFailed(display_path(&dir), err))?;

    let path = dir.join(CONFIG_FILE);
    let yaml = serde_yaml::to_string(credentials).map_err(ConfigError::SerializeFailed)?;
    fs::write(&path, yaml).map_err(|err| ConfigError::WriteFailed(display_path(&path), err))?;

    Ok(())
}

fn prompt_for_missing(creds: &mut TelldusCredentials) -> Result<(), ConfigError> {
    println!(
        "Telldus Live credentials are required. Values are stored in {}.",
        display_path(&credentials_path_internal()?)
    );

    creds.public_key = prompt_field("Public API key", &creds.public_key, false)?;
    creds.private_key = prompt_field("Private API key", &creds.private_key, true)?;
    if creds.token.trim().is_empty() || creds.token_secret.trim().is_empty() {
        println!(
            "OAuth access token details are optional for validation and can be set later via the OAuth flow."
        );
    } else {
        println!("Existing OAuth access token details detected; leaving untouched.");
    }

    Ok(())
}

fn prompt_field(prompt: &str, current: &str, secret: bool) -> Result<String, ConfigError> {
    if !current.trim().is_empty() {
        println!("{prompt} already present; leave blank to keep.");
    }

    if secret {
        if current.trim().is_empty() {
            let value = Password::new()
                .with_prompt(prompt)
                .with_confirmation("Confirm", "Entries do not match.")
                .interact()?;
            return Ok(value.trim().to_string());
        } else {
            let value = Password::new()
                .with_prompt(prompt)
                .allow_empty_password(true)
                .interact()?;
            return Ok(if value.trim().is_empty() {
                current.to_string()
            } else {
                value.trim().to_string()
            });
        }
    }

    let value = Input::<String>::new()
        .with_prompt(prompt)
        .allow_empty(!current.trim().is_empty())
        .interact()?;

    if value.trim().is_empty() && !current.trim().is_empty() {
        Ok(current.to_string())
    } else if value.trim().is_empty() {
        println!("A value is required for {prompt}.");
        prompt_field(prompt, current, secret)
    } else {
        Ok(value.trim().to_string())
    }
}

pub fn credentials_path() -> Result<PathBuf, ConfigError> {
    credentials_path_internal()
}

fn config_dir() -> Result<PathBuf, ConfigError> {
    let home = home_dir().ok_or(ConfigError::MissingHomeDir)?;
    Ok(home.join(CONFIG_SUBDIR))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn credentials_path_internal() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join(CONFIG_FILE))
}

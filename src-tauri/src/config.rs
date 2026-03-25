use crate::crypto;
use crate::models::{AppConfig, ExchangeCredentials, MaskedCredentials};
use crate::paths;
use std::collections::HashMap;
use std::fs;

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = paths::get_data_dir()?.join("config.json");
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load_config() -> Result<AppConfig, String> {
    let path = paths::get_data_dir()?.join("config.json");
    if path.exists() {
        let json = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read config: {e}"))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse config: {e}"))
    } else {
        Ok(AppConfig::default())
    }
}

pub fn save_credentials(creds: &ExchangeCredentials) -> Result<(), String> {
    let key = crypto::load_or_create_key()?;
    let path = paths::get_data_dir()?.join("credentials.json");

    let mut all_creds = load_all_credentials_raw()?;

    let encrypted = EncryptedCredentials {
        exchange: creds.exchange.clone(),
        api_key: crypto::encrypt(&creds.api_key, &key)?,
        secret: crypto::encrypt(&creds.secret, &key)?,
        passphrase: creds
            .passphrase
            .as_ref()
            .map(|p| crypto::encrypt(p, &key))
            .transpose()?,
    };

    all_creds.insert(creds.exchange.clone(), encrypted);

    let json = serde_json::to_string_pretty(&all_creds).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn get_credentials(exchange: &str) -> Result<Option<MaskedCredentials>, String> {
    let key = crypto::load_or_create_key()?;
    let all = load_all_credentials_raw()?;
    let enc = match all.get(exchange) {
        Some(e) => e,
        None => return Ok(None),
    };

    let api_key = crypto::decrypt(&enc.api_key, &key)?;
    let masked = if api_key.len() > 4 {
        format!("****{}", &api_key[api_key.len() - 4..])
    } else {
        "****".to_string()
    };

    Ok(Some(MaskedCredentials {
        exchange: exchange.to_string(),
        api_key_masked: masked,
        has_secret: true,
        has_passphrase: enc.passphrase.is_some(),
    }))
}

pub fn delete_credentials(exchange: &str) -> Result<(), String> {
    let path = paths::get_data_dir()?.join("credentials.json");
    let mut all = load_all_credentials_raw()?;
    all.remove(exchange);
    let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EncryptedCredentials {
    exchange: String,
    api_key: String,
    secret: String,
    passphrase: Option<String>,
}

fn load_all_credentials_raw() -> Result<HashMap<String, EncryptedCredentials>, String> {
    let path = paths::get_data_dir()?.join("credentials.json");
    if path.exists() {
        let json = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read credentials: {e}"))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse credentials: {e}"))
    } else {
        Ok(HashMap::new())
    }
}

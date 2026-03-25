use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use std::fs;
use std::path::Path;

use crate::paths;

fn get_key_path() -> Result<std::path::PathBuf, String> {
    Ok(paths::get_data_dir()?.join("master.key"))
}

pub fn load_or_create_key() -> Result<Vec<u8>, String> {
    let key_path = get_key_path()?;
    if key_path.exists() {
        let encoded = fs::read_to_string(&key_path)
            .map_err(|e| format!("Failed to read encryption key: {e}. Your encrypted credentials may be unrecoverable if this file is damaged."))?;
        BASE64
            .decode(encoded.trim())
            .map_err(|e| format!("Encryption key file is corrupted: {e}. Refusing to overwrite — back up or delete {} manually.", key_path.display()))
    } else {
        generate_and_save_key(&key_path)
    }
}

fn generate_and_save_key(path: &Path) -> Result<Vec<u8>, String> {
    let mut key = vec![0u8; 32];
    OsRng.fill_bytes(&mut key);
    let encoded = BASE64.encode(&key);
    fs::write(path, &encoded)
        .map_err(|e| format!("Failed to write encryption key to {}: {e}", path.display()))?;
    Ok(key)
}

pub fn encrypt(plaintext: &str, key: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

pub fn decrypt(encrypted: &str, key: &[u8]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let combined = BASE64.decode(encrypted).map_err(|e| e.to_string())?;

    if combined.len() < 12 {
        return Err("Invalid encrypted data: too short".to_string());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Decryption failed: data may be corrupted or encrypted with a different key".to_string())?;

    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

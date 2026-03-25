use std::fs;
use std::path::PathBuf;

pub fn get_data_dir() -> Result<PathBuf, String> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| "Could not determine application data directory".to_string())?
        .join("aetherbot");
    fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to create data directory: {e}"))?;
    Ok(data_dir)
}

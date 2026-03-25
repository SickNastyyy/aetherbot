use crate::config;
use crate::engine::BotEngine;
use crate::models::*;
use crate::EngineState;

const ALLOWED_EXCHANGES: &[&str] = &["Jupiter", "Uniswap", "Kalshi"];
const MAX_KEY_LENGTH: usize = 256;

fn validate_exchange(exchange: &str) -> Result<(), String> {
    if !ALLOWED_EXCHANGES.contains(&exchange) {
        return Err(format!(
            "Unknown exchange '{}'. Allowed: {}",
            exchange,
            ALLOWED_EXCHANGES.join(", ")
        ));
    }
    Ok(())
}

fn validate_key_length(value: &str, field: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} cannot be empty"));
    }
    if trimmed.len() > MAX_KEY_LENGTH {
        return Err(format!("{field} exceeds maximum length of {MAX_KEY_LENGTH} characters"));
    }
    Ok(())
}

#[tauri::command]
pub fn save_exchange_credentials(
    exchange: String,
    api_key: String,
    secret: String,
    passphrase: Option<String>,
) -> Result<(), String> {
    validate_exchange(&exchange)?;
    validate_key_length(&api_key, "API key")?;
    validate_key_length(&secret, "Secret")?;
    if let Some(ref p) = passphrase {
        validate_key_length(p, "Passphrase")?;
    }

    let creds = ExchangeCredentials {
        exchange,
        api_key: api_key.trim().to_string(),
        secret: secret.trim().to_string(),
        passphrase: passphrase.map(|p| p.trim().to_string()),
    };
    config::save_credentials(&creds)
}

#[tauri::command]
pub fn get_exchange_credentials(exchange: String) -> Result<Option<MaskedCredentials>, String> {
    validate_exchange(&exchange)?;
    config::get_credentials(&exchange)
}

#[tauri::command]
pub fn delete_exchange_credentials(exchange: String) -> Result<(), String> {
    validate_exchange(&exchange)?;
    config::delete_credentials(&exchange)
}

fn validate_config(config: &AppConfig) -> Result<(), String> {
    if config.max_drawdown_pct <= 0.0 || config.max_drawdown_pct > 100.0 {
        return Err("Max drawdown must be between 0 and 100%".to_string());
    }
    if config.position_size_pct <= 0.0 || config.position_size_pct > 100.0 {
        return Err("Position size must be between 0 and 100%".to_string());
    }
    if config.stop_loss_pct <= 0.0 || config.stop_loss_pct > 50.0 {
        return Err("Stop-loss must be between 0 and 50%".to_string());
    }
    if config.max_drawdown_pct.is_nan() || config.position_size_pct.is_nan() || config.stop_loss_pct.is_nan() {
        return Err("Risk parameters must be valid numbers".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn save_app_config(config_data: AppConfig) -> Result<(), String> {
    validate_config(&config_data)?;
    config::save_config(&config_data)
}

#[tauri::command]
pub fn get_app_config() -> Result<AppConfig, String> {
    config::load_config()
}

#[tauri::command]
pub fn get_system_status() -> SystemStatus {
    SystemStatus {
        uptime_secs: 0,
        mode: "paper".to_string(),
        connections: vec![
            ConnectionStatus {
                name: "Jupiter".to_string(),
                connected: false,
                latency_ms: None,
            },
            ConnectionStatus {
                name: "Uniswap".to_string(),
                connected: false,
                latency_ms: None,
            },
            ConnectionStatus {
                name: "Kalshi".to_string(),
                connected: false,
                latency_ms: None,
            },
        ],
        bot_running: false,
    }
}

#[tauri::command]
pub fn get_mock_pnl() -> PnlSummary {
    PnlSummary {
        total_pnl: 0.0,
        total_pnl_pct: 0.0,
        today_pnl: 0.0,
        win_rate: 0.0,
        total_trades: 0,
        active_positions: 0,
    }
}

// ── Engine commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_bot(
    app: tauri::AppHandle,
    engine_state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    let mut engine_lock = engine_state.lock().await;

    if engine_lock.is_some() {
        return Err("Bot is already running".into());
    }

    let config = config::load_config()?;
    let mut engine = BotEngine::new(app, config);
    engine.start().await;
    *engine_lock = Some(engine);

    Ok(())
}

#[tauri::command]
pub async fn stop_bot(
    mode: String,
    engine_state: tauri::State<'_, EngineState>,
) -> Result<(), String> {
    let mut engine_lock = engine_state.lock().await;

    match engine_lock.as_mut() {
        Some(engine) => {
            engine.stop(&mode).await;
            *engine_lock = None;
            Ok(())
        }
        None => Err("Bot is not running".into()),
    }
}

#[tauri::command]
pub async fn get_bot_state(
    engine_state: tauri::State<'_, EngineState>,
) -> Result<BotEngineState, String> {
    let engine_lock = engine_state.lock().await;
    match engine_lock.as_ref() {
        Some(engine) => Ok(engine.state().await),
        None => Ok(BotEngineState::Idle),
    }
}

use crate::config;
use crate::database::Database;
use crate::engine::jupiter::config::JupiterSwapConfig;
use crate::engine::jupiter::swap::SwapOrchestrator;
use crate::engine::jupiter::ticket::SwapTicket;
use crate::engine::BotEngine;
use crate::models::*;
use crate::paths;
use crate::EngineState;
use std::sync::Arc;

pub type JupiterState = Arc<tokio::sync::Mutex<Option<SwapOrchestrator>>>;

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
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    let mut engine_lock = engine_state.lock().await;

    if engine_lock.is_some() {
        return Err("Bot is already running".into());
    }

    let config = config::load_config()?;
    let mut engine = BotEngine::new(app, config);

    // Create an Arc to share database with the engine
    // We re-open a connection since Tauri manages the original
    let db_path = crate::paths::get_data_dir()?.join("trades.db");
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("Failed to open database for engine: {e}"))?;
    let engine_db = Arc::new(Database { conn: std::sync::Mutex::new(conn) });

    engine.start(engine_db).await;
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
pub fn get_trade_history(
    limit: u32,
    db: tauri::State<'_, Database>,
) -> Result<Vec<crate::database::TradeHistoryRow>, String> {
    let conn = db.conn.lock().map_err(|e| format!("DB lock error: {e}"))?;
    crate::database::get_trade_history(&conn, limit).map_err(|e| format!("DB error: {e}"))
}

#[tauri::command]
pub fn run_backtest(
    exchange: String,
    pair: String,
    db: tauri::State<'_, Database>,
) -> Result<crate::engine::backtest::BacktestResult, String> {
    let config = crate::engine::backtest::BacktestConfig {
        exchange,
        pair,
        initial_balance: 10000.0,
        position_size_pct: 5.0,
        stop_loss_pct: 3.0,
        take_profit_pct: 6.0,
    };
    crate::engine::backtest::run_backtest(&db, config)
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

// ── Jupiter live swap commands ────────────────────────────────────────────────

#[tauri::command]
pub fn jupiter_get_config() -> Result<JupiterSwapConfig, String> {
    let data_dir = paths::get_data_dir()?;
    JupiterSwapConfig::load(&data_dir)
}

#[tauri::command]
pub async fn jupiter_save_config(
    config: JupiterSwapConfig,
    jup_state: tauri::State<'_, JupiterState>,
) -> Result<(), String> {
    // Validate all limits (config.validate() checks all 8 hard limits)
    config.validate()?;
    let data_dir = paths::get_data_dir()?;
    config.save(&data_dir)?;
    // Reset the orchestrator so the new config is picked up on the next call
    *jup_state.lock().await = None;
    Ok(())
}

/// Step 1 + 2: Fetch a quote and run all safety gates.
/// Returns a SwapTicket for the owner to review. Does NOT sign or submit.
#[tauri::command]
pub async fn jupiter_quote_swap(
    input_mint: String,
    output_mint: String,
    in_amount: u64,
    sol_price_usd: f64,
    jup_state: tauri::State<'_, JupiterState>,
    db: tauri::State<'_, Database>,
) -> Result<SwapTicket, String> {
    let mut lock = jup_state.lock().await;

    // Lazily create the orchestrator on first use
    if lock.is_none() {
        let data_dir = paths::get_data_dir()?;
        let config = JupiterSwapConfig::load(&data_dir)?;
        let db_path = data_dir.join("trades.db");
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| format!("DB open: {e}"))?;
        let arc_db = Arc::new(Database { conn: std::sync::Mutex::new(conn) });
        let orchestrator = SwapOrchestrator::new(config, arc_db);

        // Restore persisted kill switch state
        let ks_active = {
            let db_conn = db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
            crate::database::get_kill_switch(&db_conn)
                .map_err(|e| format!("DB get_kill_switch: {e}"))?
        };
        if ks_active {
            orchestrator.kill_switch().activate();
        }

        *lock = Some(orchestrator);
    }

    let orchestrator = lock.as_ref().unwrap();
    orchestrator
        .create_ticket(&input_mint, &output_mint, in_amount, sol_price_usd)
        .await
}

/// Step 3: Owner explicitly approves the ticket (behind UI password check).
/// Returns the updated ticket. Does NOT submit.
#[tauri::command]
pub async fn jupiter_approve_ticket(
    ticket_id: String,
    jup_state: tauri::State<'_, JupiterState>,
) -> Result<SwapTicket, String> {
    let lock = jup_state.lock().await;
    let orchestrator = lock
        .as_ref()
        .ok_or("Jupiter orchestrator not initialised — call jupiter_quote_swap first")?;
    orchestrator.approve_ticket(&ticket_id)
}

/// Steps 4–8: Execute an already-approved ticket.
/// Simulate → sign → submit → confirm → reconcile.
/// On timeout or unknown result: marks PENDING, never resubmits.
#[tauri::command]
pub async fn jupiter_execute_approved(
    ticket_id: String,
    jup_state: tauri::State<'_, JupiterState>,
    db: tauri::State<'_, Database>,
) -> Result<SwapTicket, String> {
    // Check DB kill switch before acquiring the orchestrator lock
    {
        let conn = db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        let ks = crate::database::get_kill_switch(&conn)
            .map_err(|e| format!("DB get_kill_switch: {e}"))?;
        if ks {
            return Err("Kill switch is active — execution blocked".into());
        }
    }

    let lock = jup_state.lock().await;
    let orchestrator = lock
        .as_ref()
        .ok_or("Jupiter orchestrator not initialised")?;
    orchestrator.execute_approved(&ticket_id).await
}

/// One command that blocks all new swap tickets immediately.
/// Persists to DB so the kill switch survives restarts.
#[tauri::command]
pub async fn jupiter_kill_switch(
    activate: bool,
    jup_state: tauri::State<'_, JupiterState>,
    db: tauri::State<'_, Database>,
) -> Result<(), String> {
    // Persist to DB first — works even if orchestrator is not running
    {
        let conn = db.conn.lock().map_err(|e| format!("DB lock: {e}"))?;
        crate::database::set_kill_switch(&conn, activate)
            .map_err(|e| format!("DB set_kill_switch: {e}"))?;
    }

    // Also set on the in-memory orchestrator if it exists
    let lock = jup_state.lock().await;
    if let Some(orchestrator) = lock.as_ref() {
        if activate {
            orchestrator.kill_switch().activate();
        } else {
            orchestrator.kill_switch().deactivate();
        }
    }
    Ok(())
}

/// Return the Jupiter audit log (most recent first).
#[tauri::command]
pub async fn jupiter_get_audit_log(
    limit: u32,
    jup_state: tauri::State<'_, JupiterState>,
) -> Result<Vec<crate::database::JupiterAuditRow>, String> {
    let lock = jup_state.lock().await;
    let orchestrator = lock
        .as_ref()
        .ok_or("Jupiter orchestrator not initialised")?;

    // Access audit log via the ticket store's underlying db
    // We re-open the DB for simplicity (avoids exposing AuditLedger from SwapOrchestrator)
    let data_dir = paths::get_data_dir()?;
    let db_path = data_dir.join("trades.db");
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| format!("DB open: {e}"))?;
    crate::database::get_jupiter_audit_log(&conn, limit)
        .map_err(|e| format!("DB error: {e}"))
}

/// List all in-flight tickets (for dashboard display).
#[tauri::command]
pub async fn jupiter_list_tickets(
    jup_state: tauri::State<'_, JupiterState>,
) -> Result<Vec<SwapTicket>, String> {
    let lock = jup_state.lock().await;
    match lock.as_ref() {
        Some(o) => o.ticket_store().all(),
        None => Ok(vec![]),
    }
}

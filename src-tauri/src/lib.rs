mod commands;
mod config;
mod crypto;
mod database;
mod engine;
mod models;
mod paths;

use commands::*;
use engine::BotEngine;
use std::sync::Arc;
use tokio::sync::Mutex;

pub type EngineState = Arc<Mutex<Option<BotEngine>>>;
pub type JupiterState = commands::JupiterState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let db = database::init_database().expect("Failed to initialize database");

    tauri::Builder::default()
        .manage(db)
        .manage(Arc::new(Mutex::new(None::<BotEngine>)) as EngineState)
        .manage(Arc::new(Mutex::new(None)) as JupiterState)
        .invoke_handler(tauri::generate_handler![
            save_exchange_credentials,
            get_exchange_credentials,
            delete_exchange_credentials,
            save_app_config,
            get_app_config,
            get_system_status,
            get_mock_pnl,
            start_bot,
            stop_bot,
            get_bot_state,
            get_trade_history,
            run_backtest,
            jupiter_get_config,
            jupiter_save_config,
            jupiter_quote_swap,
            jupiter_approve_ticket,
            jupiter_execute_approved,
            jupiter_kill_switch,
            jupiter_get_audit_log,
            jupiter_list_tickets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

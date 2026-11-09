/// Jupiter swap executor.
///
/// This module bridges the BotEngine's existing order flow to the Jupiter
/// live swap path. The actual swap logic lives in `engine::jupiter::swap`.
///
/// Execution gate: ALL of the following must be true before any live swap:
///   1. `JupiterSwapConfig.live_execution_enabled = true`  (owner sets explicitly)
///   2. `wallet_public_key` is non-empty
///   3. `mint_allowlist` is non-empty
///   4. `signer_key_path` is set
///   5. Per-ticket owner approval (see `SwapOrchestrator::approve_ticket`)
///
/// Default stays OFF. Paper mode is unaffected.
use crate::engine::jupiter::config::JupiterSwapConfig;
use crate::engine::jupiter::swap::SwapOrchestrator;
use crate::engine::jupiter::ticket::SwapTicket;

/// Create and return a SwapOrchestrator bound to the given config and database.
/// The caller is responsible for holding it and routing approval commands to it.
pub fn build_orchestrator(
    config: JupiterSwapConfig,
    db: std::sync::Arc<crate::database::Database>,
) -> SwapOrchestrator {
    SwapOrchestrator::new(config, db)
}

/// Convenience wrapper: returns Err immediately if the execution gate is closed.
/// Pass through to `SwapOrchestrator::create_ticket` when the gate is open.
pub async fn request_quote(
    orchestrator: &SwapOrchestrator,
    input_mint: &str,
    output_mint: &str,
    in_amount: u64,
    sol_price_usd: f64,
) -> Result<SwapTicket, String> {
    orchestrator
        .create_ticket(input_mint, output_mint, in_amount, sol_price_usd)
        .await
}

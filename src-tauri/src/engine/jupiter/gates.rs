use serde::{Deserialize, Serialize};

use super::config::{JupiterSwapConfig, SOL_MINT, USDC_MINT};
use super::quote::SwapQuote;
use super::wallet::WalletBalances;

// ── Gate result types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "reason", rename_all = "snake_case")]
pub enum GateStatus {
    Pass,
    Warn(String),
    Fail(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub name: String,
    pub status: GateStatus,
}

impl GateResult {
    fn pass(name: &'static str) -> Self {
        Self { name: name.into(), status: GateStatus::Pass }
    }
    fn warn(name: &'static str, reason: impl Into<String>) -> Self {
        Self { name: name.into(), status: GateStatus::Warn(reason.into()) }
    }
    fn fail(name: &'static str, reason: impl Into<String>) -> Self {
        Self { name: name.into(), status: GateStatus::Fail(reason.into()) }
    }
}

// ── Gate input ────────────────────────────────────────────────────────────────

pub struct GateInput<'a> {
    pub quote: &'a SwapQuote,
    pub balances: &'a WalletBalances,
    pub config: &'a JupiterSwapConfig,
    /// Today's realized + unrealized loss in USD (positive = loss).
    pub daily_loss_usd: f64,
    pub start_of_day_equity_usd: f64,
    pub daily_stop_blocked: bool,
    pub kill_switch_active: bool,
    /// Current SOL price in USD (fresh from price feed).
    pub sol_price_usd: f64,
    /// Current unix timestamp (for quote age check).
    pub now_ts: i64,
}

// ── Run all gates ──────────────────────────────────────────────────────────────

pub fn run_all_gates(input: &GateInput) -> Vec<GateResult> {
    vec![
        gate_kill_switch(input),
        gate_daily_stop(input),
        gate_mint_allowlist(input),
        gate_stale_quote(input),
        gate_account_cap(input),
        gate_per_swap_limit(input),
        gate_slippage(input),
        gate_price_impact(input),
        gate_sol_reserve(input),
        gate_meme_cap_warning(input),
    ]
}

/// Returns true when no gate returned Fail.
/// Warn results are advisory and do not block execution.
pub fn all_gates_passed(results: &[GateResult]) -> bool {
    results
        .iter()
        .all(|r| !matches!(r.status, GateStatus::Fail(_)))
}

// ── Individual gates ─────────────────────────────────────────────────────────

fn gate_kill_switch(input: &GateInput) -> GateResult {
    if input.kill_switch_active {
        GateResult::fail("kill_switch", "Kill switch is active — all swaps blocked")
    } else {
        GateResult::pass("kill_switch")
    }
}

fn gate_daily_stop(input: &GateInput) -> GateResult {
    if input.daily_stop_blocked {
        return GateResult::fail(
            "daily_stop",
            "Daily stop triggered — no new swaps until next NY calendar day",
        );
    }
    let loss_pct = if input.start_of_day_equity_usd > 0.0 {
        (input.daily_loss_usd / input.start_of_day_equity_usd) * 100.0
    } else {
        0.0
    };
    if loss_pct >= input.config.daily_stop_pct {
        GateResult::fail(
            "daily_stop",
            format!(
                "Daily loss {loss_pct:.1}% >= {:.1}% stop threshold",
                input.config.daily_stop_pct
            ),
        )
    } else {
        GateResult::pass("daily_stop")
    }
}

fn gate_mint_allowlist(input: &GateInput) -> GateResult {
    if input.config.mint_allowlist.is_empty() {
        return GateResult::fail("mint_allowlist", "Allowlist is empty — fail closed");
    }
    if !input
        .config
        .mint_allowlist
        .contains(&input.quote.input_mint)
    {
        return GateResult::fail(
            "mint_allowlist",
            format!("Input mint {} is not in allowlist", input.quote.input_mint),
        );
    }
    if !input
        .config
        .mint_allowlist
        .contains(&input.quote.output_mint)
    {
        return GateResult::fail(
            "mint_allowlist",
            format!("Output mint {} is not in allowlist", input.quote.output_mint),
        );
    }
    GateResult::pass("mint_allowlist")
}

fn gate_stale_quote(input: &GateInput) -> GateResult {
    let age_secs = input.now_ts - input.quote.fetched_at;
    let max_age = (input.config.quote_max_age_secs + input.config.clock_skew_secs) as i64;
    if age_secs < 0 {
        // Clock skew tolerance — quote appears newer than now
        GateResult::pass("stale_quote")
    } else if age_secs > max_age {
        GateResult::fail(
            "stale_quote",
            format!(
                "Quote is {age_secs}s old — max allowed {}s ({}s + {}s skew)",
                max_age, input.config.quote_max_age_secs, input.config.clock_skew_secs
            ),
        )
    } else {
        GateResult::pass("stale_quote")
    }
}

fn gate_account_cap(input: &GateInput) -> GateResult {
    if input.balances.total_usd > input.config.account_cap_usd {
        GateResult::fail(
            "account_cap",
            format!(
                "Account value ${:.2} > hard cap ${:.2} — raise cap in config to proceed",
                input.balances.total_usd, input.config.account_cap_usd
            ),
        )
    } else {
        GateResult::pass("account_cap")
    }
}

fn gate_per_swap_limit(input: &GateInput) -> GateResult {
    let max_swap_usd = input.balances.total_usd * (input.config.max_swap_pct / 100.0);
    let input_usd = estimate_input_usd(input);

    match input_usd {
        Some(usd) => {
            if usd > max_swap_usd {
                GateResult::fail(
                    "per_swap_limit",
                    format!(
                        "Swap value ~${usd:.2} > {:.0}% limit ~${max_swap_usd:.2}",
                        input.config.max_swap_pct
                    ),
                )
            } else {
                GateResult::pass("per_swap_limit")
            }
        }
        None => GateResult::warn(
            "per_swap_limit",
            "Cannot estimate input USD for this token pair — verify 25% limit manually",
        ),
    }
}

fn gate_slippage(input: &GateInput) -> GateResult {
    if input.quote.slippage_bps > input.config.max_slippage_bps {
        GateResult::fail(
            "slippage",
            format!(
                "Requested slippage {}bps > max {}bps",
                input.quote.slippage_bps, input.config.max_slippage_bps
            ),
        )
    } else {
        GateResult::pass("slippage")
    }
}

fn gate_price_impact(input: &GateInput) -> GateResult {
    if input.quote.price_impact_pct > input.config.max_price_impact_pct {
        GateResult::fail(
            "price_impact",
            format!(
                "Price impact {:.2}% > max {:.1}%",
                input.quote.price_impact_pct, input.config.max_price_impact_pct
            ),
        )
    } else {
        GateResult::pass("price_impact")
    }
}

fn gate_sol_reserve(input: &GateInput) -> GateResult {
    if input.sol_price_usd <= 0.0 {
        return GateResult::fail("sol_reserve", "SOL price unavailable — cannot check reserve");
    }

    let reserve_lamports =
        ((input.config.sol_reserve_usd / input.sol_price_usd) * 1_000_000_000.0) as u64;

    // Estimated fee: priority fee + base transaction fee (~5000 lamports)
    let est_fee_lamports =
        (input.config.priority_fee_cap_sol * 1_000_000_000.0) as u64 + 5_000;

    // SOL outflow: only non-zero when input token is SOL (wrapped SOL)
    let sol_outflow_lamports = if input.quote.input_mint == SOL_MINT {
        input.quote.in_amount
    } else {
        0
    };

    let available = input
        .balances
        .sol_lamports
        .saturating_sub(sol_outflow_lamports)
        .saturating_sub(est_fee_lamports);

    if available < reserve_lamports {
        GateResult::fail(
            "sol_reserve",
            format!(
                "SOL after swap ({:.4} SOL) < ${:.2} reserve ({:.4} SOL)",
                available as f64 / 1e9,
                input.config.sol_reserve_usd,
                reserve_lamports as f64 / 1e9,
            ),
        )
    } else {
        GateResult::pass("sol_reserve")
    }
}

/// Advisory warning — not a hard block. Meme cap requires owner confirmation of the 75% default.
fn gate_meme_cap_warning(input: &GateInput) -> GateResult {
    // A "meme" token is any token not in { SOL_MINT, USDC_MINT }.
    let is_meme_output = input.quote.output_mint != SOL_MINT
        && input.quote.output_mint != USDC_MINT;

    if !is_meme_output {
        return GateResult::pass("meme_cap");
    }

    let meme_usd: f64 = input
        .balances
        .token_accounts
        .iter()
        .filter(|t| t.mint != SOL_MINT && t.mint != USDC_MINT)
        .filter_map(|t| t.amount_usd)
        .sum();

    let input_usd = estimate_input_usd(input).unwrap_or(0.0);
    let projected_meme_pct = if input.balances.total_usd > 0.0 {
        ((meme_usd + input_usd) / input.balances.total_usd) * 100.0
    } else {
        0.0
    };

    if projected_meme_pct > input.config.meme_cap_pct {
        GateResult::warn(
            "meme_cap",
            format!(
                "Projected meme exposure {projected_meme_pct:.1}% > {:.0}% cap — owner should confirm",
                input.config.meme_cap_pct
            ),
        )
    } else {
        GateResult::pass("meme_cap")
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/// Estimate the USD value of the input token.
/// Returns None when it cannot be computed without an external price feed.
fn estimate_input_usd(input: &GateInput) -> Option<f64> {
    if input.quote.input_mint == SOL_MINT {
        // SOL input: atoms / 1e9 * price
        Some((input.quote.in_amount as f64 / 1_000_000_000.0) * input.sol_price_usd)
    } else if input.quote.output_mint == SOL_MINT {
        // X→SOL: derive from output value
        Some((input.quote.out_amount as f64 / 1_000_000_000.0) * input.sol_price_usd)
    } else if input.quote.input_mint == USDC_MINT {
        // USDC input: atoms / 1e6 (USDC ≈ $1)
        Some(input.quote.in_amount as f64 / 1_000_000.0)
    } else if input.quote.output_mint == USDC_MINT {
        // X→USDC: derive from output value
        Some(input.quote.out_amount as f64 / 1_000_000.0)
    } else {
        None
    }
}

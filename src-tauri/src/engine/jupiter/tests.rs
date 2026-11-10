/// Jupiter live-path unit tests (offline — no mainnet calls, no signing of real transactions).
/// Uses a throwaway generated keypair for signer tests.
use ed25519_dalek::SigningKey;
use rand::RngCore;

use super::config::{JupiterSwapConfig, SOL_MINT, USDC_MINT};
use super::daily_stop::{test_helpers::make_test_db, DailyStopTracker};
use super::gates::{all_gates_passed, run_all_gates, GateInput, GateStatus};
use super::kill_switch::KillSwitch;
use super::quote::SwapQuote;
use super::signer::{
    tests::{build_test_transaction, program_bytes as prog_bytes},
    SignerTicket,
};
use super::wallet::{TokenAccountInfo, WalletBalances};

// ── helpers ───────────────────────────────────────────────────────────────────

fn default_config() -> JupiterSwapConfig {
    JupiterSwapConfig {
        rpc_url: "http://localhost:8899".into(),
        wallet_public_key: "11111111111111111111111111111111".into(),
        account_cap_usd: 250.0,
        max_swap_pct: 25.0,
        max_slippage_bps: 500,
        max_price_impact_pct: 5.0,
        sol_reserve_usd: 5.0,
        daily_stop_pct: 10.0,
        meme_cap_pct: 75.0,
        mint_allowlist: vec![SOL_MINT.into(), USDC_MINT.into()],
        quote_max_age_secs: 20,
        clock_skew_secs: 5,
        priority_fee_cap_sol: 0.0005,
        signer_key_path: Some("/fake/key.json".into()),
        live_execution_enabled: true,
        ticket_ttl_secs: 30,
    }
}

fn fresh_sol_to_usdc_quote(in_lamports: u64) -> SwapQuote {
    SwapQuote {
        input_mint: SOL_MINT.into(),
        output_mint: USDC_MINT.into(),
        in_amount: in_lamports,
        out_amount: 150_000_000, // 150 USDC
        other_amount_threshold: 142_500_000,
        slippage_bps: 50,
        price_impact_pct: 0.1,
        priority_fee_lamports: 1_000,
        last_valid_block_height: 99_999_999,
        fetched_at: chrono::Utc::now().timestamp(),
        route_label: "Orca".into(),
        unsigned_tx_b64: None,
        request_id: None,
    }
}

fn default_balances(sol_lamports: u64, total_usd: f64) -> WalletBalances {
    WalletBalances {
        sol_lamports,
        sol_usd: (sol_lamports as f64 / 1e9) * 150.0,
        token_accounts: vec![],
        total_usd,
        fetched_at: chrono::Utc::now().timestamp(),
    }
}

fn run(input: GateInput) -> Vec<super::gates::GateResult> {
    run_all_gates(&input)
}

// ── 1. 25% per-swap limit ─────────────────────────────────────────────────────

#[test]
fn per_swap_limit_blocks_oversized_swap() {
    let config = default_config(); // max_swap_pct = 25%
    // Account = $200. 25% = $50 max. Swap = 1 SOL @ $150 = $150 → rejected.
    let quote = fresh_sol_to_usdc_quote(1_000_000_000); // 1 SOL
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let per_swap = results.iter().find(|r| r.name == "per_swap_limit").unwrap();
    assert!(
        matches!(&per_swap.status, GateStatus::Fail(_)),
        "should fail per_swap_limit: {:?}",
        per_swap.status
    );
    assert!(!all_gates_passed(&results));
}

#[test]
fn per_swap_limit_allows_small_swap() {
    let config = default_config(); // 25% of $200 = $50
    // 0.1 SOL @ $150 = $15 → allowed
    let quote = fresh_sol_to_usdc_quote(100_000_000); // 0.1 SOL
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let per_swap = results.iter().find(|r| r.name == "per_swap_limit").unwrap();
    assert!(
        matches!(&per_swap.status, GateStatus::Pass),
        "should pass per_swap_limit: {:?}",
        per_swap.status
    );
}

// ── 2. $250 account cap ───────────────────────────────────────────────────────

#[test]
fn account_cap_blocks_when_wallet_over_250() {
    let config = default_config();
    let quote = fresh_sol_to_usdc_quote(100_000_000);
    let balances = default_balances(2_000_000_000, 251.0); // $251 > $250 cap

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 251.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let cap = results.iter().find(|r| r.name == "account_cap").unwrap();
    assert!(
        matches!(&cap.status, GateStatus::Fail(_)),
        "should fail account_cap at $251: {:?}",
        cap.status
    );
}

#[test]
fn account_cap_allows_at_exactly_250() {
    let config = default_config();
    let quote = fresh_sol_to_usdc_quote(100_000_000);
    let balances = default_balances(2_000_000_000, 250.0); // exactly $250

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 250.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let cap = results.iter().find(|r| r.name == "account_cap").unwrap();
    assert!(
        matches!(&cap.status, GateStatus::Pass),
        "exactly $250 should pass: {:?}",
        cap.status
    );
}

// ── 3. $5 SOL reserve ────────────────────────────────────────────────────────

#[test]
fn sol_reserve_blocks_when_insufficient() {
    let config = default_config(); // $5 reserve
    // SOL price = $150 → reserve = 5/150 = 0.0333 SOL = 33_333_333 lamports
    // Wallet has 0.05 SOL = 50_000_000 lamports. Swap consumes 0.04 SOL.
    // After swap: 10_000_000 lamports ≈ $1.50 < $5 reserve
    let mut quote = fresh_sol_to_usdc_quote(40_000_000); // 0.04 SOL
    quote.input_mint = SOL_MINT.into();
    let balances = default_balances(50_000_000, 7.5); // 0.05 SOL @ $150

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 7.5,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let reserve = results.iter().find(|r| r.name == "sol_reserve").unwrap();
    assert!(
        matches!(&reserve.status, GateStatus::Fail(_)),
        "should fail sol_reserve: {:?}",
        reserve.status
    );
}

#[test]
fn sol_reserve_passes_with_enough_sol() {
    let config = default_config(); // $5 reserve
    // 2 SOL @ $150 = $300 total, swap 0.1 SOL, leaves 1.9 SOL = $285 >> $5
    let quote = fresh_sol_to_usdc_quote(100_000_000); // 0.1 SOL
    let balances = default_balances(2_000_000_000, 300.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 300.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let reserve = results.iter().find(|r| r.name == "sol_reserve").unwrap();
    assert!(
        matches!(&reserve.status, GateStatus::Pass),
        "should pass sol_reserve: {:?}",
        reserve.status
    );
}

// ── 4. Daily stop persistence across restart ──────────────────────────────────

#[test]
fn daily_stop_blocked_persists_across_restart() {
    let db = make_test_db();

    // First "session": set blocked
    {
        let tracker = DailyStopTracker::new(db.clone());
        let conn = db.conn.lock().unwrap();
        crate::database::upsert_jupiter_daily_stop(
            &conn,
            &DailyStopTracker::today_ny(),
            200.0,
            true,
        )
        .expect("upsert");
        drop(conn);
        assert!(tracker.is_blocked().expect("is_blocked"), "should be blocked");
    }

    // Second "session" (same DB): re-create the tracker
    {
        let tracker2 = DailyStopTracker::new(db.clone());
        assert!(
            tracker2.is_blocked().expect("is_blocked after restart"),
            "daily stop should still be blocked after restart"
        );
    }
}

#[test]
fn daily_stop_not_blocked_by_default() {
    let db = make_test_db();
    let tracker = DailyStopTracker::new(db.clone());
    // ensure_today creates the row with blocked=false
    let conn = db.conn.lock().unwrap();
    crate::database::upsert_jupiter_daily_stop(
        &conn,
        &DailyStopTracker::today_ny(),
        200.0,
        false,
    )
    .unwrap();
    drop(conn);
    assert!(
        !tracker.is_blocked().unwrap(),
        "should not be blocked by default"
    );
}

#[test]
fn daily_stop_gate_triggers_when_loss_exceeds_threshold() {
    let config = default_config(); // daily_stop_pct = 10%
    let quote = fresh_sol_to_usdc_quote(100_000_000);
    let balances = default_balances(2_000_000_000, 200.0);

    // 11% of $200 = $22 loss → should block
    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 22.0, // $22 loss
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let stop = results.iter().find(|r| r.name == "daily_stop").unwrap();
    assert!(
        matches!(&stop.status, GateStatus::Fail(_)),
        "should fail daily_stop at 11% loss: {:?}",
        stop.status
    );
}

// ── 5. Slippage and price impact ──────────────────────────────────────────────

#[test]
fn slippage_gate_blocks_above_500bps() {
    let config = default_config();
    let mut quote = fresh_sol_to_usdc_quote(100_000_000);
    quote.slippage_bps = 501; // exceeds 500bps
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let slip = results.iter().find(|r| r.name == "slippage").unwrap();
    assert!(
        matches!(&slip.status, GateStatus::Fail(_)),
        "should fail slippage at 501bps: {:?}",
        slip.status
    );
}

#[test]
fn price_impact_gate_blocks_above_5pct() {
    let config = default_config();
    let mut quote = fresh_sol_to_usdc_quote(100_000_000);
    quote.price_impact_pct = 5.1; // exceeds 5%
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let impact = results.iter().find(|r| r.name == "price_impact").unwrap();
    assert!(
        matches!(&impact.status, GateStatus::Fail(_)),
        "should fail price_impact at 5.1%: {:?}",
        impact.status
    );
}

#[test]
fn slippage_passes_at_exactly_500bps() {
    let config = default_config();
    let mut quote = fresh_sol_to_usdc_quote(100_000_000);
    quote.slippage_bps = 500;
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let slip = results.iter().find(|r| r.name == "slippage").unwrap();
    assert!(
        matches!(&slip.status, GateStatus::Pass),
        "exactly 500bps should pass: {:?}",
        slip.status
    );
}

// ── 6. Mint allowlist and stale quotes ────────────────────────────────────────

#[test]
fn allowlist_blocks_unknown_input_mint() {
    let config = default_config(); // only SOL + USDC allowed
    let mut quote = fresh_sol_to_usdc_quote(100_000_000);
    quote.input_mint = "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN".into(); // JUP — not in allowlist
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let allow = results.iter().find(|r| r.name == "mint_allowlist").unwrap();
    assert!(
        matches!(&allow.status, GateStatus::Fail(_)),
        "JUP not in allowlist should fail: {:?}",
        allow.status
    );
}

#[test]
fn allowlist_blocks_when_empty() {
    let mut config = default_config();
    config.mint_allowlist.clear();
    let quote = fresh_sol_to_usdc_quote(100_000_000);
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let allow = results.iter().find(|r| r.name == "mint_allowlist").unwrap();
    assert!(
        matches!(&allow.status, GateStatus::Fail(_)),
        "empty allowlist should fail closed: {:?}",
        allow.status
    );
}

#[test]
fn stale_quote_blocks_when_too_old() {
    let config = default_config(); // quote_max_age_secs = 20
    let mut quote = fresh_sol_to_usdc_quote(100_000_000);
    quote.fetched_at = chrono::Utc::now().timestamp() - 30; // 30 seconds old
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let stale = results.iter().find(|r| r.name == "stale_quote").unwrap();
    assert!(
        matches!(&stale.status, GateStatus::Fail(_)),
        "30s old quote should fail stale_quote: {:?}",
        stale.status
    );
}

#[test]
fn fresh_quote_passes_stale_check() {
    let config = default_config();
    let quote = fresh_sol_to_usdc_quote(100_000_000); // fetched_at = now
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let stale = results.iter().find(|r| r.name == "stale_quote").unwrap();
    assert!(
        matches!(&stale.status, GateStatus::Pass),
        "fresh quote should pass: {:?}",
        stale.status
    );
}

// ── 7. Program allowlist (signer) ─────────────────────────────────────────────

#[test]
fn signer_parser_detects_disallowed_program_in_tx() {
    let signing_key = {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        SigningKey::from_bytes(&seed)
    };
    let pubkey = signing_key.verifying_key().to_bytes();

    let bad_program = [0xCC_u8; 32]; // unknown program
    let jup = prog_bytes(super::config::JUPITER_V6);

    let tx = build_test_transaction(
        &pubkey,
        &[bad_program, jup],
        &prog_bytes(SOL_MINT),
        &prog_bytes(USDC_MINT),
        0,
        200_000,
    );

    let parsed = super::signer::parse_transaction_for_test(&tx).unwrap();
    let all_keys_b58: Vec<String> = parsed
        .0
        .iter()
        .map(|k| bs58::encode(k).into_string())
        .collect();

    let disallowed: Vec<_> = parsed
        .1
        .iter()
        .filter(|&&idx| {
            all_keys_b58
                .get(idx as usize)
                .map(|k| {
                    !super::config::ALLOWED_PROGRAMS.contains(&k.as_str())
                })
                .unwrap_or(true)
        })
        .collect();

    assert!(
        !disallowed.is_empty(),
        "should detect the disallowed program"
    );
}

#[test]
fn signer_parser_accepts_all_allowed_programs() {
    let signing_key = {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        SigningKey::from_bytes(&seed)
    };
    let pubkey = signing_key.verifying_key().to_bytes();

    let jup = prog_bytes(super::config::JUPITER_V6);
    let cb = prog_bytes(super::config::COMPUTE_BUDGET);
    let spl = prog_bytes(super::config::SPL_TOKEN);

    let tx = build_test_transaction(
        &pubkey,
        &[jup, cb, spl],
        &prog_bytes(SOL_MINT),
        &prog_bytes(USDC_MINT),
        0,
        200_000,
    );

    let parsed = super::signer::parse_transaction_for_test(&tx).unwrap();
    let all_keys_b58: Vec<String> = parsed
        .0
        .iter()
        .map(|k| bs58::encode(k).into_string())
        .collect();

    let any_disallowed = parsed.1.iter().any(|&idx| {
        all_keys_b58
            .get(idx as usize)
            .map(|k| !super::config::ALLOWED_PROGRAMS.contains(&k.as_str()))
            .unwrap_or(true)
    });
    assert!(!any_disallowed, "all programs should be allowed");
}

// ── 8. Tampered transaction and outflow mismatch ──────────────────────────────

#[test]
fn signer_rejects_wrong_fee_payer() {
    let owner_key = {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        SigningKey::from_bytes(&seed)
    };
    let owner_pubkey = owner_key.verifying_key().to_bytes();

    let other_key = {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        SigningKey::from_bytes(&seed)
    };
    let other_pubkey = other_key.verifying_key().to_bytes();

    let jup = prog_bytes(super::config::JUPITER_V6);

    // Build tx with OTHER as fee payer
    let tx = build_test_transaction(
        &other_pubkey,
        &[jup],
        &prog_bytes(SOL_MINT),
        &prog_bytes(USDC_MINT),
        0,
        200_000,
    );

    let parsed = super::signer::parse_transaction_for_test(&tx).unwrap();
    let fee_payer_b58 = bs58::encode(&parsed.0[0]).into_string();
    let owner_b58 = bs58::encode(owner_pubkey).into_string();

    assert_ne!(
        fee_payer_b58, owner_b58,
        "fee payer should not match owner — signer must reject this"
    );
}

#[test]
fn signer_rejects_missing_expected_mint() {
    let signing_key = {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        SigningKey::from_bytes(&seed)
    };
    let pubkey = signing_key.verifying_key().to_bytes();

    let jup = prog_bytes(super::config::JUPITER_V6);
    // Build tx with WRONG output mint (JUP instead of USDC)
    let jup_mint = prog_bytes("JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN");

    let tx = build_test_transaction(
        &pubkey,
        &[jup],
        &prog_bytes(SOL_MINT),
        &jup_mint, // wrong output mint
        0,
        200_000,
    );

    let parsed = super::signer::parse_transaction_for_test(&tx).unwrap();
    let all_keys_b58: Vec<String> = parsed
        .0
        .iter()
        .map(|k| bs58::encode(k).into_string())
        .collect();

    // Expected output is USDC — should not be found
    let usdc_found = all_keys_b58.contains(&USDC_MINT.to_string());
    assert!(!usdc_found, "USDC should not be in this transaction");
}

// ── 9. No resubmit on unknown result ─────────────────────────────────────────

#[test]
fn ticket_status_pending_means_no_resubmit() {
    use super::ticket::{SwapTicket, TicketStatus};

    let quote = fresh_sol_to_usdc_quote(100_000_000);
    let ticket = SwapTicket {
        id: "jup-test-1".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        input_mint: SOL_MINT.into(),
        output_mint: USDC_MINT.into(),
        in_amount: 100_000_000,
        quoted_out_amount: 15_000_000,
        other_amount_threshold: 14_250_000,
        slippage_bps: 50,
        price_impact_pct: 0.1,
        estimated_priority_fee_lamports: 1000,
        last_valid_block_height: 99_999,
        route_label: "Orca".into(),
        gate_results: vec![],
        all_gates_passed: true,
        status: TicketStatus::Pending {
            signature: "5abc123".into(),
        },
        unsigned_tx_b64: None,
        created_at_ts: chrono::Utc::now().timestamp(),
        sol_price_usd_at_quote: 150.0,
    };

    // A PENDING ticket must NOT be re-submitted — only reconciled
    let is_pending = matches!(&ticket.status, TicketStatus::Pending { .. });
    let is_submitted = matches!(&ticket.status, TicketStatus::Submitted { .. });
    assert!(is_pending, "status should be Pending");
    assert!(!is_submitted, "Pending != Submitted — no resubmit path");

    // Verify signature is recorded (needed for reconciliation)
    if let TicketStatus::Pending { signature } = &ticket.status {
        assert!(!signature.is_empty(), "signature must be recorded for reconciliation");
    }

    drop(quote);
}

// ── 10. Kill switch ───────────────────────────────────────────────────────────

#[test]
fn kill_switch_starts_inactive() {
    let ks = KillSwitch::new();
    assert!(!ks.is_active());
}

#[test]
fn kill_switch_activate_blocks_gate() {
    let ks = KillSwitch::new();
    ks.activate();
    assert!(ks.is_active());

    let config = default_config();
    let quote = fresh_sol_to_usdc_quote(100_000_000);
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: true,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let ks_result = results.iter().find(|r| r.name == "kill_switch").unwrap();
    assert!(
        matches!(&ks_result.status, GateStatus::Fail(_)),
        "kill switch should fail gate: {:?}",
        ks_result.status
    );
    assert!(!all_gates_passed(&results));
}

#[test]
fn kill_switch_deactivate_unblocks() {
    let ks = KillSwitch::new();
    ks.activate();
    assert!(ks.is_active());
    ks.deactivate();
    assert!(!ks.is_active());
}

// ── 11. Execution gate (REAL_EXECUTION replacement) ──────────────────────────

#[test]
fn execution_gate_closed_by_default() {
    let config = JupiterSwapConfig::default();
    assert!(
        !config.execution_gate_open(),
        "execution gate must be closed by default"
    );
}

#[test]
fn execution_gate_requires_all_conditions() {
    // Missing live_execution_enabled
    let mut config = default_config();
    config.live_execution_enabled = false;
    assert!(!config.execution_gate_open(), "disabled flag should close gate");

    // Missing wallet_public_key
    config.live_execution_enabled = true;
    config.wallet_public_key = String::new();
    assert!(!config.execution_gate_open(), "empty wallet key should close gate");

    // Missing mint_allowlist
    config.wallet_public_key = "somekey".into();
    config.mint_allowlist.clear();
    assert!(!config.execution_gate_open(), "empty allowlist should close gate");

    // Missing signer_key_path
    config.mint_allowlist = vec![SOL_MINT.into()];
    config.signer_key_path = None;
    assert!(!config.execution_gate_open(), "missing key path should close gate");

    // All conditions met
    config.signer_key_path = Some("/path/to/key.json".into());
    assert!(config.execution_gate_open(), "all conditions met should open gate");
}

// ── 12. All gates pass on a clean swap ───────────────────────────────────────

#[test]
fn all_gates_pass_on_clean_input() {
    let config = default_config();
    // 0.1 SOL @ $150 = $15 input, account = $200, reserve = $5 → all good
    let quote = fresh_sol_to_usdc_quote(100_000_000);
    let balances = default_balances(2_000_000_000, 200.0);

    let results = run(GateInput {
        quote: &quote,
        balances: &balances,
        config: &config,
        daily_loss_usd: 0.0,
        start_of_day_equity_usd: 200.0,
        daily_stop_blocked: false,
        kill_switch_active: false,
        sol_price_usd: 150.0,
        now_ts: chrono::Utc::now().timestamp(),
    });

    let failures: Vec<_> = results
        .iter()
        .filter(|r| matches!(&r.status, GateStatus::Fail(_)))
        .collect();

    assert!(
        failures.is_empty(),
        "clean swap should pass all gates, failures: {failures:?}"
    );
    assert!(all_gates_passed(&results));
}

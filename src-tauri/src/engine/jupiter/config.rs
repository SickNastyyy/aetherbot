use serde::{Deserialize, Serialize};

// ── Mint constants ────────────────────────────────────────────────────────────

pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// ── Allowed program IDs ──────────────────────────────────────────────────────

pub const JUPITER_V6: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";
pub const SPL_TOKEN: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const TOKEN_2022: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
pub const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bNX";
pub const COMPUTE_BUDGET: &str = "ComputeBudget111111111111111111111111111111";
pub const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

pub const ALLOWED_PROGRAMS: &[&str] = &[
    JUPITER_V6,
    SPL_TOKEN,
    TOKEN_2022,
    ATA_PROGRAM,
    COMPUTE_BUDGET,
    SYSTEM_PROGRAM,
];

// ── JupiterSwapConfig ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterSwapConfig {
    /// Solana RPC URL — owner-configurable
    pub rpc_url: String,
    /// Owner's public wallet address (base58). No private key here.
    pub wallet_public_key: String,
    /// Hard cap on account USD value. Refuse any swap if wallet > this.
    pub account_cap_usd: f64,
    /// Max percent of current account value per single swap (25 = 25%).
    pub max_swap_pct: f64,
    /// Max slippage in bps (500 = 5%).
    pub max_slippage_bps: u32,
    /// Max price impact % allowed (5.0).
    pub max_price_impact_pct: f64,
    /// Minimum SOL reserve in USD. Swap outflow + fees must never breach it.
    pub sol_reserve_usd: f64,
    /// Daily stop loss %. Block all swaps once realized + unrealized loss
    /// reaches this % of start-of-day account value.
    pub daily_stop_pct: f64,
    /// Aggregate meme exposure cap %. Default 75%. ⚠️ OWNER MUST CONFIRM.
    pub meme_cap_pct: f64,
    /// Exact mint addresses only. Empty = fail closed. SOL + USDC prefilled.
    pub mint_allowlist: Vec<String>,
    /// Maximum quote age in seconds (20).
    pub quote_max_age_secs: u64,
    /// Clock skew tolerance in seconds (5).
    pub clock_skew_secs: u64,
    /// Priority fee cap in SOL (0.0005).
    pub priority_fee_cap_sol: f64,
    /// Path to keypair JSON file. Read ONLY at sign time. Never log or store.
    pub signer_key_path: Option<String>,
    /// Master execution switch. Defaults to false.
    /// Only enable after all other conditions are met.
    pub live_execution_enabled: bool,
}

impl Default for JupiterSwapConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".into(),
            wallet_public_key: String::new(),
            account_cap_usd: 250.0,
            max_swap_pct: 25.0,
            max_slippage_bps: 500,
            max_price_impact_pct: 5.0,
            sol_reserve_usd: 5.0,
            daily_stop_pct: 10.0,
            meme_cap_pct: 75.0, // ⚠️ needs owner confirmation
            mint_allowlist: vec![SOL_MINT.into(), USDC_MINT.into()],
            quote_max_age_secs: 20,
            clock_skew_secs: 5,
            priority_fee_cap_sol: 0.0005,
            signer_key_path: None,
            live_execution_enabled: false,
        }
    }
}

impl JupiterSwapConfig {
    /// Returns true only when ALL of:
    ///   1. live_execution_enabled = true (owner must set explicitly)
    ///   2. wallet_public_key is set
    ///   3. mint_allowlist is non-empty
    ///   4. signer_key_path is set
    /// Default stays false.
    pub fn execution_gate_open(&self) -> bool {
        self.live_execution_enabled
            && !self.wallet_public_key.is_empty()
            && !self.mint_allowlist.is_empty()
            && self.signer_key_path.is_some()
    }

    pub fn load(data_dir: &std::path::Path) -> Result<Self, String> {
        let path = data_dir.join("jupiter_config.json");
        if path.exists() {
            let json = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read Jupiter config: {e}"))?;
            serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse Jupiter config: {e}"))
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, data_dir: &std::path::Path) -> Result<(), String> {
        let path = data_dir.join("jupiter_config.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialize error: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("Write error: {e}"))
    }
}

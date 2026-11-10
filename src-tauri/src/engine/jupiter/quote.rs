use reqwest::Client;
use serde::{Deserialize, Serialize};

// ── SwapQuote ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapQuote {
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: u64,
    pub out_amount: u64,
    /// Minimum output after slippage
    pub other_amount_threshold: u64,
    pub slippage_bps: u32,
    /// Price impact as a percentage (0.0 – 100.0).
    /// Set to 100.0 (fail-closed) when the field is missing from the API response.
    pub price_impact_pct: f64,
    /// Estimated priority fee in lamports (from Jupiter response)
    pub priority_fee_lamports: u64,
    /// Last valid block height for the transaction
    pub last_valid_block_height: u64,
    /// Unix timestamp when this quote was fetched (for age checks)
    pub fetched_at: i64,
    /// Route description for display
    pub route_label: String,
    /// Base64-encoded unsigned transaction from /order response (with taker set).
    /// This is the EXACT transaction that will be verified and signed.
    pub unsigned_tx_b64: Option<String>,
    /// Request ID for the /execute endpoint (if provided by Jupiter).
    pub request_id: Option<String>,
}

// ── JupiterQuoteClient ────────────────────────────────────────────────────────

pub struct JupiterQuoteClient {
    client: Client,
    base_url: String,
}

impl JupiterQuoteClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("HTTP client"),
            base_url: "https://api.jup.ag/swap/v2".into(),
        }
    }

    /// GET /swap/v2/order — fetch a swap quote.
    /// `taker` is the owner's public key (base58).
    pub async fn get_quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        in_amount: u64,
        slippage_bps: u32,
        taker: &str,
    ) -> Result<SwapQuote, String> {
        let url = format!(
            "{}/order?inputMint={}&outputMint={}&amount={}&slippageBps={}&taker={}",
            self.base_url, input_mint, output_mint, in_amount, slippage_bps, taker,
        );

        let resp: serde_json::Value = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Jupiter quote request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Jupiter quote parse failed: {e}"))?;

        self.parse_quote_response(resp, input_mint, output_mint, in_amount, slippage_bps)
    }

    fn parse_quote_response(
        &self,
        resp: serde_json::Value,
        input_mint: &str,
        output_mint: &str,
        in_amount: u64,
        slippage_bps: u32,
    ) -> Result<SwapQuote, String> {
        // Handle possible error response
        if let Some(err) = resp["error"].as_str() {
            return Err(format!("Jupiter API error: {err}"));
        }

        // Try top-level and nested "order" key
        let data = if resp["outAmount"].is_string() || resp["outAmount"].is_number() {
            &resp
        } else if resp["order"]["outAmount"].is_string() || resp["order"]["outAmount"].is_number() {
            &resp["order"]
        } else {
            return Err(format!("Unexpected Jupiter quote response: {resp}"));
        };

        let out_amount = parse_u64_field(data, "outAmount")?;
        let other_amount_threshold = parse_u64_field(data, "otherAmountThreshold")
            .unwrap_or_else(|_| out_amount.saturating_sub(out_amount / 100));

        // Fail closed: treat a missing/null price impact as 100% to block the swap.
        let price_impact_pct: f64 = data["priceImpactPct"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| data["priceImpactPct"].as_f64())
            .unwrap_or(100.0);

        let priority_fee_lamports: u64 = data["prioritizationFeeLamports"]
            .as_u64()
            .unwrap_or(0);

        let last_valid_block_height: u64 = data["lastValidBlockHeight"]
            .as_u64()
            .unwrap_or(0);

        // Build a human-readable route label
        let route_label = data["routePlan"]
            .as_array()
            .and_then(|r| r.first())
            .and_then(|step| step["swapInfo"]["label"].as_str())
            .unwrap_or("Jupiter")
            .to_string();

        // The /order endpoint (with taker set) includes the unsigned transaction directly.
        let unsigned_tx_b64 = resp["transaction"]
            .as_str()
            .map(|s| s.to_string());

        let request_id = resp["requestId"]
            .as_str()
            .map(|s| s.to_string());

        Ok(SwapQuote {
            input_mint: input_mint.to_string(),
            output_mint: output_mint.to_string(),
            in_amount,
            out_amount,
            other_amount_threshold,
            slippage_bps,
            price_impact_pct,
            priority_fee_lamports,
            last_valid_block_height,
            fetched_at: chrono::Utc::now().timestamp(),
            route_label,
            unsigned_tx_b64,
            request_id,
        })
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_u64_field(data: &serde_json::Value, key: &str) -> Result<u64, String> {
    if let Some(s) = data[key].as_str() {
        s.parse::<u64>()
            .map_err(|_| format!("Cannot parse {key}={s} as u64"))
    } else if let Some(n) = data[key].as_u64() {
        Ok(n)
    } else {
        Err(format!("Missing or invalid field: {key}"))
    }
}

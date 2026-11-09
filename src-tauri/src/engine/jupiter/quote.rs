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
    /// Price impact as a percentage (0.0 – 100.0)
    pub price_impact_pct: f64,
    /// Estimated priority fee in lamports (from Jupiter response)
    pub priority_fee_lamports: u64,
    /// Last valid block height for the transaction
    pub last_valid_block_height: u64,
    /// Unix timestamp when this quote was fetched (for age checks)
    pub fetched_at: i64,
    /// Route description for display
    pub route_label: String,
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

        let price_impact_pct: f64 = data["priceImpactPct"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .or_else(|| data["priceImpactPct"].as_f64())
            .unwrap_or(0.0);

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
        })
    }

    /// POST /swap/v2/swap — get the unsigned base64-encoded transaction for
    /// the previously quoted swap. Called AFTER owner approval (step 4 in
    /// the flow). Returns (base64_tx, last_valid_block_height).
    pub async fn get_swap_transaction(
        &self,
        quote: &SwapQuote,
        user_public_key: &str,
        priority_fee_lamports: Option<u64>,
    ) -> Result<(String, u64), String> {
        let body = serde_json::json!({
            "quoteResponse": {
                "inputMint": quote.input_mint,
                "inAmount": quote.in_amount.to_string(),
                "outputMint": quote.output_mint,
                "outAmount": quote.out_amount.to_string(),
                "otherAmountThreshold": quote.other_amount_threshold.to_string(),
                "swapMode": "ExactIn",
                "slippageBps": quote.slippage_bps,
                "priceImpactPct": quote.price_impact_pct.to_string(),
                "routePlan": [],
                "contextSlot": 0,
                "timeTaken": 0.0
            },
            "userPublicKey": user_public_key,
            "wrapAndUnwrapSol": true,
            "computeUnitPriceMicroLamports": priority_fee_lamports.map(|f| f * 1_000).unwrap_or(1000),
            "asLegacyTransaction": false
        });

        let url = format!("{}/swap", self.base_url);
        let resp: serde_json::Value = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Jupiter swap request failed: {e}"))?
            .json()
            .await
            .map_err(|e| format!("Jupiter swap parse failed: {e}"))?;

        if let Some(err) = resp["error"].as_str() {
            return Err(format!("Jupiter swap API error: {err}"));
        }

        let tx_b64 = resp["swapTransaction"]
            .as_str()
            .ok_or_else(|| format!("Missing swapTransaction in Jupiter response: {resp}"))?
            .to_string();

        let last_valid = resp["lastValidBlockHeight"]
            .as_u64()
            .unwrap_or(quote.last_valid_block_height);

        Ok((tx_b64, last_valid))
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

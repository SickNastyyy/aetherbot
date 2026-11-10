use reqwest::Client;
use serde_json::json;
use bs58;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WalletBalances {
    pub sol_lamports: u64,
    pub sol_usd: f64,
    pub token_accounts: Vec<TokenAccountInfo>,
    pub total_usd: f64,
    /// Unix timestamp when this snapshot was taken
    pub fetched_at: i64,
}

#[derive(Debug, Clone)]
pub struct TokenAccountInfo {
    pub mint: String,
    pub amount_atoms: u64,
    /// USD value if a price was supplied externally
    pub amount_usd: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct SignatureStatus {
    pub slot: u64,
    pub confirmed: bool,
    /// null if no error, otherwise the transaction error object
    pub err: serde_json::Value,
}

// ── WalletReader ─────────────────────────────────────────────────────────────

/// Read-only wallet reader. Accepts only a PUBLIC key. No private key involved.
pub struct WalletReader {
    client: Client,
    rpc_url: String,
}

impl WalletReader {
    pub fn new(rpc_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("HTTP client"),
            rpc_url,
        }
    }

    /// Fetch SOL balance + SPL token accounts for the given public key.
    /// `sol_price_usd` is used to compute USD values.
    pub async fn read_balances(
        &self,
        public_key: &str,
        sol_price_usd: f64,
    ) -> Result<WalletBalances, String> {
        let sol_lamports = self.get_sol_balance(public_key).await?;
        let token_accounts = self.get_token_accounts(public_key).await?;

        let sol_usd = (sol_lamports as f64 / 1_000_000_000.0) * sol_price_usd;
        let token_usd: f64 = token_accounts.iter().filter_map(|t| t.amount_usd).sum();

        Ok(WalletBalances {
            sol_lamports,
            sol_usd,
            token_accounts,
            total_usd: sol_usd + token_usd,
            fetched_at: chrono::Utc::now().timestamp(),
        })
    }

    async fn get_sol_balance(&self, public_key: &str) -> Result<u64, String> {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getBalance",
            "params": [public_key, {"commitment": "confirmed"}]
        });
        let resp: serde_json::Value = self
            .client
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("RPC getBalance: {e}"))?
            .json()
            .await
            .map_err(|e| format!("RPC parse: {e}"))?;

        resp["result"]["value"]
            .as_u64()
            .ok_or_else(|| "Missing SOL balance in RPC response".into())
    }

    async fn get_token_accounts(
        &self,
        public_key: &str,
    ) -> Result<Vec<TokenAccountInfo>, String> {
        // Query both SPL Token and Token-2022 program IDs
        let programs = [
            "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
            "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        ];
        let mut all_accounts = Vec::new();

        for program_id in &programs {
            let req = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getTokenAccountsByOwner",
                "params": [
                    public_key,
                    {"programId": program_id},
                    {"encoding": "jsonParsed", "commitment": "confirmed"}
                ]
            });
            let resp: serde_json::Value = match self
                .client
                .post(&self.rpc_url)
                .json(&req)
                .send()
                .await
            {
                Ok(r) => match r.json().await {
                    Ok(v) => v,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            if let Some(accounts) = resp["result"]["value"].as_array() {
                for acct in accounts {
                    let parsed = &acct["account"]["data"]["parsed"]["info"];
                    if let (Some(mint), Some(amount_str)) = (
                        parsed["mint"].as_str(),
                        parsed["tokenAmount"]["amount"].as_str(),
                    ) {
                        let amount_atoms: u64 = amount_str.parse().unwrap_or(0);
                        // Price USDC at $1 (6 decimals). All other tokens left as None.
                        const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
                        let amount_usd = if mint == USDC_MINT {
                            Some(amount_atoms as f64 / 1_000_000.0)
                        } else {
                            None
                        };
                        all_accounts.push(TokenAccountInfo {
                            mint: mint.to_string(),
                            amount_atoms,
                            amount_usd,
                        });
                    }
                }
            }
        }

        Ok(all_accounts)
    }

    /// Fetch the current confirmed block height.
    /// Used for blockhash expiry checks. `lastValidBlockHeight` from Jupiter
    /// must be compared against block height, NOT slot.
    pub async fn get_current_block_height(&self) -> Result<u64, String> {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getBlockHeight",
            "params": [{"commitment": "confirmed"}]
        });
        let resp: serde_json::Value = self
            .client
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("RPC getBlockHeight: {e}"))?
            .json()
            .await
            .map_err(|e| format!("RPC parse: {e}"))?;

        resp["result"]
            .as_u64()
            .ok_or_else(|| "Missing block height in RPC response — failing closed".into())
    }

    /// Fetch the full account list for a Solana address lookup table.
    /// Returns the ordered list of pubkeys stored in the table.
    /// Fails closed: if the RPC call fails or the account is not an ALT, returns Err.
    pub async fn get_lookup_table_addresses(
        &self,
        table_address: &str,
    ) -> Result<Vec<[u8; 32]>, String> {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                table_address,
                {"encoding": "jsonParsed", "commitment": "confirmed"}
            ]
        });
        let resp: serde_json::Value = self
            .client
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("RPC getAccountInfo(ALT {table_address}): {e}"))?
            .json()
            .await
            .map_err(|e| format!("RPC parse(ALT): {e}"))?;

        let addresses = resp["result"]["value"]["data"]["parsed"]["info"]["addresses"]
            .as_array()
            .ok_or_else(|| {
                format!("ALT {table_address}: not a parsed lookup table or account not found")
            })?;

        addresses
            .iter()
            .map(|v| {
                let b58 = v.as_str().ok_or_else(|| "ALT address entry is not a string".to_string())?;
                let bytes = bs58::decode(b58)
                    .into_vec()
                    .map_err(|e| format!("ALT address decode: {e}"))?;
                if bytes.len() != 32 {
                    return Err(format!("ALT address {b58} is not 32 bytes"));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                Ok(arr)
            })
            .collect()
    }

    /// Check the status of a submitted transaction signature.
    /// Returns None if the signature is not yet known to the cluster.
    pub async fn get_signature_status(
        &self,
        signature: &str,
    ) -> Result<Option<SignatureStatus>, String> {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSignatureStatuses",
            "params": [[signature], {"searchTransactionHistory": true}]
        });
        let resp: serde_json::Value = self
            .client
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("RPC getSignatureStatuses: {e}"))?
            .json()
            .await
            .map_err(|e| format!("RPC parse: {e}"))?;

        let status = &resp["result"]["value"][0];
        if status.is_null() {
            return Ok(None);
        }
        let slot = status["slot"].as_u64().unwrap_or(0);
        let err = status["err"].clone();
        let confirmed = matches!(
            status["confirmationStatus"].as_str(),
            Some("confirmed") | Some("finalized")
        );

        Ok(Some(SignatureStatus { slot, confirmed, err }))
    }

    /// Submit a signed transaction (base64-encoded) to the RPC.
    /// Returns the transaction signature string on success.
    pub async fn send_transaction(&self, signed_tx_b64: &str) -> Result<String, String> {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                signed_tx_b64,
                {
                    "encoding": "base64",
                    "skipPreflight": false,
                    "preflightCommitment": "confirmed",
                    "maxRetries": 0
                }
            ]
        });
        let resp: serde_json::Value = self
            .client
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("RPC sendTransaction: {e}"))?
            .json()
            .await
            .map_err(|e| format!("RPC parse: {e}"))?;

        if let Some(err) = resp["error"].as_object() {
            return Err(format!("RPC error: {err:?}"));
        }

        resp["result"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing signature in sendTransaction response".into())
    }

    /// Simulate a transaction (base64-encoded). Returns Ok if simulation passes.
    pub async fn simulate_transaction(&self, tx_b64: &str) -> Result<(), String> {
        let req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "simulateTransaction",
            "params": [
                tx_b64,
                {
                    "encoding": "base64",
                    "commitment": "confirmed",
                    "sigVerify": false
                }
            ]
        });
        let resp: serde_json::Value = self
            .client
            .post(&self.rpc_url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("RPC simulateTransaction: {e}"))?
            .json()
            .await
            .map_err(|e| format!("RPC parse: {e}"))?;

        if let Some(err) = resp["result"]["value"]["err"].as_object() {
            return Err(format!("Simulation error: {err:?}"));
        }
        if !resp["result"]["value"]["err"].is_null() {
            return Err(format!(
                "Simulation error: {}",
                resp["result"]["value"]["err"]
            ));
        }

        Ok(())
    }
}

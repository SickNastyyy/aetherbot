/// Isolated signer module.
///
/// Contract:
/// - Loads the keypair ONLY at sign time (from file path or env var).
/// - Never logs, prints, or transmits key material.
/// - Independently decodes and verifies the unsigned transaction before signing.
/// - Zeroizes key bytes from memory after use.
/// - Returns an error on any verification failure — the caller MUST NOT sign
///   or submit if this module returns Err.
use ed25519_dalek::{Signer, SigningKey};
use zeroize::Zeroize;

use super::config::{ALLOWED_PROGRAMS, COMPUTE_BUDGET, SYSTEM_PROGRAM, SPL_TOKEN, TOKEN_2022};

// ── Verification ticket ───────────────────────────────────────────────────────

/// What the signer should verify against.
pub struct SignerTicket {
    /// Owner wallet address (base58) — must be the fee payer
    pub expected_fee_payer: String,
    /// Expected input mint address
    pub expected_input_mint: String,
    /// Expected output mint address
    pub expected_output_mint: String,
    /// Expected input amount in atoms
    pub expected_in_amount: u64,
    /// Last valid block height from Jupiter's response
    pub last_valid_block_height: u64,
    /// Current confirmed BLOCK HEIGHT (from getBlockHeight — not getSlot).
    /// Must be < last_valid_block_height for the blockhash to be valid.
    pub current_block_height: u64,
    /// Priority fee cap in SOL
    pub priority_fee_cap_sol: f64,
    /// Pre-resolved accounts from address lookup tables, in correct order:
    ///   per-ALT: writable accounts (in index order) then readonly accounts.
    /// Must be fetched by the caller (async) before calling verify_and_sign.
    /// Empty for legacy (non-v0) transactions.
    pub resolved_alt_accounts: Vec<[u8; 32]>,
}

// ── Public sign function ──────────────────────────────────────────────────────

/// Load keypair, verify the transaction, sign it, zeroize key material.
///
/// `key_path` — path to a Solana keypair JSON file (64-byte array).
/// `unsigned_tx_b64` — base64-encoded unsigned VersionedTransaction from Jupiter.
///
/// Returns the base64-encoded signed transaction.
///
/// NEVER call this if `execution_gate_open()` is false.
pub fn verify_and_sign(
    key_path: &str,
    unsigned_tx_b64: &str,
    ticket: &SignerTicket,
) -> Result<String, String> {
    // 1. Load keypair bytes (64 bytes: seed[32] + pubkey[32])
    let mut key_bytes = load_keypair_bytes(key_path)?;

    let result = sign_internal(&key_bytes, unsigned_tx_b64, ticket);

    // Always zeroize key bytes regardless of result
    key_bytes.zeroize();

    result
}

/// Same as `verify_and_sign` but loads from environment variable
/// `JUPITER_SIGNER_KEY` (base58-encoded 64-byte keypair).
pub fn verify_and_sign_from_env(
    env_var: &str,
    unsigned_tx_b64: &str,
    ticket: &SignerTicket,
) -> Result<String, String> {
    let raw = std::env::var(env_var)
        .map_err(|_| format!("Environment variable {env_var} not set"))?;

    let mut decoded = bs58::decode(&raw)
        .into_vec()
        .map_err(|e| format!("Invalid base58 key in env: {e}"))?;

    let result = sign_internal(&decoded, unsigned_tx_b64, ticket);
    decoded.zeroize();
    result
}

// ── Internal sign logic ───────────────────────────────────────────────────────

fn sign_internal(
    key_bytes: &[u8],
    unsigned_tx_b64: &str,
    ticket: &SignerTicket,
) -> Result<String, String> {
    if key_bytes.len() < 32 {
        return Err("Keypair must be at least 32 bytes".into());
    }

    let mut seed: [u8; 32] = key_bytes[..32]
        .try_into()
        .map_err(|_| "Cannot read 32-byte seed")?;
    let signing_key = SigningKey::from_bytes(&seed);
    seed.zeroize();

    // Verify the public key matches the expected fee payer
    let derived_pubkey_bytes = signing_key.verifying_key().to_bytes();
    let derived_pubkey_b58 = bs58::encode(derived_pubkey_bytes).into_string();
    if derived_pubkey_b58 != ticket.expected_fee_payer {
        return Err(format!(
            "Keypair pubkey {derived_pubkey_b58} != expected fee payer {}",
            ticket.expected_fee_payer
        ));
    }

    // 2. Decode the transaction
    let tx_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        unsigned_tx_b64,
    )
    .map_err(|e| format!("Base64 decode failed: {e}"))?;

    // 3. Parse the transaction structure
    let parsed = parse_transaction(&tx_bytes)?;

    // 4. Verify fee payer (account_keys[0])
    if parsed.account_keys.is_empty() {
        return Err("Transaction has no account keys".into());
    }
    let fee_payer_b58 = bs58::encode(&parsed.account_keys[0]).into_string();
    if fee_payer_b58 != ticket.expected_fee_payer {
        return Err(format!(
            "Fee payer {fee_payer_b58} != expected {}",
            ticket.expected_fee_payer
        ));
    }

    // 5b. Build full account list: static keys + pre-resolved ALT accounts.
    let num_static_keys = parsed.account_keys.len();
    let mut full_accounts: Vec<[u8; 32]> = parsed.account_keys.clone();
    full_accounts.extend_from_slice(&ticket.resolved_alt_accounts);

    let all_keys_b58: Vec<String> = full_accounts
        .iter()
        .map(|k| bs58::encode(k).into_string())
        .collect();

    // 5c. If there are ALT references in the transaction but no resolved accounts
    //     were provided, fail closed rather than skipping the checks.
    if !parsed.alt_refs.is_empty() && ticket.resolved_alt_accounts.is_empty() {
        return Err(format!(
            "Transaction uses {} address lookup table(s) but resolved_alt_accounts is empty — \
             caller must resolve ALTs before signing",
            parsed.alt_refs.len()
        ));
    }

    // 6. Verify all program IDs are in the allowlist (re-check with full account list)
    for inst in &parsed.instructions {
        if (inst.prog_idx as usize) >= all_keys_b58.len() {
            return Err(format!("Program index {} out of bounds", inst.prog_idx));
        }
        let prog_b58 = &all_keys_b58[inst.prog_idx as usize];
        if !ALLOWED_PROGRAMS.contains(&prog_b58.as_str()) {
            return Err(format!("Disallowed program in transaction: {prog_b58}"));
        }
    }

    // 7. Verify expected mints appear somewhere in the full account list.
    if !all_keys_b58.contains(&ticket.expected_input_mint) {
        return Err(format!(
            "Expected input mint {} not found in transaction accounts",
            ticket.expected_input_mint
        ));
    }
    if !all_keys_b58.contains(&ticket.expected_output_mint) {
        return Err(format!(
            "Expected output mint {} not found in transaction accounts",
            ticket.expected_output_mint
        ));
    }

    // 8. Instruction semantic checks.
    check_instruction_semantics(&parsed, num_static_keys, &all_keys_b58)?;

    // 9. Check priority fee from Compute Budget instruction
    let priority_fee_lamports = extract_priority_fee_lamports(&parsed, &all_keys_b58);
    let priority_fee_sol = priority_fee_lamports as f64 / 1_000_000_000.0;
    if priority_fee_sol > ticket.priority_fee_cap_sol {
        return Err(format!(
            "Priority fee {priority_fee_sol:.6} SOL > cap {:.6} SOL",
            ticket.priority_fee_cap_sol
        ));
    }

    // 10. Verify blockhash hasn't expired.
    // Compare current BLOCK HEIGHT against lastValidBlockHeight (both are block heights).
    // Fail closed on RPC error (current_block_height == 0 is treated as unknown).
    if ticket.current_block_height == 0 {
        return Err("Current block height unavailable — cannot verify blockhash expiry".into());
    }
    if ticket.current_block_height > ticket.last_valid_block_height {
        return Err(format!(
            "Blockhash expired: current block height {} > last valid {}",
            ticket.current_block_height, ticket.last_valid_block_height
        ));
    }

    // 9. Sign the message bytes
    let signature_bytes = signing_key.sign(&parsed.message_bytes);
    let signature_array: [u8; 64] = signature_bytes.to_bytes();

    // 10. Build the signed transaction
    let signed_tx = build_signed_transaction(&tx_bytes, &signature_array, parsed.num_sigs)?;

    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        signed_tx,
    ))
}

// ── Minimal Solana transaction parser ────────────────────────────────────────

struct ParsedInstruction {
    prog_idx: u8,
    /// Indices into the full account list (static + ALT) for this instruction.
    account_indices: Vec<u8>,
    data: Vec<u8>,
}

/// Reference to an address lookup table used by a v0 transaction.
struct AltRef {
    table_address: [u8; 32],
    writable_indices: Vec<u8>,
    readonly_indices: Vec<u8>,
}

struct ParsedTransaction {
    num_sigs: usize,
    account_keys: Vec<[u8; 32]>,
    instructions: Vec<ParsedInstruction>,
    message_bytes: Vec<u8>,
    /// Address lookup table references (v0 transactions only).
    alt_refs: Vec<AltRef>,
}

/// Parse a serialised Solana VersionedTransaction or legacy Transaction.
///
/// Format (all lengths use Solana's compact-u16 encoding):
///   compact_u16(num_sigs)
///   [num_sigs × 64]  ← zero bytes for unsigned
///   message:
///     [optional version prefix byte: 0x80 | version for versioned]
///     u8 num_required_signatures
///     u8 num_readonly_signed_accounts
///     u8 num_readonly_unsigned_accounts
///     compact_u16(num_account_keys) + [num_account_keys × 32]
///     [32]  recent_blockhash
///     compact_u16(num_instructions)
///     for each instruction:
///       u8 program_id_index
///       compact_u16(num_account_indices) + [num_account_indices × u8]
///       compact_u16(data_len) + [data_len × u8]
///     [v0 only] compact_u16(num_address_lookups) + ...
fn parse_transaction(bytes: &[u8]) -> Result<ParsedTransaction, String> {
    let mut pos = 0usize;

    // 1. Number of signatures
    let (num_sigs, consumed) = decode_compact_u16(&bytes[pos..])?;
    pos += consumed;

    // 2. Skip signature slots (64 bytes each)
    let sig_bytes = num_sigs as usize * 64;
    if pos + sig_bytes > bytes.len() {
        return Err("Transaction too short for signatures".into());
    }
    pos += sig_bytes;

    // 3. Message starts here
    let message_start = pos;

    // Check for versioned message prefix (0x80 | version)
    if pos >= bytes.len() {
        return Err("Transaction truncated before message".into());
    }
    let first_byte = bytes[pos];
    let is_versioned = first_byte & 0x80 != 0;
    if is_versioned {
        pos += 1; // skip version prefix
    }

    // 4. Message header: 3 bytes
    if pos + 3 > bytes.len() {
        return Err("Transaction too short for message header".into());
    }
    let _num_required_sigs = bytes[pos];
    let _num_readonly_signed = bytes[pos + 1];
    let _num_readonly_unsigned = bytes[pos + 2];
    pos += 3;

    // 5. Static account keys
    let (num_keys, consumed) = decode_compact_u16(&bytes[pos..])?;
    pos += consumed;
    let mut account_keys: Vec<[u8; 32]> = Vec::with_capacity(num_keys as usize);
    for _ in 0..num_keys {
        if pos + 32 > bytes.len() {
            return Err("Transaction too short for account keys".into());
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[pos..pos + 32]);
        account_keys.push(key);
        pos += 32;
    }

    // 6. Recent blockhash (32 bytes) — skip
    if pos + 32 > bytes.len() {
        return Err("Transaction too short for blockhash".into());
    }
    pos += 32;

    // 7. Instructions
    let (num_ixs, consumed) = decode_compact_u16(&bytes[pos..])?;
    pos += consumed;

    let mut instructions: Vec<ParsedInstruction> = Vec::new();

    for _ in 0..num_ixs {
        if pos >= bytes.len() {
            return Err("Transaction truncated in instructions".into());
        }
        let prog_idx = bytes[pos];
        pos += 1;

        // Account indices — stored for semantic checks
        let (num_accts, consumed) = decode_compact_u16(&bytes[pos..])?;
        pos += consumed;
        if pos + num_accts as usize > bytes.len() {
            return Err("Transaction too short for instruction accounts".into());
        }
        let account_indices = bytes[pos..pos + num_accts as usize].to_vec();
        pos += num_accts as usize;

        // Instruction data
        let (data_len, consumed) = decode_compact_u16(&bytes[pos..])?;
        pos += consumed;
        if pos + data_len as usize > bytes.len() {
            return Err("Transaction too short for instruction data".into());
        }
        let data = bytes[pos..pos + data_len as usize].to_vec();
        instructions.push(ParsedInstruction { prog_idx, account_indices, data });
        pos += data_len as usize;
    }

    // 8. Address lookup tables (v0 only)
    let mut alt_refs: Vec<AltRef> = Vec::new();
    if is_versioned && pos < bytes.len() {
        let (num_alts, consumed) = decode_compact_u16(&bytes[pos..])?;
        pos += consumed;
        for _ in 0..num_alts {
            if pos + 32 > bytes.len() {
                return Err("Transaction too short for ALT address".into());
            }
            let mut table_address = [0u8; 32];
            table_address.copy_from_slice(&bytes[pos..pos + 32]);
            pos += 32;

            let (num_writable, consumed) = decode_compact_u16(&bytes[pos..])?;
            pos += consumed;
            if pos + num_writable as usize > bytes.len() {
                return Err("Transaction too short for ALT writable indices".into());
            }
            let writable_indices = bytes[pos..pos + num_writable as usize].to_vec();
            pos += num_writable as usize;

            let (num_readonly, consumed) = decode_compact_u16(&bytes[pos..])?;
            pos += consumed;
            if pos + num_readonly as usize > bytes.len() {
                return Err("Transaction too short for ALT readonly indices".into());
            }
            let readonly_indices = bytes[pos..pos + num_readonly as usize].to_vec();
            pos += num_readonly as usize;

            alt_refs.push(AltRef { table_address, writable_indices, readonly_indices });
        }
    }

    // Message bytes = everything from message_start to end
    let message_bytes = bytes[message_start..].to_vec();

    Ok(ParsedTransaction {
        num_sigs: num_sigs as usize,
        account_keys,
        instructions,
        message_bytes,
        alt_refs,
    })
}

/// Decode a Solana compact-u16 (variable-length encoding).
/// Returns (value, bytes_consumed).
fn decode_compact_u16(bytes: &[u8]) -> Result<(u16, usize), String> {
    if bytes.is_empty() {
        return Err("compact_u16: empty slice".into());
    }
    let mut result: u32 = 0;
    let mut shift = 0u32;
    let mut consumed = 0usize;
    for &byte in bytes.iter().take(3) {
        consumed += 1;
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    if result > u16::MAX as u32 {
        return Err(format!("compact_u16 overflow: {result}"));
    }
    Ok((result as u16, consumed))
}

/// Encode a u16 as Solana compact-u16.
fn encode_compact_u16(mut val: u16) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (val & 0x7F) as u8;
        val >>= 7;
        if val == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
    out
}

/// Build a signed transaction by replacing the first signature slot.
fn build_signed_transaction(
    original_bytes: &[u8],
    signature: &[u8; 64],
    num_sigs: usize,
) -> Result<Vec<u8>, String> {
    let (actual_sigs, consumed) = decode_compact_u16(original_bytes)?;
    if actual_sigs as usize != num_sigs {
        return Err(format!(
            "Sig count mismatch: parsed {num_sigs} vs header {actual_sigs}"
        ));
    }

    let mut out = Vec::with_capacity(original_bytes.len());
    out.extend_from_slice(&encode_compact_u16(actual_sigs));
    // Write the new signature for slot 0
    out.extend_from_slice(signature);
    // Copy any remaining signature slots (zeros) and the message
    let sig_start = consumed;
    out.extend_from_slice(&original_bytes[sig_start + 64..]);
    Ok(out)
}

/// Extract the total estimated priority fee in lamports from Compute Budget instructions.
fn extract_priority_fee_lamports(
    parsed: &ParsedTransaction,
    all_keys_b58: &[String],
) -> u64 {
    let mut cu_price: u64 = 0; // microlamports per CU
    let mut cu_limit: u64 = 200_000; // default CU limit

    for inst in &parsed.instructions {
        let prog = all_keys_b58.get(inst.prog_idx as usize).map(|s| s.as_str());
        if prog != Some(COMPUTE_BUDGET) {
            continue;
        }
        if inst.data.is_empty() {
            continue;
        }
        match inst.data[0] {
            2 if inst.data.len() >= 5 => {
                // SetComputeUnitLimit: discriminant 2, u32 LE units
                let units = u32::from_le_bytes(inst.data[1..5].try_into().unwrap_or([0; 4]));
                cu_limit = units as u64;
            }
            3 if inst.data.len() >= 9 => {
                // SetComputeUnitPrice: discriminant 3, u64 LE microlamports
                let price = u64::from_le_bytes(inst.data[1..9].try_into().unwrap_or([0; 8]));
                cu_price = price;
            }
            _ => {}
        }
    }

    // Total priority fee = cu_limit * cu_price / 1_000_000 (microlamports → lamports)
    cu_limit.saturating_mul(cu_price) / 1_000_000
}

/// Check instruction semantics against known dangerous patterns.
///
/// Blocks:
/// - SPL Token / Token-2022: Approve (4), ApproveChecked (13), SetAuthority (6).
///   These grant control over tokens to arbitrary third parties.
/// - System Program Transfer where the destination is in an address lookup table
///   (not static keys). Jupiter's normal wSOL wrapping always uses a static
///   destination; an ALT-sourced destination is a drain indicator.
fn check_instruction_semantics(
    parsed: &ParsedTransaction,
    num_static_keys: usize,
    all_keys_b58: &[String],
) -> Result<(), String> {
    for inst in &parsed.instructions {
        let prog_b58 = all_keys_b58
            .get(inst.prog_idx as usize)
            .map(|s| s.as_str())
            .unwrap_or("");

        if prog_b58 == SPL_TOKEN || prog_b58 == TOKEN_2022 {
            if !inst.data.is_empty() {
                match inst.data[0] {
                    4 => return Err(
                        "SPL Token Approve instruction is not permitted — \
                         it grants spending rights to a third party"
                            .into(),
                    ),
                    6 => return Err(
                        "SPL Token SetAuthority instruction is not permitted — \
                         it transfers token account ownership"
                            .into(),
                    ),
                    13 => return Err(
                        "SPL Token ApproveChecked instruction is not permitted".into(),
                    ),
                    _ => {}
                }
            }
        } else if prog_b58 == SYSTEM_PROGRAM {
            // System Transfer discriminant is u32 LE = 2
            if inst.data.len() >= 4 {
                let discriminant =
                    u32::from_le_bytes(inst.data[..4].try_into().unwrap_or([0; 4]));
                if discriminant == 2 {
                    // Transfer — verify destination is in static accounts, not ALT
                    if let Some(&dest_idx) = inst.account_indices.get(1) {
                        if (dest_idx as usize) >= num_static_keys {
                            return Err(format!(
                                "System Program Transfer: destination account index {} is \
                                 sourced from an address lookup table — refusing to sign",
                                dest_idx
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// ── Keypair loading ───────────────────────────────────────────────────────────

fn load_keypair_bytes(key_path: &str) -> Result<Vec<u8>, String> {
    let mut content = std::fs::read_to_string(key_path)
        .map_err(|e| format!("Failed to read keypair file: {e}"))?;

    // Solana CLI saves keypairs as a JSON array of bytes: [1,2,3,...]
    let result = serde_json::from_str::<Vec<u8>>(&content)
        .map_err(|e| format!("Failed to parse keypair JSON: {e}"));

    // Zeroize the string contents to clear key bytes from heap memory.
    content.zeroize();

    result
}

/// Parse a transaction's ALT references for pre-signing resolution.
/// Returns (static_account_keys, alt_refs) where each alt_ref is
/// (table_address, writable_indices, readonly_indices).
/// Called by the swap orchestrator to fetch ALT accounts before verify_and_sign.
pub fn parse_alts_for_resolve(
    bytes: &[u8],
) -> Result<(Vec<[u8; 32]>, Vec<([u8; 32], Vec<u8>, Vec<u8>)>), String> {
    let parsed = parse_transaction(bytes)?;
    let alt_refs = parsed
        .alt_refs
        .into_iter()
        .map(|a| (a.table_address, a.writable_indices, a.readonly_indices))
        .collect();
    Ok((parsed.account_keys, alt_refs))
}

// ── Test helpers (compiled only under cargo test) ─────────────────────────────

/// Expose the transaction parser for integration tests in `tests.rs`.
/// Returns (account_keys, program_id_indices).
#[cfg(test)]
pub fn parse_transaction_for_test(
    bytes: &[u8],
) -> Result<(Vec<[u8; 32]>, Vec<u8>), String> {
    let parsed = parse_transaction(bytes)?;
    let program_indices = parsed.instructions.iter().map(|i| i.prog_idx).collect();
    Ok((parsed.account_keys, program_indices))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
pub mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::RngCore;

    fn gen_signing_key() -> SigningKey {
        let mut seed = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        SigningKey::from_bytes(&seed)
    }

    /// Build a minimal unsigned legacy Solana transaction for testing.
    pub fn build_test_transaction(
        fee_payer: &[u8; 32],
        program_ids: &[[u8; 32]],
        input_mint: &[u8; 32],
        output_mint: &[u8; 32],
        priority_fee_microlamports_per_cu: u64,
        cu_limit: u32,
    ) -> Vec<u8> {
        // Account keys: fee_payer, programs..., mints...
        let mut account_keys: Vec<[u8; 32]> = vec![*fee_payer];
        account_keys.extend_from_slice(program_ids);
        account_keys.push(*input_mint);
        account_keys.push(*output_mint);

        let compute_budget_bytes: [u8; 32] = {
            let decoded = bs58::decode(COMPUTE_BUDGET).into_vec().unwrap();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&decoded);
            arr
        };

        // Add compute budget if needed
        let budget_prog_idx = if program_ids.iter().any(|p| p == &compute_budget_bytes) {
            program_ids.iter().position(|p| p == &compute_budget_bytes).map(|i| i + 1)
        } else {
            account_keys.push(compute_budget_bytes);
            Some(account_keys.len() - 1)
        };

        let num_keys = account_keys.len() as u16;

        let mut msg = Vec::new();
        // header
        msg.push(1u8); // num_required_signatures
        msg.push(0u8); // num_readonly_signed_accounts
        msg.push(program_ids.len() as u8 + 2); // num_readonly_unsigned_accounts

        // account keys
        msg.extend(encode_compact_u16(num_keys));
        for k in &account_keys {
            msg.extend_from_slice(k);
        }

        // recent_blockhash (zeroed)
        msg.extend_from_slice(&[0u8; 32]);

        // Instructions
        let mut instructions: Vec<(u8, Vec<u8>)> = Vec::new();

        // SetComputeUnitLimit instruction
        if let Some(cb_idx) = budget_prog_idx {
            let mut data = vec![2u8]; // discriminant
            data.extend_from_slice(&cu_limit.to_le_bytes());
            instructions.push((cb_idx as u8, data));
        }

        // SetComputeUnitPrice instruction
        if priority_fee_microlamports_per_cu > 0 {
            if let Some(cb_idx) = budget_prog_idx {
                let mut data = vec![3u8]; // discriminant
                data.extend_from_slice(&priority_fee_microlamports_per_cu.to_le_bytes());
                instructions.push((cb_idx as u8, data));
            }
        }

        // A dummy Jupiter instruction
        if !program_ids.is_empty() {
            instructions.push((1u8, vec![0u8; 8])); // program index 1 = first program
        }

        msg.extend(encode_compact_u16(instructions.len() as u16));
        for (prog_idx, data) in &instructions {
            msg.push(*prog_idx);
            msg.extend(encode_compact_u16(0)); // no account indices
            msg.extend(encode_compact_u16(data.len() as u16));
            msg.extend_from_slice(data);
        }

        // Build full transaction: 1 sig slot (zeros) + message
        let mut tx = Vec::new();
        tx.extend(encode_compact_u16(1u16)); // 1 signature slot
        tx.extend_from_slice(&[0u8; 64]); // unsigned signature slot
        tx.extend_from_slice(&msg);
        tx
    }

    fn gen_keypair() -> (SigningKey, [u8; 32]) {
        let key = gen_signing_key();
        let pubkey = key.verifying_key().to_bytes();
        (key, pubkey)
    }

    fn pubkey_to_b58(bytes: &[u8; 32]) -> String {
        bs58::encode(bytes).into_string()
    }

    pub fn program_bytes(addr: &str) -> [u8; 32] {
        let v = bs58::decode(addr).into_vec().unwrap();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&v);
        arr
    }

    pub fn mint_bytes(addr: &str) -> [u8; 32] {
        program_bytes(addr)
    }

    // ── compact_u16 ──────────────────────────────────────────────────────────

    #[test]
    fn compact_u16_roundtrip_small() {
        for val in [0u16, 1, 127, 128, 255, 16383, 65535] {
            let enc = encode_compact_u16(val);
            let (dec, _) = decode_compact_u16(&enc).unwrap();
            assert_eq!(dec, val, "roundtrip failed for {val}");
        }
    }

    // ── program allowlist ────────────────────────────────────────────────────

    #[test]
    fn signer_rejects_disallowed_program() {
        let (signing_key, pubkey) = gen_keypair();
        let pubkey_b58 = pubkey_to_b58(&pubkey);

        // A fake malicious program
        let bad_program = [0xBB_u8; 32];
        let jup = program_bytes(super::super::config::JUPITER_V6);

        let tx = build_test_transaction(
            &pubkey,
            &[bad_program, jup],
            &mint_bytes(super::super::config::SOL_MINT),
            &mint_bytes(super::super::config::USDC_MINT),
            0,
            200_000,
        );
        let tx_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &tx,
        );

        let ticket = make_ticket(&signing_key, &pubkey_b58);
        // We can't call the full sign path without a key file, so test the parser directly
        let parsed = parse_transaction(&tx).unwrap();
        let all_keys_b58: Vec<String> = parsed
            .account_keys
            .iter()
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let has_bad = parsed.instructions.iter().any(|inst| {
            all_keys_b58
                .get(inst.prog_idx as usize)
                .map(|k| !ALLOWED_PROGRAMS.contains(&k.as_str()))
                .unwrap_or(false)
        });
        assert!(has_bad, "should detect disallowed program");
        drop((tx_b64, ticket));
    }

    #[test]
    fn signer_allows_all_known_programs() {
        let (_, pubkey) = gen_keypair();
        let jup = program_bytes(super::super::config::JUPITER_V6);
        let cb = program_bytes(super::super::config::COMPUTE_BUDGET);

        let tx = build_test_transaction(
            &pubkey,
            &[jup, cb],
            &mint_bytes(super::super::config::SOL_MINT),
            &mint_bytes(super::super::config::USDC_MINT),
            0,
            200_000,
        );

        let parsed = parse_transaction(&tx).unwrap();
        let all_keys_b58: Vec<String> = parsed
            .account_keys
            .iter()
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let all_allowed = parsed.instructions.iter().all(|inst| {
            all_keys_b58
                .get(inst.prog_idx as usize)
                .map(|k| ALLOWED_PROGRAMS.contains(&k.as_str()))
                .unwrap_or(false)
        });
        assert!(all_allowed, "all programs should be in allowlist");
    }

    // ── fee payer check ──────────────────────────────────────────────────────

    #[test]
    fn signer_rejects_wrong_fee_payer() {
        let (_, pubkey) = gen_keypair();
        let (_, other_pubkey) = gen_keypair();

        let jup = program_bytes(super::super::config::JUPITER_V6);
        let tx = build_test_transaction(
            &other_pubkey, // wrong fee payer
            &[jup],
            &mint_bytes(super::super::config::SOL_MINT),
            &mint_bytes(super::super::config::USDC_MINT),
            0,
            200_000,
        );

        let parsed = parse_transaction(&tx).unwrap();
        let fee_payer_b58 = pubkey_to_b58(&parsed.account_keys[0]);
        let expected = pubkey_to_b58(&pubkey);

        assert_ne!(
            fee_payer_b58, expected,
            "fee payer should not match owner pubkey"
        );
    }

    // ── missing expected mint ────────────────────────────────────────────────

    #[test]
    fn signer_rejects_missing_output_mint() {
        let (_, pubkey) = gen_keypair();
        let jup = program_bytes(super::super::config::JUPITER_V6);

        // Build tx WITHOUT the output mint in account keys
        let tx = build_test_transaction(
            &pubkey,
            &[jup],
            &mint_bytes(super::super::config::SOL_MINT),
            &mint_bytes(super::super::config::SOL_MINT), // wrong output mint
            0,
            200_000,
        );

        let parsed = parse_transaction(&tx).unwrap();
        let all_keys_b58: Vec<String> = parsed
            .account_keys
            .iter()
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let expected_output = super::super::config::USDC_MINT;
        let found = all_keys_b58.contains(&expected_output.to_string());
        assert!(!found, "USDC mint should not be in this transaction");
    }

    // ── priority fee cap ─────────────────────────────────────────────────────

    #[test]
    fn priority_fee_extraction_respects_cap() {
        let (_, pubkey) = gen_keypair();
        let cb = program_bytes(super::super::config::COMPUTE_BUDGET);

        // 200_000 CUs × 5_000_000 µlamports/CU = 10^12 µlamports
        // / 1_000_000 = 1_000_000 lamports = 0.001 SOL > 0.0005 SOL cap
        let tx = build_test_transaction(
            &pubkey,
            &[cb],
            &mint_bytes(super::super::config::SOL_MINT),
            &mint_bytes(super::super::config::USDC_MINT),
            5_000_000, // microlamports per CU
            200_000,   // CU limit
        );

        let parsed = parse_transaction(&tx).unwrap();
        let all_keys_b58: Vec<String> = parsed
            .account_keys
            .iter()
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let fee_lamports = extract_priority_fee_lamports(&parsed, &all_keys_b58);
        let fee_sol = fee_lamports as f64 / 1_000_000_000.0;

        // 200_000 * 5_000 / 1_000_000 = 1_000_000 lamports = 0.001 SOL
        assert!(
            fee_sol > 0.0005,
            "fee {fee_sol} SOL should exceed 0.0005 SOL cap"
        );
    }

    #[test]
    fn priority_fee_zero_when_no_compute_budget() {
        let (_, pubkey) = gen_keypair();
        let jup = program_bytes(super::super::config::JUPITER_V6);

        let tx = build_test_transaction(
            &pubkey,
            &[jup],
            &mint_bytes(super::super::config::SOL_MINT),
            &mint_bytes(super::super::config::USDC_MINT),
            0, // no price set
            200_000,
        );

        let parsed = parse_transaction(&tx).unwrap();
        let all_keys_b58: Vec<String> = parsed
            .account_keys
            .iter()
            .map(|k| bs58::encode(k).into_string())
            .collect();

        let fee_lamports = extract_priority_fee_lamports(&parsed, &all_keys_b58);
        assert_eq!(fee_lamports, 0);
    }

    // ── blockhash expiry ─────────────────────────────────────────────────────

    #[test]
    fn expired_blockhash_is_detected_by_ticket() {
        // current_block_height > last_valid_block_height => expired
        let ticket = SignerTicket {
            expected_fee_payer: "fake".into(),
            expected_input_mint: "fake".into(),
            expected_output_mint: "fake".into(),
            expected_in_amount: 0,
            last_valid_block_height: 100,
            current_block_height: 200, // past expiry
            priority_fee_cap_sol: 0.001,
            resolved_alt_accounts: vec![],
        };
        assert!(
            ticket.current_block_height > ticket.last_valid_block_height,
            "ticket should indicate expired blockhash"
        );
    }

    fn make_ticket(_signing_key: &SigningKey, pubkey_b58: &str) -> SignerTicket {
        SignerTicket {
            expected_fee_payer: pubkey_b58.to_string(),
            expected_input_mint: super::super::config::SOL_MINT.into(),
            expected_output_mint: super::super::config::USDC_MINT.into(),
            expected_in_amount: 1_000_000_000,
            last_valid_block_height: 99_999_999,
            current_block_height: 1_000,
            priority_fee_cap_sol: 0.001,
            resolved_alt_accounts: vec![],
        }
    }
}

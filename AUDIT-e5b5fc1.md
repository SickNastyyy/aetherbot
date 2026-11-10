# AetherBot audit: commit e5b5fc1 ("Jupiter live swap V2")

- **Auditor:** Grok Bot (MoneyMan desk), read-only.
- **Date:** 2026-11-09, about 3:55 PM ET.
- **Scope:** `src-tauri/src/engine/jupiter/*`, `commands.rs`, `lib.rs`, and related config, capabilities and credential code.
- **Rules followed:**
  - No keypair or wallet was created or loaded. PoCs use an ephemeral random ed25519 key that exists only in memory, the same way the repo's own tests do.
  - `live_execution_enabled` was never set.
  - No calls were made to Jupiter or any Solana RPC, and the app was never launched.
  - Nothing was written to the repo except this file. It is untracked and not committed.

## 1. Build and test result

| Item | Result |
|---|---|
| HEAD | `e5b5fc10b691b7a567b97aa53dabaf320cae791e` (author SickNastyyy, 2026-11-09 13:04 ET) |
| Files | 19 changed, 3935 insertions and 4 deletions; 12 added (A) and 7 modified (M). **Verified.** |
| Full crate `cargo test --lib` | **Did not build on the box.** Box rustc 1.85.1 is too old for the locked deps (darling/time/serde_with need 1.88). After installing rustc 1.99.1, `glib-sys` failed because `pkg-config` and the GTK/WebKit dev libs are missing, and the box's apt mirror returned HTTP 500 / SSL EOF. The 139 non-Jupiter tests were **not run**. |
| Jupiter module (Tauri-free harness at `/workspace/audits/aetherbot-harness`, byte-identical copies of `engine/jupiter/*.rs`, `database.rs` and `paths.rs` checked with `cmp`) | **35 passed, 0 failed.** That is 27 in `tests.rs` and 8 in `signer.rs`. No network calls. |
| Audit PoCs (harness only, `audit_poc/*.rs`) | **7 passed.** Each one demonstrates a finding below. |
| Total `#[test]` annotations in the repo | 174 (139 before this commit, plus 35 added). The claimed "174 passed" matches this count, but I could only run 35 of them. |

## 2. Claim-by-claim

| # | Claim (Claude report / commit msg) | Verdict | Evidence |
|---|---|---|---|
| 1 | Commit e5b5fc1, 19 files (12 new, 7 modified) | **Verified** | `git show --stat`, `--name-status` |
| 2 | 174 tests pass | **Partial / unverified** | 174 annotations exist. The 35 Jupiter tests pass. The other 139 could not be built here because the Tauri system libs are missing. |
| 3 | "tests.rs ← 36 unit tests" / "36 Jupiter unit tests" | **False** | `tests.rs` has 27. Jupiter has 35 in total (27 + 8 in `signer.rs`). |
| 4 | All 8 owner limits are typed fields | **Verified, but defaults are Claude's, not Coleman's** | `config.rs:29-62`. Defaults at `config.rs:64-83`: cap $250, 25%/swap, 500 bps, 5% impact, $5 SOL reserve, 10% daily stop, 75% meme cap, SOL+USDC allowlist. |
| 5 | "Key limits all enforced in Rust, not config" (commit msg) | **False** | These are editable config fields. `jupiter_save_config` (`commands.rs:232-249`) only bounds `account_cap_usd ≤ 250`, `0 < max_swap_pct ≤ 100` and `max_slippage_bps ≤ 1000`. Price impact, daily stop, reserve, fee cap, quote age, meme cap and the allowlist contents are not validated at all. `JupiterSwapConfig::load` (`config.rs:100-110`) does no validation, so editing `jupiter_config.json` by hand bypasses even the $250 check. |
| 6 | `execution_gate_open()` requires 4 conditions | **Verified** | `config.rs:93-98`: live flag, a non-empty pubkey, a non-empty allowlist, and `signer_key_path.is_some()`. It does not check that the path exists, that the key matches, or the RPC. |
| 7 | $250 hard ceiling enforced server-side | **Partial / misleading** | The check is in `commands.rs:234` (save only), so editing the file bypasses it. Worse, the `account_cap` gate measures only SOL. Every SPL token's `amount_usd` is hard-coded `None` (`wallet.rs:143`), and `total_usd = sol_usd + Σ amount_usd` (`wallet.rs:62,68`). A wallet holding $10,000 of USDC and $20 of SOL passes the $250 cap. |
| 8 | 10 gates in order: kill → daily_stop → allowlist → stale → cap → per-swap → slippage → impact → reserve → meme | **Verified (order)** | `gates.rs:54-67` |
| 9 | Gates are meaningful | **Mostly false** | **Slippage** is circular: `quote.slippage_bps` is our own request value echoed back (`quote.rs:127`, `swap.rs:96`), so `gates.rs:198` compares config to config. **Stale-quote** is circular: `fetched_at` is set locally at parse time (`quote.rs:131`), so it is about 0 s old when checked. **Price impact** fails open: a missing `priceImpactPct` becomes 0.0 (`quote.rs:99-103`). **Daily stop** is dead: `daily_loss_usd: 0.0` is hard-coded (`swap.rs:107`) and `check_and_maybe_block` (`daily_stop.rs:72`) is never called. **Per-swap/reserve/cap** depend on `sol_price_usd`, which is passed in by the frontend caller (`commands.rs:258`, `swap.rs:71,81,111`), and the 25% limit is measured against the SOL-only total. **Meme cap** never counts held tokens because `amount_usd` is always None (`gates.rs:275-281`). |
| 10 | Meme cap is Warn only | **Verified** | `gates.rs:265-301` (Warn). Note that Warn also passes `all_gates_passed` (`gates.rs:71-75`). `per_swap_limit` downgrades to Warn when it can't price a pair (`gates.rs:190-193`). |
| 11 | Gates re-run / no TOCTOU at execute | **False** | `execute_approved` (`swap.rs:164-348`) re-checks only the status, the execution gate and the kill switch. It does **not** re-run gates, re-read balances, check the daily stop, or check ticket age. It rebuilds the quote with `fetched_at: now` (`swap.rs:200`). Tickets never expire (`ticket.rs`, no TTL), so a ticket approved hours or days ago will execute against whatever the market and wallet look like then. |
| 12 | Signer checks fee payer | **Verified** | `signer.rs:116-126`, plus the key/pubkey match at `signer.rs:96-104` |
| 13 | Signer program allowlist | **Verified, but the allowlist itself is the hole** | `signer.rs:128-137` against `config.rs:17-24`, which includes **System Program and SPL Token/Token-2022**. Any `system::transfer`, `token::transfer`, `approve` or `set_authority` to any address passes. **PoC:** `poc_signer_signs_full_sol_drain_to_attacker` and `poc_signer_signs_spl_approve_unlimited_delegate`: the signer returns Ok and signs. |
| 14 | Signer checks mints | **Weak** | `signer.rs:139-157` only checks that the two mint pubkeys appear somewhere in the static keys. There is no check of direction, amount or recipient. `expected_in_amount` is never read (`signer.rs:26`). `other_amount_threshold` (min-out) is never compared to the instruction data. |
| 15 | Signer checks priority fee cap | **Partial** | `signer.rs:159-167, 386-419` parses only ComputeBudget price × limit (with a default CU limit of 200k). Tips paid as System transfers are not counted (**PoC** `poc_priority_fee_ignores_jito_style_tip_transfer`: a 0.05 SOL tip is signed under a 0.0005 SOL cap). |
| 16 | Signer checks blockhash age (">150 blocks") | **False / broken** | `signer.rs:172` compares **slot** (`getSlot`, `wallet.rs:154-175`) against **lastValidBlockHeight**. On mainnet, slot runs tens of millions ahead of block height, so every real tx would be refused as "expired". If the `getSlot` RPC call fails, `swap.rs:248` uses `unwrap_or(0)` and the check **passes**. **PoC** `poc_blockhash_check_compares_slot_to_block_height`. |
| 17 | v0 address lookup tables handled | **False** | The parser skips the version byte (`signer.rs:242-245`) and never parses the ALT section (`signer.rs:311-312`). Accounts loaded through ALTs, such as destination token accounts, are invisible to every check. **PoC** `poc_v0_lookup_table_accounts_are_invisible_and_signed`. Conversely, if the mints live only in an ALT (common for Jupiter routes), the signer refuses (**PoC** `poc_v0_mints_only_in_lookup_table_are_refused`). |
| 18 | Key bytes zeroized | **Partial** | The `Vec<u8>` is zeroized (`signer.rs:56`), and `SigningKey` zeroizes on drop (dalek `zeroize` feature). The **JSON file text** (`String` holding the full 64-byte secret, `signer.rs:424`) and the `seed: [u8;32]` copy (`signer.rs:91`) are not zeroized. Neither is the env-var path's `raw` String (`signer.rs:68`). |
| 19 | Key never leaks to logs/IPC/frontend | **Verified (for signer path)** | There is no `tracing`/`println` in the jupiter module or `commands.rs`. Errors carry only the pubkey. The key is read from a **plaintext keypair JSON** at `signer_key_path`, and its protection is file permissions only (as Claude admits). `signer_key_path` itself is returned to the frontend by `jupiter_get_config`. |
| 20 | Orchestrator: simulate → sign → submit → confirm → reconcile | **Partial** | The order is correct (`swap.rs:204-343`). **"Reconcile" is false:** actual output is recorded as `0` (`swap.rs:313`) and the fee is a hard-coded 5000 (`swap.rs:294`). Simulation checks only `err` and does not check post-balances (`wallet.rs:253-289`). |
| 21 | Never resubmits; PENDING on timeout | **Verified** | `maxRetries: 0` (`wallet.rs:227`), one `send_transaction` call, and a 60 s timeout sets Pending (`swap.rs:302-343`). If `sendTransaction` errors, the status is set to Pending with an **empty signature** (`swap.rs:278-290`), even though the signature is known locally (it is the first signature). That makes on-chain reconciliation harder. |
| 22 | Uses Jupiter API correctly | **False (non-functional)** | `GET /swap/v2/order` is real, but `POST {base}/swap` (`quote.rs:165`) is **not an endpoint in Swap API V2**, which has only `/order`, `/build` and `/execute` (developers.jup.ag, March 2026 changelog). The body is a fabricated `quoteResponse` with `routePlan: []` (`quote.rs:145-163`), so the tx that is signed is **not bound to the approved quote**. The `/order` response with `taker` already contains the transaction, and the code ignores it. As written, `execute_approved` should fail at step 4a. |
| 23 | Kill switch: atomic, one command | **Partial** | `kill_switch.rs:12-14` uses an in-memory `AtomicBool`. **It is not persisted**, so it resets to OFF on restart. `jupiter_kill_switch` silently returns Ok and does nothing if the orchestrator hasn't been created yet (`commands.rs:324-327`); the next quote then creates a fresh orchestrator with the switch OFF. **It cannot stop an in-flight execute:** `jupiter_execute_approved` holds the `JupiterState` tokio Mutex for the whole sign/submit/60 s confirm (`commands.rs:303-307`), and `jupiter_kill_switch` waits on that same lock (`commands.rs:316`). |
| 24 | Daily stop persisted in SQLite, NY day boundary | **Verified (storage) / dead (logic)** | The NY date and the SQLite row are implemented (`daily_stop.rs:28, 72-105`). Nothing ever sets `blocked=true` in the live path, and loss is hard-coded to 0 (`swap.rs:107`). Start-of-day equity is SOL-only. |
| 25 | TicketStore "single-slot, only one pending" | **False** | `ticket.rs:81-84` is an unbounded `HashMap`. Multiple tickets can be pending or approved at once, with no TTL. |
| 26 | Config changes take effect | **False (safety-relevant)** | The orchestrator loads config once, lazily (`commands.rs:265-273`). `jupiter_save_config` writes the file but never refreshes the live orchestrator. **Turning `live_execution_enabled` off or lowering limits in the UI has no effect until the app restarts.** |
| 27 | Approval is a human step "behind UI password check" | **False** | `jupiter_approve_ticket` (`commands.rs:283-293`) has no password, nonce or second factor. Any code in the webview can call `jupiter_quote_swap` → `jupiter_approve_ticket` → `jupiter_execute_approved` back-to-back with no human involved. Tauri v2 app commands are allowed for the `main` window by default. `capabilities/default.json` does not restrict them (and its `$schema` points at a NiceGUI URL, not Tauri). `tauri.conf.json` sets **no CSP**. Today the frontend `src/` has **no** jupiter UI or invoke calls at all, so the "review gate results in the UI" step does not exist yet. There is no Risk Officer step anywhere. |
| 28 | No autonomous/scheduled path | **Verified** | `BotEngine` executes only `if cfg.paper_mode` (`engine/mod.rs:519`). `jupiter_executor.rs` helpers (`build_orchestrator`, `request_quote`) are never called. Jupiter execution exists only via the three IPC commands. |
| 29 | Tests test real behavior | **Partial** | Gate tests call the real pure gate functions (good). Signer tests (`signer.rs:579-785`, `tests.rs:512-668`) re-implement checks on the parser output and **never call `verify_and_sign`/`sign_internal`**. The blockhash test (`signer.rs:770-785`) and the "no resubmit" test (`tests.rs:673-702`) are tautologies. There are no tests for `SwapOrchestrator`, `quote.rs`, `wallet.rs` or IPC commands. No test hits the network or signs a real tx. |
| 30 | No hardcoded secrets/telemetry | **Verified** | No keys, tokens or telemetry SDKs were found. Hardcoded URLs: public RPC `https://api.mainnet-beta.solana.com` (`config.rs:67`, rate-limited and unsuitable for sending), Jupiter `api.jup.ag/swap/v2` (keyless, 0.5 rps), plus pre-existing Anthropic/xAI/Kalshi/CryptoCompare/Reddit/DefiLlama endpoints whose keys come from config. The commit author email (Coleman2444@gmail.com) is public in git history. |

## 3. Credential storage (ExchangeCredForm / WalletCredForm / KalshiConfig)

- All three forms call `save_exchange_credentials` (`src/hooks/useTauriCommands.ts:16-28`) → `config::save_credentials` (`src-tauri/src/config.rs:25-46`).
- Values are encrypted with AES-256-GCM using a random nonce (`crypto.rs:36-49`) and written to `<data_dir>/aetherbot/credentials.json`.
- **The AES key is in the same folder as plaintext base64**, in `master.key` (`crypto.rs:14-34`), written with `fs::write` at default permissions. There is no OS keychain, no DPAPI and no password. In practice anyone or any process that can read the data dir can decrypt everything. This is obfuscation, not protection.
- **WalletCredForm sends the wallet private key itself** as `api_key` (`WalletCredForm.tsx:57-62`), puts the RPC URL in the passphrase slot, and uses a dummy secret. So a Solana or EVM private key may end up in `credentials.json`. This is a second key store, separate from the plaintext `signer_key_path` JSON that the Jupiter signer actually uses.
- Only masked values (`****last4`) return to the frontend (`config.rs:48-69`). Nothing persists in localStorage, and the zustand store is in-memory.

## 4. Top findings by severity

1. **CRITICAL: the signer will sign a drain.**
   - The allowlist includes System Program and SPL Token (`config.rs:17-24`).
   - There are no instruction-semantic checks (`signer.rs:128-177`).
   - `expected_in_amount` and min-out are ignored (`signer.rs:26`).
   - v0 ALT accounts are invisible (`signer.rs:311`).
   - Whoever controls the unsigned tx can get a SOL/token transfer, an unlimited `approve` or a `set_authority` to themselves signed. That could be the Jupiter API, a compromised dependency or DNS/TLS path, or a future code change. The PoCs prove this.
2. **CRITICAL: the approve step has no human or Risk Officer gate, and execution isn't bound to the approved quote.**
   - IPC `quote → approve → execute` is callable in one shot from the webview (`commands.rs:253-308`), with no CSP and no capability restriction.
   - Gates are not re-run at execute time, and tickets never expire (`swap.rs:164-202`).
   - The swap tx comes from a fabricated `quoteResponse` (`quote.rs:145-165`) that has nothing to do with what was approved.
3. **HIGH: the risk limits are largely fake.**
   - The account cap and 25% limit ignore every token balance (`wallet.rs:143`).
   - The frontend supplies the SOL price (`commands.rs:258`).
   - The slippage and stale gates are circular (`quote.rs:127,131`).
   - Price impact fails open (`quote.rs:99-103`).
   - The daily stop is dead (`swap.rs:107`, `check_and_maybe_block` never called).
   - The $250 cap is bypassable by editing the file (`config.rs:100-110`).
4. **HIGH: the off switches don't work when needed.**
   - The kill switch blocks on the same mutex as an in-flight execute (`commands.rs:303/316`).
   - It is not persisted and resets to OFF on restart (`kill_switch.rs:13`).
   - It silently does nothing before the orchestrator exists (`commands.rs:324-327`).
   - Setting `live_execution_enabled=false` doesn't apply until restart (`commands.rs:265-273`).
5. **HIGH: key handling.**
   - The signer reads a plaintext keypair JSON, and the String copy is not zeroized (`signer.rs:424`).
   - The pre-existing credential store keeps the AES key next to the ciphertext (`crypto.rs:14-34`), and WalletCredForm stores raw wallet private keys there (`WalletCredForm.tsx:57-62`).
   - Also present: blockhash slot/height confusion that fails open when `getSlot` errors (`signer.rs:172`, `swap.rs:248`), a fake reconcile (`swap.rs:313`), and a non-existent `/swap` endpoint (`quote.rs:165`).

**Net:** as written, live execution probably fails before signing (dead endpoint), or is refused by the signer (slot vs height bug). It is **non-functional and unsafe at the same time**. Fixing either bug without fixing findings 1–4 makes it dangerous.

## 5. Verdict

**Not safe to fund a wallet.** Before any funding:

1. Bind execution to the approved order.
   - Use `/order` (with `taker`) → sign that exact tx → `/execute`, or `/build` plus your own assembly.
   - Make the ticket store the tx hash.
   - Give tickets a TTL of 30 s or less.
   - Re-run all gates and re-read balances at execute time.
2. Rebuild the signer as a real verifier.
   - Resolve ALTs (fetch tables and pin them).
   - Allow System and Token instructions only in known shapes: ATA create, wSOL sync/close to the owner, ComputeBudget.
   - Reject any transfer, approve or set_authority to non-owner accounts.
   - Simulate with `accounts` and assert the owner's pre/post balances: input decrease ≤ `in_amount`, output increase ≥ min-out, SOL decrease ≤ fee cap.
3. Fix the blockhash check to use `getBlockHeight`, and fail closed on RPC errors.
4. Price every holding in the backend (no caller-supplied price). Validate all limits on load as well as on save, and hot-reload config into the orchestrator.
5. Fix the kill switch.
   - It must not share the execute mutex.
   - It must persist.
   - It must be checked again right before submit.
   - It must work before the orchestrator exists.
6. Real approval.
   - Add an out-of-band confirmation that the webview can't forge.
   - Set a CSP and capability permissions that scope the jupiter commands.
   - Add a Risk Officer step if this lane is ever approved.
7. Keys.
   - Use the OS keychain or a hardware wallet. Remove the plaintext JSON and the `master.key`-beside-ciphertext design.
   - Never store wallet private keys via WalletCredForm.
8. Desk policy, independent of code: Coleman's desk rules (`/home/box/agent-data/crypto-desk/rules.json`) cover only KCEX manual spot and Kraken data/paper.
   - Jupiter/Solana is a **new, unapproved lane**.
   - It has **no owner-set numeric limits**; all live caps are null.
   - There is **no Risk Officer review step** in the bot's approve flow.
   - The lane needs an explicit decision and limits from Coleman, plus Risk Officer review, before any funding.

## 6. Reproduce

```
cd /workspace/audits/aetherbot-harness && ~/.cargo/bin/cargo test   # 35 original + 7 PoC = 42 pass
```
The PoCs are in `audit_poc/signer_poc.rs` and `audit_poc/quote_poc.rs`. They are wired into the harness copies of `signer.rs`/`quote.rs` with a `#[path]` include and do not touch the original repo.

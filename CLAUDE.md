# AetherBot — Claude Code Project Guide

## What This Is
AetherBot is a desktop crypto + prediction market trading bot built with **Tauri 2 (Rust backend)** and **React (TypeScript frontend)**. It trades on Jupiter (Solana DEX), Uniswap (EVM DEX), and Kalshi (US-regulated prediction market) — all non-KYC, US-accessible.

## Architecture

```
Rust Backend (src-tauri/src/)
  engine/           — Core trading engine (async tick loop on tokio)
    data_provider/  — Price feeds: Jupiter API, Uniswap, Kalshi
    signals/        — TA indicators (RSI, EMA, MACD, ATR, Bollinger)
    strategy/       — Pluggable strategies (Strategy trait)
    risk/           — Pre-flight checks, circuit breakers, stops
    orders/         — Order execution + paper executor
  commands.rs       — Tauri IPC commands
  models.rs         — All shared types (Serialize/Deserialize)
  config.rs         — JSON config persistence
  crypto.rs         — AES-256-GCM credential encryption
  database.rs       — SQLite via rusqlite (trades, logs, candles)

React Frontend (src/)
  pages/            — Dashboard, Settings
  components/       — UI components (dashboard/, layout/, charts/, orb/, settings/, ui/)
  stores/           — Zustand stores (useAppStore, useSettingsStore)
  hooks/            — useTauriCommands, useBotEvents (Tauri event listener)
  lib/              — types.ts, constants.ts, utils.ts
```

## Key Conventions

### Rust
- Async runtime: tokio (engine runs on spawned tasks)
- Engine emits events via `app_handle.emit()` — frontend listens via `@tauri-apps/api/event`
- All types in `models.rs` derive `Serialize, Deserialize, Clone, Debug`
- Error handling: `EngineError` enum in `engine/error.rs` — errors are logged, never crash the tick loop
- `tauri::Emitter` must be imported where `.emit()` is called
- Credentials are AES-256-GCM encrypted at rest, decrypted only at moment of use

### TypeScript/React
- State management: Zustand (no Redux)
- UI components: shadcn/ui + Tailwind CSS + Framer Motion
- Theme: holographic dark (`holo-*` color tokens in index.css)
- `tauriAvailable()` / `isTauri()` guards for browser dev fallback
- Event types mirror Rust models: `LogEvent`, `StateChangeEvent`, `TickEvent`

### Exchange Integration
- Jupiter: REST API at api.jup.ag (Price V2 + Quote V6 + Swap V1)
- Uniswap: TBD (Trading API or direct RPC)
- Kalshi: REST API with API key auth
- Token mint addresses mapped in `data_provider/jupiter.rs`

## Build & Run
```bash
# Frontend dev (browser mode, no Tauri)
npm run dev

# Desktop app (Tauri)
npm run tauri dev

# Type checks
npx tsc --noEmit          # TypeScript
cd src-tauri && cargo check  # Rust
```

## Current State
- Phase 1-2 complete: Engine skeleton, Jupiter live price feeds, candle builder, TA indicators, EMA crossover strategy, event bus to frontend
- Phase 5-6 in progress: Risk manager, order manager, paper executor
- Phases 7-9 pending: Uniswap/Kalshi providers, live executors, frontend polish

## Rules
- NEVER log or serialize private keys, wallet keys, or API secrets
- Paper mode is the default — live executors must check `paper_mode == true` and refuse to execute
- All trades must pass through RiskManager pre-flight checks before execution
- Circuit breakers (MaxDrawdown, StoplossGuard) can halt all trading — this is intentional safety
- Keep mock data fallbacks in frontend for browser dev mode
- User is US-based — only non-KYC, US-accessible exchanges

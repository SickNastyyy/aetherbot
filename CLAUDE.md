# AetherBot — Claude Code Project Guide

## What This Is
AetherBot is a desktop crypto + prediction market trading bot built with **Tauri 2 (Rust backend)** and **React (TypeScript frontend)**. It trades on Jupiter (Solana DEX), Uniswap (EVM DEX), and Kalshi (US-regulated prediction market) — all non-KYC, US-accessible.

## Architecture

```
Rust Backend (src-tauri/src/)
  engine/                — Core trading engine (async tick loop on tokio)
    ai/                  — Claude Trading Brain + evaluator
    aggregator.rs        — Weighted consensus signal aggregation (10 strategies vote)
    backtest.rs          — Backtesting engine (replay candles through strategies)
    correlation.rs       — Cross-pair Pearson correlation + divergence detection
    data_provider/       — Price feeds: Jupiter API, Uniswap (DefiLlama), Kalshi
    orders/              — Order execution + paper executor
    regime.rs            — Market regime detection (ETH EMA-50/200)
    risk/                — Pre-flight checks, circuit breakers, ATR stops, triple barrier
    sentiment/           — Twitter (Grok/xAI), Reddit, CryptoCompare news
    signals/             — 15+ TA indicators (RSI, EMA, MACD, ATR, BB, VWAP, VolumeProfile, ROC...)
    strategy/            — 10 pluggable strategies (trend, reversion, volatility, volume, prediction)
  commands.rs            — Tauri IPC commands
  models.rs              — All shared types (Serialize/Deserialize)
  config.rs              — JSON config persistence
  crypto.rs              — AES-256-GCM credential encryption
  database.rs            — SQLite (trades, candles, portfolio snapshots, logs)

React Frontend (src/)
  pages/                 — Dashboard, Settings, TradeHistory
  components/            — UI components (dashboard/, layout/, charts/, orb/, settings/, ui/)
  stores/                — Zustand stores (useAppStore, useSettingsStore)
  hooks/                 — useTauriCommands, useBotEvents (Tauri event listener)
  lib/                   — types.ts, constants.ts, utils.ts
```

## Signal Pipeline

```
Price Data → 15+ Indicators → 10 Strategies vote → Signal Aggregator (weighted consensus)
                                                          ↑
Cross-Pair Correlation (divergence detection) ────────────┘
Sentiment (Grok/Twitter + Reddit + News) ─────────────────┘
Market Regime (ETH trend filter) ─────────────────────────┘
                                                          ↓
                                              Claude Trading Brain (final judge)
                                                          ↓
                                              Risk Manager → Paper/Live Executor → DB
```

## Key Environment Variables
- `ANTHROPIC_API_KEY` — Enables Claude AI evaluator + Trading Brain
- `XAI_API_KEY` or `GROK_API_KEY` — Enables Twitter/X sentiment via Grok
- Both optional: bot works with TA-only if not set

## Build & Run
```bash
npm run dev              # Frontend browser mode (mock data)
npm run tauri dev        # Desktop app (real engine)
npx tsc --noEmit         # TypeScript check
cd src-tauri && cargo check  # Rust check
```

## Rules
- NEVER log or serialize private keys, wallet keys, or API secrets
- Paper mode is default — live executors MUST check paper_mode before executing
- All trades pass through RiskManager pre-flight checks
- Circuit breakers can halt all trading — this is intentional safety
- Claude Brain is the FINAL judge when configured — it can veto any trade
- Brain adapts: raises confidence threshold after losing streaks
- Sentiment analysis caches for 5 minutes to avoid API spam
- User is US-based — only non-KYC, US-accessible exchanges

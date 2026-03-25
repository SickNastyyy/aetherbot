import type { MockTrade, PnlSummary, TradeLogEntry, SystemStatus } from './types'

export const MOCK_TRADES: MockTrade[] = [
  {
    id: '1',
    exchange: 'Jupiter',
    pair: 'SOL/USDC',
    side: 'LONG',
    entry_price: 185.4,
    current_price: 189.2,
    quantity: 20,
    pnl: 76.0,
    pnl_pct: 2.05,
    status: 'OPEN',
    strategy: 'Momentum',
    opened_at: '2026-03-25T08:30:00Z',
  },
  {
    id: '2',
    exchange: 'Uniswap',
    pair: 'ETH/USDC',
    side: 'LONG',
    entry_price: 3420.8,
    current_price: 3480.5,
    quantity: 1.5,
    pnl: 89.55,
    pnl_pct: 1.75,
    status: 'OPEN',
    strategy: 'DCA',
    opened_at: '2026-03-25T10:15:00Z',
  },
  {
    id: '3',
    exchange: 'Jupiter',
    pair: 'JUP/USDC',
    side: 'LONG',
    entry_price: 1.42,
    current_price: 1.38,
    quantity: 2000,
    pnl: -80.0,
    pnl_pct: -2.82,
    status: 'OPEN',
    strategy: 'Grid',
    opened_at: '2026-03-25T06:45:00Z',
  },
  {
    id: '4',
    exchange: 'Kalshi',
    pair: 'Fed Rate Cut by June',
    side: 'LONG',
    entry_price: 0.42,
    current_price: 0.48,
    quantity: 500,
    pnl: 30.0,
    pnl_pct: 14.29,
    status: 'OPEN',
    strategy: 'Claude Signal',
    opened_at: '2026-03-24T14:00:00Z',
  },
  {
    id: '5',
    exchange: 'Uniswap',
    pair: 'WBTC/ETH',
    side: 'LONG',
    entry_price: 0.0512,
    current_price: 0.0528,
    quantity: 0.5,
    pnl: 54.4,
    pnl_pct: 3.13,
    status: 'CLOSED',
    strategy: 'Momentum',
    opened_at: '2026-03-24T20:30:00Z',
  },
]

export const MOCK_PNL: PnlSummary = {
  total_pnl: 2847.63,
  total_pnl_pct: 14.23,
  today_pnl: 121.39,
  win_rate: 68.5,
  total_trades: 127,
  active_positions: 4,
}

export const MOCK_SYSTEM_STATUS: SystemStatus = {
  uptime_secs: 0,
  mode: 'paper',
  connections: [
    { name: 'Jupiter', connected: false },
    { name: 'Uniswap', connected: false },
    { name: 'Kalshi', connected: false },
  ],
  bot_running: false,
}

const LOG_MESSAGES = [
  { level: 'info' as const, message: 'Scanning SOL/USDC on Jupiter...' },
  { level: 'info' as const, message: 'RSI(14) = 42.3 on ETH/USDC — neutral zone' },
  { level: 'success' as const, message: 'Paper swap filled: LONG SOL/USDC @ $185.40 via Jupiter' },
  { level: 'info' as const, message: 'Kalshi: "Fed Rate Cut by June" volume spike detected' },
  { level: 'warn' as const, message: 'Position JUP/USDC approaching stop-loss (-2.82%)' },
  { level: 'info' as const, message: 'Grid strategy: 3 levels filled on ETH/USDC' },
  { level: 'info' as const, message: 'DCA buy triggered: ETH/USDC @ $3,420 via Uniswap' },
  { level: 'success' as const, message: 'Profit target hit: WBTC/ETH +3.13%' },
  { level: 'info' as const, message: 'Checking Uniswap pool liquidity depth...' },
  { level: 'warn' as const, message: 'High volatility detected on SOL/USDC — tightening stops' },
  { level: 'info' as const, message: 'Claude signal: Bullish on "BTC above $100k by Q3"' },
  { level: 'error' as const, message: 'RPC reconnecting to Solana mainnet (attempt 2/5)' },
  { level: 'info' as const, message: 'Risk check: drawdown at 2.1% (limit: 10%)' },
  { level: 'success' as const, message: 'New position opened: LONG LINK/USDC via Uniswap' },
  { level: 'info' as const, message: 'Backtester: Momentum strategy +23.4% over 30d on SOL/USDC' },
]

export function getRandomLogEntry(): Omit<TradeLogEntry, 'id' | 'timestamp'> {
  return LOG_MESSAGES[Math.floor(Math.random() * LOG_MESSAGES.length)]
}

export const JUPITER_PAIRS = [
  'SOL/USDC', 'JUP/USDC', 'BONK/USDC', 'WIF/USDC', 'PYTH/USDC',
  'RAY/USDC', 'ORCA/USDC', 'MSOL/SOL', 'JITO/USDC', 'HNT/USDC',
]

export const UNISWAP_PAIRS = [
  'ETH/USDC', 'WBTC/ETH', 'LINK/USDC', 'UNI/ETH', 'ARB/USDC',
  'AAVE/ETH', 'LDO/ETH', 'MKR/ETH', 'CRV/USDC', 'PEPE/ETH',
]

export const KALSHI_EVENTS = [
  'Fed Rate Cut by June',
  'BTC above $100k by Q3',
  'US GDP Growth > 3%',
  'S&P 500 above 6000 by Dec',
  'Next Supreme Court Retirement',
]

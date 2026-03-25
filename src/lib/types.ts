export interface AppConfig {
  paper_mode: boolean
  max_drawdown_pct: number
  position_size_pct: number
  stop_loss_pct: number
}

export interface ExchangeCredentials {
  exchange: string
  api_key: string
  secret: string
  passphrase?: string
}

export interface MaskedCredentials {
  exchange: string
  api_key_masked: string
  has_secret: boolean
  has_passphrase: boolean
}

export interface KalshiConfig {
  api_key: string
  secret: string
}

export interface WalletConfig {
  private_key: string
  rpc_url?: string
}

export type BotState = 'idle' | 'scanning' | 'stopping'
export type StopMode = 'hold' | 'liquidate'

export interface SystemStatus {
  uptime_secs: number
  mode: 'paper' | 'live'
  connections: ConnectionStatus[]
  bot_running: boolean
}

export interface ConnectionStatus {
  name: string
  connected: boolean
  latency_ms?: number
}

export interface MockTrade {
  id: string
  exchange: string
  pair: string
  side: 'LONG' | 'SHORT'
  entry_price: number
  current_price: number
  quantity: number
  pnl: number
  pnl_pct: number
  status: 'OPEN' | 'CLOSED' | 'STOPPED'
  strategy: string
  opened_at: string
}

export interface PnlSummary {
  total_pnl: number
  total_pnl_pct: number
  today_pnl: number
  win_rate: number
  total_trades: number
  active_positions: number
}

export interface TradeLogEntry {
  id: string
  timestamp: string
  level: 'info' | 'warn' | 'error' | 'success'
  message: string
}

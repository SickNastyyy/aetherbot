import { create } from 'zustand'
import type { MaskedCredentials } from '@/lib/types'

export interface FullAppConfig {
  paper_mode: boolean
  max_drawdown_pct: number
  position_size_pct: number
  stop_loss_pct: number
  take_profit_atr_mult: number
  stop_loss_atr_mult: number
  max_hold_hours: number
  max_concurrent_positions: number
  cooldown_minutes: number
  max_stops_before_pause: number
  stops_window_minutes: number
  max_slippage_pct: number
  min_liquidity_usd: number
  paper_balance: number
  tick_interval_ms: number
}

interface SettingsState {
  config: FullAppConfig
  credentials: Record<string, MaskedCredentials>
  setConfig: (config: Partial<FullAppConfig>) => void
  setCredentials: (exchange: string, creds: MaskedCredentials) => void
  removeCredentials: (exchange: string) => void
}

export const useSettingsStore = create<SettingsState>((set) => ({
  config: {
    paper_mode: true,
    max_drawdown_pct: 10,
    position_size_pct: 5,
    stop_loss_pct: 2,
    take_profit_atr_mult: 3.0,
    stop_loss_atr_mult: 2.0,
    max_hold_hours: 24,
    max_concurrent_positions: 5,
    cooldown_minutes: 30,
    max_stops_before_pause: 3,
    stops_window_minutes: 60,
    max_slippage_pct: 1.0,
    min_liquidity_usd: 10000,
    paper_balance: 10000,
    tick_interval_ms: 1000,
  },
  credentials: {},

  setConfig: (partial) =>
    set((state) => ({ config: { ...state.config, ...partial } })),

  setCredentials: (exchange, creds) =>
    set((state) => ({
      credentials: { ...state.credentials, [exchange]: creds },
    })),

  removeCredentials: (exchange) =>
    set((state) => {
      const next = { ...state.credentials }
      delete next[exchange]
      return { credentials: next }
    }),
}))

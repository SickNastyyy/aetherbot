import { create } from 'zustand'
import type { AppConfig, MaskedCredentials } from '@/lib/types'

interface SettingsState {
  config: AppConfig
  credentials: Record<string, MaskedCredentials>
  setConfig: (config: Partial<AppConfig>) => void
  setCredentials: (exchange: string, creds: MaskedCredentials) => void
  removeCredentials: (exchange: string) => void
}

export const useSettingsStore = create<SettingsState>((set) => ({
  config: {
    paper_mode: true,
    max_drawdown_pct: 10,
    position_size_pct: 5,
    stop_loss_pct: 2,
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

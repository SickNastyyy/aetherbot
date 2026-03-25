import { create } from 'zustand'
import type { MockTrade, PnlSummary, SystemStatus, TradeLogEntry, BotState, StopMode, ConnectionStatus } from '@/lib/types'
import { MOCK_TRADES, MOCK_PNL, MOCK_SYSTEM_STATUS } from '@/lib/constants'

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

// Start with empty data in Tauri, mock data in browser
const initialTrades = isTauri() ? [] : MOCK_TRADES
const initialPnl: PnlSummary = isTauri()
  ? { total_pnl: 0, total_pnl_pct: 0, today_pnl: 0, win_rate: 0, total_trades: 0, active_positions: 0 }
  : MOCK_PNL

interface AppState {
  trades: MockTrade[]
  pnl: PnlSummary
  systemStatus: SystemStatus
  tradeLog: TradeLogEntry[]
  selectedPair: string
  activePage: 'dashboard' | 'settings'
  livePrices: Record<string, number>

  botState: BotState
  showStopDialog: boolean

  addLogEntry: (entry: Omit<TradeLogEntry, 'id' | 'timestamp'>) => void
  setSelectedPair: (pair: string) => void
  setActivePage: (page: 'dashboard' | 'settings') => void
  incrementUptime: () => void

  startBot: () => void
  requestStop: () => void
  confirmStop: (mode: StopMode) => void
  cancelStop: () => void

  setBotStateFromEngine: (state: string) => void
  updateConnections: (connections: ConnectionStatus[]) => void
  updateFromTick: (data: {
    connections?: ConnectionStatus[]
    positions?: Array<{
      id: string; exchange: string; pair: string; side: string
      entry_price: number; current_price: number; quantity: number
      unrealized_pnl: number; unrealized_pnl_pct: number
      stop_loss: number; take_profit: number; strategy: string; opened_at: string
    }>
    pnl?: PnlSummary
    prices?: Record<string, number>
  }) => void
}

export const useAppStore = create<AppState>((set, get) => ({
  trades: initialTrades,
  pnl: initialPnl,
  systemStatus: MOCK_SYSTEM_STATUS,
  tradeLog: [],
  selectedPair: 'SOL/USDC',
  activePage: 'dashboard',
  livePrices: {},

  botState: 'idle',
  showStopDialog: false,

  addLogEntry: (entry) =>
    set((state) => ({
      tradeLog: [
        ...state.tradeLog.slice(-199),
        {
          ...entry,
          id: crypto.randomUUID(),
          timestamp: new Date().toISOString(),
        },
      ],
    })),

  setSelectedPair: (pair) => set({ selectedPair: pair }),
  setActivePage: (page) => set({ activePage: page }),
  incrementUptime: () =>
    set((state) => ({
      systemStatus: {
        ...state.systemStatus,
        uptime_secs: state.systemStatus.uptime_secs + 1,
      },
    })),

  startBot: async () => {
    if (isTauri()) {
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        set({ botState: 'scanning', trades: [], pnl: { total_pnl: 0, total_pnl_pct: 0, today_pnl: 0, win_rate: 0, total_trades: 0, active_positions: 0 } })
        await invoke('start_bot')
      } catch (e) {
        console.error('Failed to start bot:', e)
        get().addLogEntry({ level: 'error', message: `Failed to start: ${e}` })
        set({ botState: 'idle' })
      }
    } else {
      const { addLogEntry } = get()
      set({
        botState: 'scanning',
        systemStatus: {
          ...get().systemStatus,
          bot_running: true,
          connections: get().systemStatus.connections.map((c) => ({
            ...c,
            connected: true,
            latency_ms: Math.floor(Math.random() * 80) + 20,
          })),
        },
      })
      addLogEntry({ level: 'success', message: 'Bot started (browser dev mode)' })
    }
  },

  requestStop: () => set({ showStopDialog: true }),

  confirmStop: async (mode: StopMode) => {
    set({ showStopDialog: false, botState: 'stopping' })

    if (isTauri()) {
      try {
        const { invoke } = await import('@tauri-apps/api/core')
        await invoke('stop_bot', { mode })
      } catch (e) {
        console.error('Failed to stop bot:', e)
        get().addLogEntry({ level: 'error', message: `Failed to stop: ${e}` })
      }
    } else {
      const { addLogEntry } = get()
      if (mode === 'liquidate') {
        addLogEntry({ level: 'warn', message: 'Liquidating all positions (dev mode)...' })
        setTimeout(() => {
          set((state) => ({
            botState: 'idle',
            trades: state.trades.map((t) =>
              t.status === 'OPEN' ? { ...t, status: 'CLOSED' as const } : t,
            ),
            systemStatus: {
              ...state.systemStatus,
              bot_running: false,
              connections: state.systemStatus.connections.map((c) => ({
                ...c, connected: false, latency_ms: undefined,
              })),
            },
          }))
          addLogEntry({ level: 'success', message: 'All positions closed — bot stopped' })
        }, 2500)
      } else {
        addLogEntry({ level: 'info', message: 'Bot stopped — positions held (dev mode)' })
        set((state) => ({
          botState: 'idle',
          systemStatus: {
            ...state.systemStatus,
            bot_running: false,
            connections: state.systemStatus.connections.map((c) => ({
              ...c, connected: false, latency_ms: undefined,
            })),
          },
        }))
      }
    }
  },

  cancelStop: () => set({ showStopDialog: false }),

  setBotStateFromEngine: (state: string) => {
    const stateMap: Record<string, BotState> = {
      idle: 'idle', starting: 'scanning', running: 'scanning', stopping: 'stopping', error: 'idle',
    }
    const mapped = stateMap[state] || 'idle'
    set({
      botState: mapped,
      systemStatus: { ...get().systemStatus, bot_running: mapped === 'scanning' },
    })
  },

  updateConnections: (connections: ConnectionStatus[]) => {
    set((state) => ({
      systemStatus: { ...state.systemStatus, connections },
    }))
  },

  updateFromTick: (data) => {
    set((state) => {
      const updates: Partial<typeof state> = {}

      if (data.connections?.length) {
        updates.systemStatus = { ...state.systemStatus, connections: data.connections }
      }

      if (data.pnl) {
        updates.pnl = data.pnl
      }

      if (data.prices) {
        updates.livePrices = { ...state.livePrices, ...data.prices }
      }

      if (data.positions) {
        updates.trades = data.positions.map((p) => ({
          id: p.id,
          exchange: p.exchange,
          pair: p.pair,
          side: (p.side === 'long' ? 'LONG' : 'SHORT') as 'LONG' | 'SHORT',
          entry_price: p.entry_price,
          current_price: p.current_price,
          quantity: p.quantity,
          pnl: p.unrealized_pnl,
          pnl_pct: p.unrealized_pnl_pct,
          status: 'OPEN' as const,
          strategy: p.strategy,
          opened_at: p.opened_at,
        }))
      }

      return updates
    })
  },
}))

import { useEffect } from 'react'
import { useAppStore } from '@/stores/useAppStore'
import type { ConnectionStatus, PnlSummary } from '@/lib/types'

type LogEvent = { level: string; message: string }
type StateChangeEvent = { state: string }

interface PositionPayload {
  id: string
  exchange: string
  pair: string
  side: 'long' | 'short'
  entry_price: number
  current_price: number
  quantity: number
  unrealized_pnl: number
  unrealized_pnl_pct: number
  stop_loss: number
  take_profit: number
  time_limit: string
  strategy: string
  opened_at: string
}

type TickEvent = {
  timestamp: string
  prices: Record<string, number>
  positions: PositionPayload[]
  signals: unknown[]
  pnl: PnlSummary
  connections: ConnectionStatus[]
}

function isTauri(): boolean {
  return '__TAURI_INTERNALS__' in window
}

export function useBotEvents() {
  const {
    addLogEntry,
    setBotStateFromEngine,
    updateConnections,
    updateFromTick,
  } = useAppStore()

  useEffect(() => {
    if (!isTauri()) return

    const unlisten: Array<() => void> = []

    async function setup() {
      const { listen } = await import('@tauri-apps/api/event')

      const u1 = await listen<LogEvent>('bot:log', (event) => {
        const { level, message } = event.payload
        const validLevel = ['info', 'warn', 'error', 'success'].includes(level)
          ? (level as 'info' | 'warn' | 'error' | 'success')
          : 'info'
        addLogEntry({ level: validLevel, message })
      })
      unlisten.push(u1)

      const u2 = await listen<StateChangeEvent>('bot:state-change', (event) => {
        setBotStateFromEngine(event.payload.state)
      })
      unlisten.push(u2)

      const u3 = await listen<TickEvent>('bot:tick', (event) => {
        const { connections, positions, pnl, prices } = event.payload
        updateFromTick({ connections, positions, pnl, prices })
      })
      unlisten.push(u3)
    }

    setup().catch(console.error)

    return () => {
      unlisten.forEach((fn) => fn())
    }
  }, [addLogEntry, setBotStateFromEngine, updateConnections, updateFromTick])
}

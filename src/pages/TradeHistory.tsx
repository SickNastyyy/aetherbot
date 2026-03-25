import { useState, useEffect } from 'react'
import { motion } from 'framer-motion'
import { History, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'

interface TradeRecord {
  trade_id: string
  exchange: string
  pair: string
  side: string
  entry_price: number
  exit_price: number | null
  quantity: number
  pnl: number | null
  pnl_pct: number | null
  status: string
  strategy: string | null
  exit_reason: string | null
  opened_at: string
  closed_at: string | null
}

function isTauri(): boolean {
  return '__TAURI_INTERNALS__' in window
}

export function TradeHistory() {
  const [trades, setTrades] = useState<TradeRecord[]>([])
  const [loading, setLoading] = useState(false)

  const loadTrades = async () => {
    if (!isTauri()) return
    setLoading(true)
    try {
      const { invoke } = await import('@tauri-apps/api/core')
      const result = await invoke<TradeRecord[]>('get_trade_history', { limit: 100 })
      setTrades(result)
    } catch (e) {
      console.error('Failed to load trade history:', e)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    loadTrades()
  }, [])

  const totalPnl = trades.reduce((sum, t) => sum + (t.pnl ?? 0), 0)
  const closedTrades = trades.filter((t) => t.status === 'CLOSED')
  const winningTrades = closedTrades.filter((t) => (t.pnl ?? 0) > 0)
  const winRate = closedTrades.length > 0 ? (winningTrades.length / closedTrades.length) * 100 : 0

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.3 }}
      className="h-full p-3 flex flex-col gap-3"
    >
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <History size={16} className="text-holo-cyan" />
          <h2 className="text-[15px] font-semibold text-holo-text-bright">Trade History</h2>
          <span className="text-[11px] text-holo-text-muted font-mono px-2 py-0.5 rounded-md bg-white/[0.03]">
            {trades.length} trades
          </span>
        </div>
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-6 text-[12px] font-mono">
            <span className={totalPnl >= 0 ? 'text-holo-green' : 'text-holo-red'}>
              P&L: {totalPnl >= 0 ? '+' : ''}${totalPnl.toFixed(2)}
            </span>
            <span className="text-holo-text-muted">
              Win Rate: {winRate.toFixed(1)}%
            </span>
            <span className="text-holo-text-muted">
              Closed: {closedTrades.length}
            </span>
          </div>
          <Button
            size="sm"
            onClick={loadTrades}
            disabled={loading}
            className="h-7 px-3 bg-holo-surface-2 hover:bg-holo-surface-3 text-holo-text border border-holo-border text-[11px]"
          >
            <RefreshCw size={11} className={loading ? 'animate-spin' : ''} />
          </Button>
        </div>
      </div>

      {/* Table */}
      <div className="panel-glow overflow-hidden flex-1 flex flex-col">
        <div className="overflow-auto flex-1">
          <table className="w-full">
            <thead>
              <tr className="border-b border-holo-border sticky top-0 bg-holo-surface">
                {['STATUS', 'PAIR', 'SIDE', 'STRATEGY', 'ENTRY', 'EXIT', 'QTY', 'PNL', 'EXIT REASON', 'TIME'].map((h) => (
                  <th
                    key={h}
                    className={`text-[10px] text-holo-text-muted/60 font-semibold px-3 py-2.5 tracking-wider uppercase whitespace-nowrap ${
                      ['ENTRY', 'EXIT', 'QTY', 'PNL'].includes(h) ? 'text-right' : 'text-left'
                    }`}
                  >
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {trades.map((trade) => (
                <tr
                  key={trade.trade_id}
                  className="border-b border-holo-border/30 hover:bg-white/[0.015] transition-colors"
                >
                  <td className="px-3 py-2">
                    <span className={`text-[10px] font-bold px-2 py-0.5 rounded-md ${
                      trade.status === 'OPEN'
                        ? 'bg-holo-cyan/10 text-holo-cyan/90'
                        : 'bg-holo-text-muted/10 text-holo-text-muted/60'
                    }`}>
                      {trade.status}
                    </span>
                  </td>
                  <td className="px-3 py-2">
                    <div className="flex items-center gap-2">
                      <span className="text-[12px] font-mono text-holo-text-bright">{trade.pair}</span>
                      <span className="text-[9px] text-holo-text-muted/40 font-mono">{trade.exchange}</span>
                    </div>
                  </td>
                  <td className="px-3 py-2">
                    <span className={`text-[10px] font-bold px-2 py-0.5 rounded-md ${
                      trade.side === 'LONG'
                        ? 'bg-holo-green/10 text-holo-green/90'
                        : 'bg-holo-red/10 text-holo-red/90'
                    }`}>
                      {trade.side}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-[11px] text-holo-text-muted/60">
                    {trade.strategy ?? '—'}
                  </td>
                  <td className="text-right px-3 py-2 text-[11px] font-mono text-holo-text/60">
                    ${trade.entry_price.toFixed(4)}
                  </td>
                  <td className="text-right px-3 py-2 text-[11px] font-mono text-holo-text-bright">
                    {trade.exit_price != null ? `$${trade.exit_price.toFixed(4)}` : '—'}
                  </td>
                  <td className="text-right px-3 py-2 text-[11px] font-mono text-holo-text/60">
                    {trade.quantity.toFixed(4)}
                  </td>
                  <td className="text-right px-3 py-2">
                    {trade.pnl != null ? (
                      <div className="flex flex-col items-end">
                        <span className={`text-[11px] font-mono font-semibold ${
                          trade.pnl >= 0 ? 'text-holo-green' : 'text-holo-red'
                        }`}>
                          {trade.pnl >= 0 ? '+' : ''}${trade.pnl.toFixed(2)}
                        </span>
                        {trade.pnl_pct != null && (
                          <span className="text-[9px] text-holo-text-muted font-mono">
                            {trade.pnl_pct >= 0 ? '+' : ''}{trade.pnl_pct.toFixed(2)}%
                          </span>
                        )}
                      </div>
                    ) : (
                      <span className="text-[11px] text-holo-text-muted">—</span>
                    )}
                  </td>
                  <td className="px-3 py-2 text-[10px] text-holo-text-muted/50">
                    {trade.exit_reason ?? '—'}
                  </td>
                  <td className="px-3 py-2 text-[10px] text-holo-text-muted/40 font-mono whitespace-nowrap">
                    {new Date(trade.opened_at).toLocaleString('en-US', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {trades.length === 0 && (
            <div className="flex items-center justify-center py-12 text-holo-text-muted/40 text-[12px]">
              {isTauri() ? 'No trades recorded yet — start the bot to begin' : 'Trade history available in desktop mode only'}
            </div>
          )}
        </div>
      </div>
    </motion.div>
  )
}

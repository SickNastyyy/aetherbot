import { AlertTriangle, Hand, Zap } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useAppStore } from '@/stores/useAppStore'

export function StopConfirmDialog() {
  const { showStopDialog, cancelStop, confirmStop, trades } = useAppStore()

  if (!showStopDialog) return null

  const openTrades = trades.filter((t) => t.status === 'OPEN')
  const totalPnl = openTrades.reduce((sum, t) => sum + t.pnl, 0)
  const estimatedSlippage = openTrades.length * 0.15 // ~0.15% avg slippage per DEX swap

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/60 backdrop-blur-sm"
        onClick={cancelStop}
      />

      {/* Dialog */}
      <div className="relative w-[480px] bg-holo-surface border border-holo-border rounded-xl shadow-2xl shadow-black/40 overflow-hidden">
        {/* Header */}
        <div className="px-6 py-4 border-b border-holo-border flex items-center gap-3">
          <div className="p-2 rounded-lg bg-holo-yellow/10">
            <AlertTriangle size={16} className="text-holo-yellow" />
          </div>
          <div>
            <h3 className="text-[14px] font-semibold text-holo-text-bright">Stop Trading Bot</h3>
            <p className="text-[11px] text-holo-text-muted mt-0.5">
              Choose how to handle your {openTrades.length} open position{openTrades.length !== 1 ? 's' : ''}
            </p>
          </div>
        </div>

        {/* Open Positions Summary */}
        {openTrades.length > 0 && (
          <div className="px-6 py-3 border-b border-holo-border bg-holo-dark/50">
            <div className="text-[10px] text-holo-text-muted/60 font-semibold uppercase tracking-wider mb-2">
              Open Positions
            </div>
            <div className="space-y-1.5">
              {openTrades.map((trade) => (
                <div key={trade.id} className="flex items-center justify-between text-[11px]">
                  <div className="flex items-center gap-2">
                    <span className={`font-bold text-[9px] px-1.5 py-0.5 rounded ${
                      trade.side === 'LONG' ? 'bg-holo-green/10 text-holo-green/80' : 'bg-holo-red/10 text-holo-red/80'
                    }`}>
                      {trade.side}
                    </span>
                    <span className="font-mono text-holo-text-bright">{trade.pair}</span>
                    <span className="text-holo-text-muted/40 font-mono text-[10px]">{trade.exchange}</span>
                  </div>
                  <span className={`font-mono font-semibold ${trade.pnl >= 0 ? 'text-holo-green' : 'text-holo-red'}`}>
                    {trade.pnl >= 0 ? '+' : ''}${trade.pnl.toFixed(2)}
                  </span>
                </div>
              ))}
            </div>
            <div className="mt-3 pt-2.5 border-t border-holo-border flex items-center justify-between text-[11px]">
              <span className="text-holo-text-muted">Net unrealized P&L</span>
              <span className={`font-mono font-bold ${totalPnl >= 0 ? 'text-holo-green' : 'text-holo-red'}`}>
                {totalPnl >= 0 ? '+' : ''}${totalPnl.toFixed(2)}
              </span>
            </div>
            <div className="flex items-center justify-between text-[10px] mt-1">
              <span className="text-holo-text-muted/50">Est. slippage cost</span>
              <span className="font-mono text-holo-yellow/60">~{estimatedSlippage.toFixed(2)}%</span>
            </div>
          </div>
        )}

        {/* Actions */}
        <div className="px-6 py-4 flex gap-3">
          <Button
            onClick={() => confirmStop('hold')}
            className="flex-1 h-10 bg-holo-surface-2 hover:bg-holo-surface-3 text-holo-text-bright border border-holo-border text-[12px] font-semibold gap-2"
          >
            <Hand size={14} />
            Stop & Hold
            <span className="text-[10px] text-holo-text-muted font-normal ml-1">keep positions</span>
          </Button>
          <Button
            onClick={() => confirmStop('liquidate')}
            className="flex-1 h-10 bg-holo-red/12 hover:bg-holo-red/20 text-holo-red border border-holo-red/20 text-[12px] font-semibold gap-2"
          >
            <Zap size={14} />
            Stop & Liquidate
            <span className="text-[10px] text-holo-red/50 font-normal ml-1">close all</span>
          </Button>
        </div>

        {/* Cancel */}
        <div className="px-6 pb-4 -mt-1">
          <button
            onClick={cancelStop}
            className="w-full text-center text-[11px] text-holo-text-muted/50 hover:text-holo-text-muted transition-colors py-1"
          >
            Cancel — keep bot running
          </button>
        </div>
      </div>
    </div>
  )
}

import { motion, AnimatePresence } from 'framer-motion'
import { useAppStore } from '@/stores/useAppStore'

export function TradesTable() {
  const { trades } = useAppStore()
  const openCount = trades.filter((t) => t.status === 'OPEN').length

  return (
    <div className="panel-glow overflow-hidden flex flex-col h-full">
      <div className="px-4 py-2.5 border-b border-holo-border flex items-center justify-between shrink-0">
        <span className="text-[12px] font-semibold text-holo-text-bright">
          Active Trades
        </span>
        <span className="text-[10px] text-holo-text-muted font-mono px-2 py-0.5 rounded-md bg-white/[0.03]">
          {openCount} open
        </span>
      </div>
      <div className="overflow-auto flex-1">
        <table className="w-full">
          <thead>
            <tr className="border-b border-holo-border">
              {['PAIR', 'SIDE', 'ENTRY', 'CURRENT', 'PNL', 'STRATEGY'].map((h) => (
                <th
                  key={h}
                  className={`text-[10px] text-holo-text-muted/60 font-semibold px-4 py-2.5 tracking-wider uppercase ${
                    h === 'ENTRY' || h === 'CURRENT' || h === 'PNL' ? 'text-right' : 'text-left'
                  }`}
                >
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            <AnimatePresence>
              {trades.map((trade, i) => (
                <motion.tr
                  key={trade.id}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ delay: i * 0.04 }}
                  className="border-b border-holo-border/30 hover:bg-white/[0.015] transition-colors"
                >
                  <td className="px-4 py-2.5">
                    <div className="flex items-center gap-2.5">
                      <div
                        className={`w-1.5 h-1.5 rounded-full ${
                          trade.status === 'OPEN' ? 'bg-holo-cyan shadow-[0_0_6px_rgba(0,240,255,0.3)]' : 'bg-holo-text-muted/30'
                        }`}
                      />
                      <span className="text-[12px] font-mono text-holo-text-bright font-medium">{trade.pair}</span>
                      <span className="text-[9px] text-holo-text-muted/50 font-mono">{trade.exchange}</span>
                    </div>
                  </td>
                  <td className="px-4 py-2.5">
                    <span
                      className={`text-[10px] font-bold px-2 py-0.5 rounded-md ${
                        trade.side === 'LONG'
                          ? 'bg-holo-green/10 text-holo-green/90'
                          : 'bg-holo-red/10 text-holo-red/90'
                      }`}
                    >
                      {trade.side}
                    </span>
                  </td>
                  <td className="text-right px-4 py-2.5 text-[12px] font-mono text-holo-text/60">
                    ${trade.entry_price.toLocaleString()}
                  </td>
                  <td className="text-right px-4 py-2.5 text-[12px] font-mono text-holo-text-bright">
                    ${trade.current_price.toLocaleString()}
                  </td>
                  <td className="text-right px-4 py-2.5">
                    <div className="flex flex-col items-end">
                      <span
                        className={`text-[12px] font-mono font-semibold ${
                          trade.pnl_pct >= 0 ? 'text-holo-green' : 'text-holo-red'
                        }`}
                      >
                        {trade.pnl_pct >= 0 ? '+' : ''}
                        {trade.pnl_pct.toFixed(2)}%
                      </span>
                      <span className="text-[10px] text-holo-text-muted font-mono">
                        ${Math.abs(trade.pnl).toFixed(2)}
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-2.5">
                    <span className="text-[11px] text-holo-text-muted/60 px-2 py-0.5 rounded bg-white/[0.02]">
                      {trade.strategy}
                    </span>
                  </td>
                </motion.tr>
              ))}
            </AnimatePresence>
          </tbody>
        </table>
      </div>
    </div>
  )
}

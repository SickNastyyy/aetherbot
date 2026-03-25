import { motion } from 'framer-motion'
import { ArrowUpRight, ArrowDownRight, TrendingUp, Target, BarChart3, Zap } from 'lucide-react'
import { useAppStore } from '@/stores/useAppStore'

export function PnlCards() {
  const { pnl } = useAppStore()

  const cards = [
    {
      label: 'Total PNL',
      value: `$${pnl.total_pnl.toLocaleString('en-US', { minimumFractionDigits: 2 })}`,
      delta: `${pnl.total_pnl_pct >= 0 ? '+' : ''}${pnl.total_pnl_pct.toFixed(2)}%`,
      positive: pnl.total_pnl >= 0,
      icon: TrendingUp,
      color: pnl.total_pnl >= 0 ? 'green' : 'red',
    },
    {
      label: "Today's PNL",
      value: `$${pnl.today_pnl.toLocaleString('en-US', { minimumFractionDigits: 2 })}`,
      delta: 'last 24h',
      positive: pnl.today_pnl >= 0,
      icon: Zap,
      color: pnl.today_pnl >= 0 ? 'green' : 'red',
    },
    {
      label: 'Win Rate',
      value: `${pnl.win_rate.toFixed(1)}%`,
      delta: `${pnl.total_trades} trades`,
      positive: true,
      icon: Target,
      color: 'cyan',
    },
    {
      label: 'Open Positions',
      value: pnl.active_positions.toString(),
      delta: 'active now',
      positive: true,
      icon: BarChart3,
      color: 'purple',
    },
  ]

  const colorMap = {
    green: { text: 'text-holo-green', bg: 'bg-holo-green/8', border: 'border-holo-green/10' },
    red: { text: 'text-holo-red', bg: 'bg-holo-red/8', border: 'border-holo-red/10' },
    cyan: { text: 'text-holo-cyan', bg: 'bg-holo-cyan/8', border: 'border-holo-cyan/10' },
    purple: { text: 'text-holo-purple', bg: 'bg-holo-purple/8', border: 'border-holo-purple/10' },
  }

  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-2.5">
      {cards.map((card, i) => {
        const colors = colorMap[card.color as keyof typeof colorMap]
        return (
          <motion.div
            key={card.label}
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: i * 0.05, duration: 0.35 }}
            className="panel-glow p-4 group hover:border-holo-border-bright transition-all duration-300"
          >
            <div className="flex items-center justify-between mb-3">
              <span className="text-[11px] text-holo-text-muted font-medium uppercase tracking-wider">
                {card.label}
              </span>
              <div className={`p-1.5 rounded-md ${colors.bg}`}>
                <card.icon size={12} className={colors.text} />
              </div>
            </div>
            <div className={`text-[22px] font-bold font-mono ${colors.text} tracking-tight leading-none`}>
              {card.value}
            </div>
            <div className="flex items-center gap-1.5 mt-2.5">
              {card.label.includes('PNL') && (
                card.positive
                  ? <ArrowUpRight size={12} className="text-holo-green" />
                  : <ArrowDownRight size={12} className="text-holo-red" />
              )}
              <span className="text-[11px] text-holo-text-muted">{card.delta}</span>
            </div>
          </motion.div>
        )
      })}
    </div>
  )
}

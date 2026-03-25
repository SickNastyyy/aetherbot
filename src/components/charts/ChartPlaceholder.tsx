import { useId, useMemo } from 'react'
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, ResponsiveContainer, Tooltip } from 'recharts'
import { useAppStore } from '@/stores/useAppStore'

function generateMockPriceData(pair: string) {
  const base = pair.includes('BTC') || pair.includes('WBTC')
    ? 67000
    : pair.includes('ETH')
      ? 3400
      : pair.includes('SOL')
        ? 185
        : pair.includes('JUP')
          ? 1.4
          : 100
  const data = []
  let price = base
  for (let i = 0; i < 120; i++) {
    price += (Math.random() - 0.48) * (base * 0.004)
    data.push({
      time: `${Math.floor(i / 4).toString().padStart(2, '0')}:${(i % 4 * 15).toString().padStart(2, '0')}`,
      price: Math.round(price * 100) / 100,
    })
  }
  return data
}

export function ChartPlaceholder() {
  const gradientId = useId()
  const { selectedPair } = useAppStore()
  const data = useMemo(() => generateMockPriceData(selectedPair), [selectedPair])
  const isUp = data[data.length - 1].price >= data[0].price
  const color = isUp ? '#22d97f' : '#ef4444'
  const change = ((data[data.length - 1].price - data[0].price) / data[0].price * 100).toFixed(2)

  return (
    <div className="panel-glow overflow-hidden h-full flex flex-col">
      <div className="px-4 py-3 border-b border-holo-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="text-[14px] font-semibold text-holo-text-bright font-mono">
            {selectedPair}
          </span>
          <span className="text-[10px] text-holo-text-muted/60 px-2 py-0.5 rounded-md bg-white/[0.03] font-mono">
            15m
          </span>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-[16px] font-bold font-mono text-holo-text-bright tabular-nums">
            ${data[data.length - 1].price.toLocaleString()}
          </span>
          <span
            className={`text-[12px] font-mono font-semibold px-2 py-0.5 rounded-md ${
              isUp
                ? 'text-holo-green bg-holo-green/8'
                : 'text-holo-red bg-holo-red/8'
            }`}
          >
            {isUp ? '+' : ''}{change}%
          </span>
        </div>
      </div>
      <div className="flex-1 min-h-0">
        <ResponsiveContainer width="100%" height="100%" minWidth={100} minHeight={80}>
          <AreaChart data={data} margin={{ top: 16, right: 16, bottom: 4, left: 4 }}>
            <defs>
              <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor={color} stopOpacity={0.12} />
                <stop offset="100%" stopColor={color} stopOpacity={0} />
              </linearGradient>
            </defs>
            <CartesianGrid
              strokeDasharray="3 3"
              stroke="rgba(255,255,255,0.03)"
              vertical={false}
            />
            <XAxis
              dataKey="time"
              tick={{ fill: '#3d4450', fontSize: 9, fontFamily: 'var(--font-mono)' }}
              axisLine={false}
              tickLine={false}
              interval={23}
            />
            <YAxis
              domain={['auto', 'auto']}
              tick={{ fill: '#3d4450', fontSize: 9, fontFamily: 'var(--font-mono)' }}
              axisLine={false}
              tickLine={false}
              width={55}
              tickFormatter={(v: number) => `$${v.toLocaleString()}`}
            />
            <Tooltip
              contentStyle={{
                background: 'rgba(14,18,24,0.95)',
                border: '1px solid rgba(255,255,255,0.08)',
                borderRadius: '8px',
                fontSize: '11px',
                fontFamily: 'var(--font-mono)',
                color: '#e2e8f0',
                boxShadow: '0 12px 40px rgba(0,0,0,0.5)',
                backdropFilter: 'blur(8px)',
              }}
              labelStyle={{ color: '#7d8590', fontSize: '10px' }}
              cursor={{ stroke: 'rgba(0,240,255,0.15)', strokeDasharray: '3 3' }}
            />
            <Area
              type="monotone"
              dataKey="price"
              stroke={color}
              strokeWidth={1.5}
              fill={`url(#${gradientId})`}
              animationDuration={600}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}

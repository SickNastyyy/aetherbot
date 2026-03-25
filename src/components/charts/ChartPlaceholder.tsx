import { useId, useMemo, useRef, useEffect, useCallback, useState } from 'react'
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, ResponsiveContainer, Tooltip } from 'recharts'
import { useAppStore } from '@/stores/useAppStore'

interface PricePoint {
  time: string
  price: number
}

export function ChartPlaceholder() {
  const gradientId = useId()
  const { selectedPair, livePrices, botState } = useAppStore()
  const dataRef = useRef<PricePoint[]>([])
  const lastPriceRef = useRef<number | null>(null)
  const [renderTick, setRenderTick] = useState(0)

  // Get live price for selected pair
  const livePrice = livePrices[selectedPair]

  // Reset data when pair changes
  useEffect(() => {
    dataRef.current = []
    lastPriceRef.current = null
    setRenderTick(0)
  }, [selectedPair])

  // Push live price into rolling data array + trigger re-render
  const pushPrice = useCallback((price: number) => {
    const now = new Date()
    const timeStr = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}:${now.getSeconds().toString().padStart(2, '0')}`
    dataRef.current = [
      ...dataRef.current.slice(-119),
      { time: timeStr, price },
    ]
    lastPriceRef.current = price
    setRenderTick((n) => n + 1)
  }, [])

  // Push live price on each update
  useEffect(() => {
    if (livePrice && livePrice !== lastPriceRef.current) {
      pushPrice(livePrice)
    }
  }, [livePrice, pushPrice])

  // Generate mock data for pairs without live prices
  const mockData = useMemo(() => {
    const base = selectedPair.includes('BTC') || selectedPair.includes('WBTC')
      ? 67000
      : selectedPair.includes('ETH')
        ? 3400
        : selectedPair.includes('SOL')
          ? 185
          : selectedPair.includes('JUP')
            ? 1.4
            : 100
    const data: PricePoint[] = []
    let price = base
    for (let i = 0; i < 120; i++) {
      price += (Math.random() - 0.48) * (base * 0.004)
      data.push({
        time: `${Math.floor(i / 4).toString().padStart(2, '0')}:${(i % 4 * 15).toString().padStart(2, '0')}`,
        price: Math.round(price * 100) / 100,
      })
    }
    return data
  }, [selectedPair])

  // Use live data if we have enough points, otherwise mock
  const isLive = botState === 'scanning' && dataRef.current.length > 2
  const data = isLive ? dataRef.current : mockData

  const isUp = data.length >= 2 && data[data.length - 1].price >= data[0].price
  const color = isUp ? '#22d97f' : '#ef4444'
  const currentPrice = data[data.length - 1]?.price ?? 0
  const startPrice = data[0]?.price ?? currentPrice
  const change = startPrice > 0 ? ((currentPrice - startPrice) / startPrice * 100).toFixed(2) : '0.00'

  return (
    <div className="panel-glow overflow-hidden h-full flex flex-col">
      <div className="px-4 py-3 border-b border-holo-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <span className="text-[14px] font-semibold text-holo-text-bright font-mono">
            {selectedPair}
          </span>
          <span className="text-[10px] text-holo-text-muted/60 px-2 py-0.5 rounded-md bg-white/[0.03] font-mono">
            {isLive ? 'LIVE' : '15m'}
          </span>
          {isLive && (
            <span className="flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-holo-green animate-pulse" />
              <span className="text-[9px] text-holo-green/70 font-mono">STREAMING</span>
            </span>
          )}
        </div>
        <div className="flex items-center gap-3">
          <span className="text-[16px] font-bold font-mono text-holo-text-bright tabular-nums">
            ${currentPrice.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 4 })}
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
              interval={Math.max(Math.floor(data.length / 6), 1)}
            />
            <YAxis
              domain={['auto', 'auto']}
              tick={{ fill: '#3d4450', fontSize: 9, fontFamily: 'var(--font-mono)' }}
              axisLine={false}
              tickLine={false}
              width={60}
              tickFormatter={(v: number) =>
                v >= 1000 ? `$${(v / 1000).toFixed(1)}k` : `$${v.toFixed(2)}`
              }
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
              animationDuration={isLive ? 200 : 600}
              isAnimationActive={!isLive}
            />
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  )
}

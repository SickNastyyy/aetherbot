import { useEffect, useRef, useCallback } from 'react'
import { motion } from 'framer-motion'
import { PnlCards } from '@/components/dashboard/PnlCards'
import { TradesTable } from '@/components/dashboard/TradesTable'
import { TradeLog } from '@/components/dashboard/TradeLog'
import { SystemStatus } from '@/components/dashboard/SystemStatus'
import { ChartPlaceholder } from '@/components/charts/ChartPlaceholder'
import { HoloOrb } from '@/components/orb/HoloOrb'
import { StopConfirmDialog } from '@/components/dashboard/StopConfirmDialog'
import { useBotEvents } from '@/hooks/useBotEvents'
import { useAppStore } from '@/stores/useAppStore'
import { getRandomLogEntry } from '@/lib/constants'

export function Dashboard() {
  useBotEvents()
  const { addLogEntry, incrementUptime, botState } = useAppStore()
  const logTimerRef = useRef<ReturnType<typeof setTimeout>>()

  const scheduleLog = useCallback(() => {
    logTimerRef.current = setTimeout(() => {
      if (useAppStore.getState().botState === 'scanning') {
        addLogEntry(getRandomLogEntry())
      }
      scheduleLog()
    }, 2500 + Math.random() * 4000)
  }, [addLogEntry])

  useEffect(() => {
    const uptimeTimer = setInterval(incrementUptime, 1000)

    addLogEntry({ level: 'info', message: 'AetherBot v0.2 initialized' })
    addLogEntry({ level: 'info', message: 'Paper trading mode active' })
    addLogEntry({ level: 'warn', message: 'Press Start to begin scanning markets' })
    scheduleLog()

    return () => {
      clearInterval(uptimeTimer)
      if (logTimerRef.current) clearTimeout(logTimerRef.current)
    }
  }, [addLogEntry, incrementUptime, scheduleLog])

  return (
    <>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.35 }}
        className="h-full p-3 flex flex-col gap-2.5"
      >
        {/* Row 1: PNL Cards */}
        <PnlCards />

        {/* Row 2: Chart + Sidebar column */}
        <div className="flex-1 min-h-0 grid grid-cols-[1fr_200px] gap-2.5">
          <ChartPlaceholder />
          <div className="flex flex-col gap-2.5">
            <div className="panel-glow flex items-center justify-center" style={{ height: '160px' }}>
              <HoloOrb />
            </div>
            <div className="flex-1 min-h-0">
              <SystemStatus />
            </div>
          </div>
        </div>

        {/* Row 3: Trades + Log */}
        <div className="grid grid-cols-[1.4fr_1fr] gap-2.5" style={{ height: '230px' }}>
          <TradesTable />
          <TradeLog />
        </div>
      </motion.div>

      <StopConfirmDialog />
    </>
  )
}

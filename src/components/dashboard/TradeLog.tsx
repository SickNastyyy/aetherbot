import { useEffect, useRef } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import { useAppStore } from '@/stores/useAppStore'

const levelDot: Record<string, string> = {
  info: 'bg-holo-cyan/70',
  warn: 'bg-holo-yellow/70',
  error: 'bg-holo-red/70',
  success: 'bg-holo-green/70',
}

export function TradeLog() {
  const { tradeLog } = useAppStore()
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [tradeLog.length])

  return (
    <div className="panel-glow overflow-hidden flex flex-col h-full">
      <div className="px-4 py-2.5 border-b border-holo-border shrink-0 flex items-center justify-between">
        <span className="text-[12px] font-semibold text-holo-text-bright">
          Activity Log
        </span>
        <span className="text-[10px] text-holo-text-muted font-mono px-2 py-0.5 rounded-md bg-white/[0.03]">
          {tradeLog.length}
        </span>
      </div>
      <div className="flex-1 overflow-auto px-3 py-2">
        <div className="space-y-0">
          <AnimatePresence initial={false}>
            {tradeLog.map((entry) => (
              <motion.div
                key={entry.id}
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: 'auto' }}
                transition={{ duration: 0.15 }}
                className="flex items-start gap-2.5 py-[4px] font-mono text-[11px]"
              >
                <span className="text-holo-text-muted/50 shrink-0 tabular-nums leading-[18px]">
                  {new Date(entry.timestamp).toLocaleTimeString('en-US', { hour12: false })}
                </span>
                <span className={`w-1.5 h-1.5 rounded-full shrink-0 mt-[6px] ${levelDot[entry.level]}`} />
                <span className="text-holo-text/80 leading-[18px]">{entry.message}</span>
              </motion.div>
            ))}
          </AnimatePresence>
          <div ref={bottomRef} />
        </div>
        {tradeLog.length === 0 && (
          <div className="text-center py-8 text-holo-text-muted/50 text-[11px]">
            Waiting for activity...
          </div>
        )}
      </div>
    </div>
  )
}

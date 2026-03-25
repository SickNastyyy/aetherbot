import { useEffect, useState } from 'react'
import { Play, Square, Loader2, Wifi, WifiOff, Circle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useAppStore } from '@/stores/useAppStore'
import { useSettingsStore } from '@/stores/useSettingsStore'

export function TopBar() {
  const { systemStatus, botState, startBot, requestStop } = useAppStore()
  const { config } = useSettingsStore()
  const [time, setTime] = useState(new Date())

  useEffect(() => {
    const timer = setInterval(() => setTime(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  const isRunning = botState === 'scanning'
  const isStopping = botState === 'stopping'

  return (
    <div className="h-12 flex items-center justify-between px-5 border-b border-holo-border bg-holo-dark/80 backdrop-blur-sm">
      {/* Left: Bot Control */}
      <div className="flex items-center gap-4">
        {isRunning || isStopping ? (
          <Button
            size="sm"
            onClick={requestStop}
            disabled={isStopping}
            className="h-8 px-4 bg-holo-red/15 hover:bg-holo-red/25 text-holo-red border border-holo-red/25 text-xs font-semibold gap-2 transition-all"
          >
            {isStopping ? (
              <>
                <Loader2 size={13} className="animate-spin" />
                Stopping...
              </>
            ) : (
              <>
                <Square size={11} fill="currentColor" />
                Stop Bot
              </>
            )}
          </Button>
        ) : (
          <Button
            size="sm"
            onClick={startBot}
            className="h-8 px-4 bg-holo-green/15 hover:bg-holo-green/25 text-holo-green border border-holo-green/25 text-xs font-semibold gap-2 transition-all"
          >
            <Play size={12} fill="currentColor" />
            Start Bot
          </Button>
        )}

        <div className="h-5 w-px bg-holo-border" />

        <div
          className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium tracking-wide ${
            config.paper_mode
              ? 'bg-holo-yellow/8 text-holo-yellow/80 border border-holo-yellow/10'
              : 'bg-holo-red/8 text-holo-red border border-holo-red/10'
          }`}
        >
          <Circle size={5} fill="currentColor" />
          {config.paper_mode ? 'PAPER' : 'LIVE'}
        </div>

        {!config.paper_mode && (
          <span className="text-[10px] text-holo-red/50 font-medium">Real funds at risk</span>
        )}
      </div>

      {/* Right: Connections + Time */}
      <div className="flex items-center gap-5">
        <div className="flex items-center gap-4">
          {systemStatus.connections.map((conn) => (
            <div key={conn.name} className="flex items-center gap-1.5">
              {conn.connected ? (
                <Wifi size={11} className="text-holo-green/80" />
              ) : (
                <WifiOff size={11} className="text-holo-text-muted/60" />
              )}
              <span
                className={`text-[11px] font-mono ${
                  conn.connected ? 'text-holo-text' : 'text-holo-text-muted/60'
                }`}
              >
                {conn.name}
              </span>
              {conn.connected && conn.latency_ms && (
                <span className="text-[9px] text-holo-text-muted font-mono">
                  {conn.latency_ms}ms
                </span>
              )}
            </div>
          ))}
        </div>

        <div className="h-4 w-px bg-holo-border" />

        <span className="text-[12px] text-holo-text-muted font-mono tabular-nums">
          {time.toLocaleTimeString('en-US', { hour12: false })}
        </span>
      </div>
    </div>
  )
}

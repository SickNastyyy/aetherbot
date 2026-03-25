import { Wifi, WifiOff, Clock, Bot, Shield } from 'lucide-react'
import { useAppStore } from '@/stores/useAppStore'
import { useSettingsStore } from '@/stores/useSettingsStore'

function formatUptime(secs: number) {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const s = secs % 60
  return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
}

function StatusRow({ icon: Icon, iconColor, label, value, valueColor }: {
  icon: typeof Wifi
  iconColor: string
  label: string
  value: string
  valueColor: string
}) {
  return (
    <div className="flex items-center justify-between py-[5px]">
      <div className="flex items-center gap-2">
        <Icon size={12} className={iconColor} />
        <span className="text-[11px] text-holo-text/80">{label}</span>
      </div>
      <span className={`text-[10px] font-mono font-semibold ${valueColor}`}>
        {value}
      </span>
    </div>
  )
}

export function SystemStatus() {
  const { systemStatus, botState } = useAppStore()
  const { config } = useSettingsStore()

  const engineLabel = botState === 'scanning' ? 'RUNNING' : botState === 'stopping' ? 'STOPPING' : 'IDLE'
  const engineColor = botState === 'scanning' ? 'text-holo-green' : botState === 'stopping' ? 'text-holo-orange' : 'text-holo-text-muted/60'

  return (
    <div className="panel-glow overflow-hidden flex flex-col h-full">
      <div className="px-4 py-2.5 border-b border-holo-border shrink-0">
        <span className="text-[12px] font-semibold text-holo-text-bright">
          System
        </span>
      </div>
      <div className="p-3 space-y-0 flex-1">
        {systemStatus.connections.map((conn) => (
          <StatusRow
            key={conn.name}
            icon={conn.connected ? Wifi : WifiOff}
            iconColor={conn.connected ? 'text-holo-green/80' : 'text-holo-text-muted/40'}
            label={conn.name}
            value={conn.connected ? `${conn.latency_ms ?? 0}ms` : 'OFF'}
            valueColor={conn.connected ? 'text-holo-green/80' : 'text-holo-text-muted/40'}
          />
        ))}
        <div className="border-t border-holo-border my-1.5" />
        <StatusRow
          icon={Bot}
          iconColor={engineColor}
          label="Engine"
          value={engineLabel}
          valueColor={engineColor}
        />
        <StatusRow
          icon={Shield}
          iconColor={config.paper_mode ? 'text-holo-yellow/70' : 'text-holo-red/80'}
          label="Mode"
          value={config.paper_mode ? 'PAPER' : 'LIVE'}
          valueColor={config.paper_mode ? 'text-holo-yellow/70' : 'text-holo-red'}
        />
        <StatusRow
          icon={Clock}
          iconColor="text-holo-cyan/60"
          label="Uptime"
          value={formatUptime(systemStatus.uptime_secs)}
          valueColor="text-holo-text/60"
        />
      </div>
    </div>
  )
}

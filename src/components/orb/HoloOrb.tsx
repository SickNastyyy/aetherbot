import { useAppStore } from '@/stores/useAppStore'

export function HoloOrb() {
  const { botState } = useAppStore()
  const isScanning = botState === 'scanning'
  const isStopping = botState === 'stopping'

  const coreColor = isStopping
    ? 'rgba(249,115,22,0.7)'  // orange
    : isScanning
      ? 'rgba(0,240,255,0.8)' // cyan
      : 'rgba(0,240,255,0.25)' // dim cyan

  const glowColor = isStopping
    ? 'rgba(249,115,22,0.3)'
    : isScanning
      ? 'rgba(0,240,255,0.35)'
      : 'rgba(0,240,255,0.1)'

  const label = isStopping ? 'stopping' : isScanning ? 'scanning' : 'standby'

  return (
    <div className="relative flex flex-col items-center justify-center w-full h-full gap-3">
      {/* Ambient glow */}
      <div
        className="absolute w-32 h-32 rounded-full blur-3xl"
        style={{
          background: `radial-gradient(circle, ${glowColor} 0%, transparent 70%)`,
          animation: isScanning ? 'orbPulse 2s ease-in-out infinite' : 'orbPulse 4s ease-in-out infinite',
        }}
      />

      {/* Outer ring */}
      <div
        className="absolute w-[72px] h-[72px] rounded-full"
        style={{
          border: `1px solid ${isScanning ? 'rgba(0,240,255,0.15)' : 'rgba(255,255,255,0.04)'}`,
          animation: isScanning ? 'orbSpin 12s linear infinite' : 'orbSpin 30s linear infinite',
        }}
      >
        <div
          className="absolute -top-[2px] left-1/2 -translate-x-1/2 w-1 h-1 rounded-full"
          style={{ background: isScanning ? 'rgba(0,240,255,0.5)' : 'rgba(255,255,255,0.1)' }}
        />
        <div
          className="absolute -bottom-[2px] left-1/2 -translate-x-1/2 w-1 h-1 rounded-full"
          style={{ background: isScanning ? 'rgba(168,85,247,0.4)' : 'rgba(255,255,255,0.06)' }}
        />
      </div>

      {/* Second ring */}
      {isScanning && (
        <div
          className="absolute w-[90px] h-[90px] rounded-full"
          style={{
            border: '1px solid rgba(0,240,255,0.06)',
            animation: 'orbSpin 20s linear infinite reverse',
          }}
        />
      )}

      {/* Core */}
      <div
        className="relative w-11 h-11 rounded-full"
        style={{
          background: `radial-gradient(circle at 35% 35%, ${coreColor}, ${coreColor.replace(/[\d.]+\)$/, '0.15)')} 70%, transparent)`,
          boxShadow: isScanning
            ? `0 0 30px ${glowColor}, 0 0 60px ${glowColor.replace(/[\d.]+\)$/, '0.05)')}`
            : `0 0 12px ${glowColor}`,
          animation: isScanning ? 'orbPulse 1.5s ease-in-out infinite' : 'orbPulse 4s ease-in-out infinite',
        }}
      >
        {/* Highlight */}
        <div
          className="absolute top-1.5 left-2 w-2 h-1.5 rounded-full opacity-30"
          style={{ background: 'radial-gradient(circle, white, transparent 70%)' }}
        />
      </div>

      {/* Label */}
      <div className={`text-[9px] font-mono tracking-[0.2em] uppercase ${
        isScanning ? 'text-holo-cyan/60' : isStopping ? 'text-holo-orange/60' : 'text-holo-text-muted/40'
      }`}>
        {label}
      </div>

      <style>{`
        @keyframes orbSpin { to { transform: rotate(360deg); } }
        @keyframes orbPulse {
          0%, 100% { transform: scale(1); opacity: 1; }
          50% { transform: scale(1.04); opacity: 0.8; }
        }
      `}</style>
    </div>
  )
}

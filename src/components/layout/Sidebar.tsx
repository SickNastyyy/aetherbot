import { motion } from 'framer-motion'
import {
  LayoutDashboard,
  Settings,
  TrendingUp,
  Activity,
  History,
} from 'lucide-react'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useAppStore } from '@/stores/useAppStore'
import { JUPITER_PAIRS, UNISWAP_PAIRS, KALSHI_EVENTS } from '@/lib/constants'

const navItems = [
  { id: 'dashboard' as const, label: 'Dashboard', icon: LayoutDashboard },
  { id: 'history' as const, label: 'Trade History', icon: History },
  { id: 'settings' as const, label: 'Settings', icon: Settings },
]

function MarketSection({
  title,
  items,
  icon: Icon,
  activeColor,
  iconColor,
  selectedPair,
  onSelect,
}: {
  title: string
  items: string[]
  icon: typeof TrendingUp
  activeColor: string
  iconColor: string
  selectedPair: string
  onSelect: (pair: string) => void
}) {
  return (
    <>
      <div className="px-3 pt-3 pb-1 text-[10px] font-semibold tracking-wider text-holo-text-muted/60 uppercase">
        {title}
      </div>
      {items.map((item) => {
        const isActive = selectedPair === item
        return (
          <button
            key={item}
            onClick={() => onSelect(item)}
            className={`w-full flex items-center gap-2 px-3 py-[6px] rounded-lg text-[12px] transition-all duration-150 ${
              isActive
                ? `${activeColor} font-medium`
                : 'text-holo-text/70 hover:text-holo-text-bright hover:bg-white/[0.02]'
            }`}
          >
            <Icon size={11} className={isActive ? iconColor : 'text-holo-text-muted/50'} />
            <span className={`font-mono ${item.length > 20 ? 'truncate' : ''}`}>{item}</span>
          </button>
        )
      })}
    </>
  )
}

export function Sidebar() {
  const { activePage, setActivePage, selectedPair, setSelectedPair, botState } = useAppStore()

  return (
    <div className="w-[220px] h-full flex flex-col border-r border-holo-border bg-holo-dark">
      {/* Logo */}
      <div className="h-12 flex items-center gap-2.5 px-4 border-b border-holo-border">
        <div className="relative w-5 h-5 shrink-0">
          <div className="absolute inset-0 rounded-full bg-holo-cyan/20" />
          <div className="absolute inset-[3px] rounded-full bg-holo-cyan/40" />
          <div className="absolute inset-[5px] rounded-full bg-holo-cyan" />
          {botState === 'scanning' && (
            <div className="absolute inset-0 rounded-full bg-holo-cyan/30 animate-ping" />
          )}
        </div>
        <span className="text-[13px] font-semibold text-holo-text-bright tracking-wide">
          AetherBot
        </span>
        <span className="ml-auto text-[9px] text-holo-text-muted/60 font-mono">v0.2</span>
      </div>

      {/* Navigation */}
      <nav className="p-2 space-y-0.5">
        {navItems.map((item) => {
          const isActive = activePage === item.id
          return (
            <motion.button
              key={item.id}
              onClick={() => setActivePage(item.id)}
              whileTap={{ scale: 0.98 }}
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-[13px] transition-all duration-150 ${
                isActive
                  ? 'bg-holo-cyan/8 text-holo-cyan'
                  : 'text-holo-text/70 hover:text-holo-text-bright hover:bg-white/[0.02]'
              }`}
            >
              <item.icon size={15} strokeWidth={isActive ? 2 : 1.5} />
              {item.label}
            </motion.button>
          )
        })}
      </nav>

      <div className="mx-3 border-t border-holo-border" />

      {/* Market Selector */}
      <ScrollArea className="flex-1">
        <div className="p-1.5 pb-4">
          <MarketSection
            title="Jupiter"
            items={JUPITER_PAIRS}
            icon={TrendingUp}
            activeColor="bg-holo-cyan/8 text-holo-cyan"
            iconColor="text-holo-cyan"
            selectedPair={selectedPair}
            onSelect={setSelectedPair}
          />
          <MarketSection
            title="Uniswap"
            items={UNISWAP_PAIRS}
            icon={TrendingUp}
            activeColor="bg-holo-cyan/8 text-holo-cyan"
            iconColor="text-holo-cyan"
            selectedPair={selectedPair}
            onSelect={setSelectedPair}
          />
          <MarketSection
            title="Kalshi"
            items={KALSHI_EVENTS}
            icon={Activity}
            activeColor="bg-holo-purple/8 text-holo-purple"
            iconColor="text-holo-purple"
            selectedPair={selectedPair}
            onSelect={setSelectedPair}
          />
        </div>
      </ScrollArea>

      {/* Footer status */}
      <div className="px-4 py-2.5 border-t border-holo-border flex items-center gap-2">
        <div className={`w-1.5 h-1.5 rounded-full ${
          botState === 'scanning' ? 'bg-holo-green animate-pulse' :
          botState === 'stopping' ? 'bg-holo-orange animate-pulse' :
          'bg-holo-text-muted/40'
        }`} />
        <span className="text-[10px] text-holo-text-muted font-medium">
          {botState === 'scanning' ? 'Scanning' : botState === 'stopping' ? 'Stopping...' : 'Idle'}
        </span>
      </div>
    </div>
  )
}

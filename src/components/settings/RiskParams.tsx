import { useState, useEffect } from 'react'
import { Save, AlertTriangle, Shield, Loader2, Timer, Zap, Brain } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { useSettingsStore } from '@/stores/useSettingsStore'
import { saveAppConfig, getAppConfig, tauriAvailable } from '@/hooks/useTauriCommands'

function clamp(value: number, min: number, max: number): number {
  if (Number.isNaN(value)) return min
  return Math.min(max, Math.max(min, value))
}

function NumericField({
  label,
  value,
  min,
  max,
  step,
  hint,
  onChange,
}: {
  label: string
  value: number
  min: number
  max: number
  step?: number
  hint: string
  onChange: (val: number) => void
}) {
  return (
    <div className="space-y-1.5">
      <Label className="text-[11px] text-holo-text/60">{label}</Label>
      <Input
        type="number"
        min={min}
        max={max}
        step={step ?? 1}
        value={value}
        onChange={(e) => {
          const num = Number(e.target.value)
          onChange(e.target.value === '' || Number.isNaN(num) ? min : clamp(num, min, max))
        }}
        className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright"
      />
      <p className="text-[10px] text-holo-text/30">{hint}</p>
    </div>
  )
}

export function RiskParams() {
  const { config, setConfig } = useSettingsStore()
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [justSaved, setJustSaved] = useState(false)

  useEffect(() => {
    if (tauriAvailable()) {
      getAppConfig().then((loaded) => {
        setConfig(loaded)
      }).catch(() => {})
    }
  }, [setConfig])

  const handleSave = async () => {
    setError(null)
    setSaving(true)
    try {
      if (tauriAvailable()) {
        await saveAppConfig(config)
      }
      setJustSaved(true)
      setTimeout(() => setJustSaved(false), 2000)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="space-y-6">
      {/* Trading Mode */}
      <div className="space-y-3">
        <h4 className="text-sm font-semibold text-holo-text-bright flex items-center gap-2">
          <Shield size={14} className="text-holo-yellow" />
          Trading Mode
        </h4>
        <div className="flex items-center justify-between p-3 rounded-lg bg-holo-dark/50 border border-holo-border">
          <div>
            <p className="text-xs text-holo-text-bright">
              {config.paper_mode ? 'Paper Trading' : 'Live Trading'}
            </p>
            <p className="text-[11px] text-holo-text/50">
              {config.paper_mode
                ? 'Simulated trades — no real funds used'
                : 'Real orders will be executed on-chain'}
            </p>
          </div>
          <Switch
            checked={!config.paper_mode}
            onCheckedChange={(checked) => setConfig({ paper_mode: !checked })}
          />
        </div>
        {!config.paper_mode && (
          <div className="flex items-start gap-2 p-3 rounded-lg bg-holo-red/5 border border-holo-red/20">
            <AlertTriangle size={14} className="text-holo-red shrink-0 mt-0.5" />
            <p className="text-[11px] text-holo-red/80">
              Live mode enabled — real funds will be used. Configure risk parameters carefully.
            </p>
          </div>
        )}
      </div>

      {/* Risk Parameters */}
      <div className="space-y-4">
        <h4 className="text-sm font-semibold text-holo-text-bright flex items-center gap-2">
          <Zap size={14} className="text-holo-cyan" />
          Risk Parameters
        </h4>

        <div className="grid grid-cols-3 gap-4">
          <NumericField
            label="Max Drawdown %"
            value={config.max_drawdown_pct}
            min={1} max={100}
            hint="Kill-switch trigger"
            onChange={(v) => setConfig({ max_drawdown_pct: v })}
          />
          <NumericField
            label="Position Size %"
            value={config.position_size_pct}
            min={1} max={100}
            hint="% of equity per trade"
            onChange={(v) => setConfig({ position_size_pct: v })}
          />
          <NumericField
            label="Stop-Loss % (fallback)"
            value={config.stop_loss_pct}
            min={0.1} max={50} step={0.1}
            hint="Used when ATR unavailable"
            onChange={(v) => setConfig({ stop_loss_pct: v })}
          />
        </div>
      </div>

      {/* ATR Dynamic Stops */}
      <div className="space-y-4">
        <h4 className="text-sm font-semibold text-holo-text-bright flex items-center gap-2">
          <Brain size={14} className="text-holo-purple" />
          Dynamic Stops (ATR-Based)
        </h4>

        <div className="grid grid-cols-3 gap-4">
          <NumericField
            label="Stop-Loss ATR Multiplier"
            value={config.stop_loss_atr_mult ?? 2.0}
            min={0.5} max={10} step={0.1}
            hint="SL = ATR x this value"
            onChange={(v) => setConfig({ stop_loss_atr_mult: v })}
          />
          <NumericField
            label="Take-Profit ATR Multiplier"
            value={config.take_profit_atr_mult ?? 3.0}
            min={0.5} max={20} step={0.1}
            hint="TP = ATR x this value"
            onChange={(v) => setConfig({ take_profit_atr_mult: v })}
          />
          <NumericField
            label="Max Hold (hours)"
            value={config.max_hold_hours ?? 24}
            min={1} max={720}
            hint="Time limit per position"
            onChange={(v) => setConfig({ max_hold_hours: v })}
          />
        </div>
      </div>

      {/* Circuit Breakers */}
      <div className="space-y-4">
        <h4 className="text-sm font-semibold text-holo-text-bright flex items-center gap-2">
          <Timer size={14} className="text-holo-orange" />
          Circuit Breakers
        </h4>

        <div className="grid grid-cols-2 gap-4">
          <NumericField
            label="Max Concurrent Positions"
            value={config.max_concurrent_positions ?? 5}
            min={1} max={20}
            hint="Hard cap on open trades"
            onChange={(v) => setConfig({ max_concurrent_positions: v })}
          />
          <NumericField
            label="Cooldown (minutes)"
            value={config.cooldown_minutes ?? 30}
            min={1} max={1440}
            hint="Pause after circuit breaker"
            onChange={(v) => setConfig({ cooldown_minutes: v })}
          />
          <NumericField
            label="Max Stops Before Pause"
            value={config.max_stops_before_pause ?? 3}
            min={1} max={20}
            hint="Stop-losses to trigger halt"
            onChange={(v) => setConfig({ max_stops_before_pause: v })}
          />
          <NumericField
            label="Stops Window (minutes)"
            value={config.stops_window_minutes ?? 60}
            min={5} max={1440}
            hint="Lookback for stop count"
            onChange={(v) => setConfig({ stops_window_minutes: v })}
          />
        </div>
      </div>

      {/* DEX Settings */}
      <div className="space-y-4">
        <h4 className="text-sm font-semibold text-holo-text-bright">DEX & Execution</h4>

        <div className="grid grid-cols-3 gap-4">
          <NumericField
            label="Max Slippage %"
            value={config.max_slippage_pct ?? 1.0}
            min={0.1} max={5} step={0.1}
            hint="Reject swaps above this"
            onChange={(v) => setConfig({ max_slippage_pct: v })}
          />
          <NumericField
            label="Min Liquidity ($)"
            value={config.min_liquidity_usd ?? 10000}
            min={100} max={10000000} step={100}
            hint="Skip illiquid pools"
            onChange={(v) => setConfig({ min_liquidity_usd: v })}
          />
          <NumericField
            label="Paper Balance ($)"
            value={config.paper_balance ?? 10000}
            min={100} max={10000000} step={100}
            hint="Starting simulation funds"
            onChange={(v) => setConfig({ paper_balance: v })}
          />
        </div>
      </div>

      {error && (
        <div className="text-xs text-holo-red bg-holo-red/5 border border-holo-red/20 rounded-lg px-3 py-2">
          {error}
        </div>
      )}

      <Button
        size="sm"
        onClick={handleSave}
        disabled={saving}
        className="bg-holo-cyan/20 hover:bg-holo-cyan/30 text-holo-cyan border border-holo-cyan/30 text-xs"
      >
        {saving ? (
          <Loader2 size={12} className="mr-1.5 animate-spin" />
        ) : (
          <Save size={12} className="mr-1.5" />
        )}
        {justSaved ? 'Saved!' : 'Save Settings'}
      </Button>
    </div>
  )
}

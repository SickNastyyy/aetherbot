import { useState, useEffect } from 'react'
import { Save, AlertTriangle, Shield, Loader2 } from 'lucide-react'
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
    const validated = {
      paper_mode: config.paper_mode,
      max_drawdown_pct: clamp(config.max_drawdown_pct, 1, 100),
      position_size_pct: clamp(config.position_size_pct, 1, 100),
      stop_loss_pct: clamp(config.stop_loss_pct, 0.1, 50),
    }
    setConfig(validated)

    setSaving(true)
    try {
      if (tauriAvailable()) {
        await saveAppConfig(validated)
      }
      setJustSaved(true)
      setTimeout(() => setJustSaved(false), 2000)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleNumberChange = (
    field: 'max_drawdown_pct' | 'position_size_pct' | 'stop_loss_pct',
    value: string,
    min: number,
    max: number,
  ) => {
    const num = Number(value)
    if (value === '' || Number.isNaN(num)) {
      setConfig({ [field]: min })
    } else {
      setConfig({ [field]: clamp(num, min, max) })
    }
  }

  return (
    <div className="space-y-6">
      {/* Paper/Live Toggle */}
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
                : 'Real orders will be executed'}
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
              Live mode is enabled. Real funds will be used for trading. Ensure your risk
              parameters are properly configured before starting the bot.
            </p>
          </div>
        )}
      </div>

      {/* Risk Parameters */}
      <div className="space-y-4">
        <h4 className="text-sm font-semibold text-holo-text-bright">Risk Parameters</h4>

        <div className="grid grid-cols-3 gap-4">
          <div className="space-y-1.5">
            <Label className="text-[11px] text-holo-text/60">Max Drawdown %</Label>
            <Input
              type="number"
              min={1}
              max={100}
              value={config.max_drawdown_pct}
              onChange={(e) => handleNumberChange('max_drawdown_pct', e.target.value, 1, 100)}
              className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright"
            />
            <p className="text-[10px] text-holo-text/30">Kill-switch trigger</p>
          </div>

          <div className="space-y-1.5">
            <Label className="text-[11px] text-holo-text/60">Position Size %</Label>
            <Input
              type="number"
              min={1}
              max={100}
              value={config.position_size_pct}
              onChange={(e) => handleNumberChange('position_size_pct', e.target.value, 1, 100)}
              className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright"
            />
            <p className="text-[10px] text-holo-text/30">% of portfolio per trade</p>
          </div>

          <div className="space-y-1.5">
            <Label className="text-[11px] text-holo-text/60">Stop-Loss %</Label>
            <Input
              type="number"
              min={0.1}
              max={50}
              step={0.1}
              value={config.stop_loss_pct}
              onChange={(e) => handleNumberChange('stop_loss_pct', e.target.value, 0.1, 50)}
              className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright"
            />
            <p className="text-[10px] text-holo-text/30">Per-position stop</p>
          </div>
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

import { useState, useEffect } from 'react'
import { Eye, EyeOff, Save, Trash2, CheckCircle, Info, Loader2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { useSettingsStore } from '@/stores/useSettingsStore'
import {
  saveExchangeCredentials,
  getExchangeCredentials,
  deleteExchangeCredentials,
  tauriAvailable,
} from '@/hooks/useTauriCommands'

export function KalshiConfig() {
  const { credentials, setCredentials, removeCredentials } = useSettingsStore()
  const saved = credentials['Kalshi']

  const [apiKey, setApiKey] = useState('')
  const [secret, setSecret] = useState('')
  const [showKey, setShowKey] = useState(false)
  const [showSecret, setShowSecret] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [justSaved, setJustSaved] = useState(false)

  useEffect(() => {
    if (tauriAvailable()) {
      getExchangeCredentials('Kalshi').then((creds) => {
        if (creds) setCredentials('Kalshi', creds)
      }).catch(() => {})
    }
  }, [setCredentials])

  const handleSave = async () => {
    setError(null)
    const trimmedKey = apiKey.trim()
    const trimmedSecret = secret.trim()

    if (trimmedKey.length === 0 || trimmedSecret.length === 0) {
      setError('API key and secret are required')
      return
    }
    if (trimmedKey.length > 256 || trimmedSecret.length > 256) {
      setError('Key or secret exceeds maximum length (256 chars)')
      return
    }

    setSaving(true)
    try {
      if (tauriAvailable()) {
        await saveExchangeCredentials('Kalshi', trimmedKey, trimmedSecret)
        const masked = await getExchangeCredentials('Kalshi')
        if (masked) setCredentials('Kalshi', masked)
      } else {
        setCredentials('Kalshi', {
          exchange: 'Kalshi',
          api_key_masked: trimmedKey.length > 4
            ? `****${trimmedKey.slice(-4)}`
            : '****',
          has_secret: true,
          has_passphrase: false,
        })
      }
      setApiKey('')
      setSecret('')
      setJustSaved(true)
      setTimeout(() => setJustSaved(false), 2000)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async () => {
    setError(null)
    try {
      if (tauriAvailable()) {
        await deleteExchangeCredentials('Kalshi')
      }
      removeCredentials('Kalshi')
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-holo-text-bright">Kalshi API</h4>
        {saved ? (
          <Badge variant="outline" className="text-[10px] border-holo-green/40 text-holo-green">
            <CheckCircle size={10} className="mr-1" />
            Configured
          </Badge>
        ) : (
          <Badge variant="outline" className="text-[10px] border-holo-text/20 text-holo-text/40">
            Not configured
          </Badge>
        )}
      </div>

      <div className="flex items-start gap-2 p-3 rounded-lg bg-holo-purple/5 border border-holo-purple/20">
        <Info size={14} className="text-holo-purple shrink-0 mt-0.5" />
        <p className="text-[11px] text-holo-text/60 leading-relaxed">
          Kalshi is a CFTC-regulated US prediction market. Generate API credentials from your
          Kalshi account dashboard at kalshi.com. Supports event contracts on economics,
          politics, weather, and more.
        </p>
      </div>

      {error && (
        <div className="text-xs text-holo-red bg-holo-red/5 border border-holo-red/20 rounded-lg px-3 py-2">
          {error}
        </div>
      )}

      <div className="space-y-3">
        <div className="space-y-1.5">
          <Label className="text-[11px] text-holo-text/60">API Key</Label>
          <div className="relative">
            <Input
              type={showKey ? 'text' : 'password'}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="Enter Kalshi API key..."
              maxLength={256}
              className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright pr-8"
            />
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              aria-label={showKey ? 'Hide API key' : 'Show API key'}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-holo-text/30 hover:text-holo-text/60"
            >
              {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
        </div>

        <div className="space-y-1.5">
          <Label className="text-[11px] text-holo-text/60">API Secret</Label>
          <div className="relative">
            <Input
              type={showSecret ? 'text' : 'password'}
              value={secret}
              onChange={(e) => setSecret(e.target.value)}
              placeholder="Enter Kalshi API secret..."
              maxLength={256}
              className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright pr-8"
            />
            <button
              type="button"
              onClick={() => setShowSecret(!showSecret)}
              aria-label={showSecret ? 'Hide secret' : 'Show secret'}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-holo-text/30 hover:text-holo-text/60"
            >
              {showSecret ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
        </div>
      </div>

      {saved && (
        <div className="text-xs text-holo-text/50 font-mono">
          API Key: {saved.api_key_masked}
        </div>
      )}

      <div className="flex gap-2">
        <Button
          size="sm"
          onClick={handleSave}
          disabled={!apiKey.trim() || !secret.trim() || saving}
          className="bg-holo-purple/20 hover:bg-holo-purple/30 text-holo-purple border border-holo-purple/30 text-xs"
        >
          {saving ? (
            <Loader2 size={12} className="mr-1.5 animate-spin" />
          ) : (
            <Save size={12} className="mr-1.5" />
          )}
          {justSaved ? 'Saved!' : 'Save Credentials'}
        </Button>
        {saved && (
          <Button
            size="sm"
            variant="outline"
            onClick={handleDelete}
            className="border-holo-red/30 text-holo-red/70 hover:text-holo-red hover:bg-holo-red/10 text-xs"
          >
            <Trash2 size={12} className="mr-1.5" />
            Remove
          </Button>
        )}
      </div>
    </div>
  )
}

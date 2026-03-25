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

interface Props {
  exchange: 'Jupiter' | 'Uniswap'
  chain: string
  placeholder: string
  description: string
}

export function WalletCredForm({ exchange, chain, placeholder, description }: Props) {
  const { credentials, setCredentials, removeCredentials } = useSettingsStore()
  const saved = credentials[exchange]

  const [privateKey, setPrivateKey] = useState('')
  const [rpcUrl, setRpcUrl] = useState('')
  const [showKey, setShowKey] = useState(false)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [justSaved, setJustSaved] = useState(false)

  useEffect(() => {
    if (tauriAvailable()) {
      getExchangeCredentials(exchange).then((creds) => {
        if (creds) setCredentials(exchange, creds)
      }).catch(() => {})
    }
  }, [exchange, setCredentials])

  const handleSave = async () => {
    setError(null)
    const trimmedKey = privateKey.trim()

    if (trimmedKey.length === 0) {
      setError('Wallet private key is required')
      return
    }
    if (trimmedKey.length > 256) {
      setError('Private key exceeds maximum length (256 chars)')
      return
    }

    setSaving(true)
    try {
      if (tauriAvailable()) {
        await saveExchangeCredentials(
          exchange,
          trimmedKey,
          `${exchange.toLowerCase()}-wallet`,
          rpcUrl.trim() || undefined,
        )
        const masked = await getExchangeCredentials(exchange)
        if (masked) setCredentials(exchange, masked)
      } else {
        setCredentials(exchange, {
          exchange,
          api_key_masked: trimmedKey.length > 4
            ? `****${trimmedKey.slice(-4)}`
            : '****',
          has_secret: false,
          has_passphrase: !!rpcUrl.trim(),
        })
      }
      setPrivateKey('')
      setRpcUrl('')
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
        await deleteExchangeCredentials(exchange)
      }
      removeCredentials(exchange)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-holo-text-bright">{exchange}</h4>
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

      <div className="flex items-start gap-2 p-3 rounded-lg bg-holo-cyan/5 border border-holo-cyan/20">
        <Info size={14} className="text-holo-cyan shrink-0 mt-0.5" />
        <p className="text-[11px] text-holo-text/60 leading-relaxed">
          {description}
        </p>
      </div>

      {saved && (
        <div className="text-xs text-holo-text/50 font-mono">
          Wallet: {saved.api_key_masked}
        </div>
      )}

      {error && (
        <div className="text-xs text-holo-red bg-holo-red/5 border border-holo-red/20 rounded-lg px-3 py-2">
          {error}
        </div>
      )}

      <div className="space-y-3">
        <div className="space-y-1.5">
          <Label className="text-[11px] text-holo-text/60">{chain} Private Key</Label>
          <div className="relative">
            <Input
              type={showKey ? 'text' : 'password'}
              value={privateKey}
              onChange={(e) => setPrivateKey(e.target.value)}
              placeholder={placeholder}
              maxLength={256}
              className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright pr-8"
            />
            <button
              type="button"
              onClick={() => setShowKey(!showKey)}
              aria-label={showKey ? 'Hide private key' : 'Show private key'}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-holo-text/30 hover:text-holo-text/60"
            >
              {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
        </div>

        <div className="space-y-1.5">
          <Label className="text-[11px] text-holo-text/60">RPC Endpoint (optional)</Label>
          <Input
            type="text"
            value={rpcUrl}
            onChange={(e) => setRpcUrl(e.target.value)}
            placeholder="https://..."
            maxLength={256}
            className="bg-holo-dark/50 border-holo-border text-xs font-mono text-holo-text-bright"
          />
        </div>
      </div>

      <div className="flex gap-2">
        <Button
          size="sm"
          onClick={handleSave}
          disabled={!privateKey.trim() || saving}
          className="bg-holo-cyan/20 hover:bg-holo-cyan/30 text-holo-cyan border border-holo-cyan/30 text-xs"
        >
          {saving ? (
            <Loader2 size={12} className="mr-1.5 animate-spin" />
          ) : (
            <Save size={12} className="mr-1.5" />
          )}
          {justSaved ? 'Saved!' : 'Save Wallet'}
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

import type { AppConfig, MaskedCredentials } from '@/lib/types'

// Detect if running inside Tauri (desktop) or browser (dev mode)
function isTauri(): boolean {
  return '__TAURI_INTERNALS__' in window
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    throw new Error(`Tauri not available — cannot invoke '${cmd}' in browser mode`)
  }
  const { invoke: tauriInvoke } = await import('@tauri-apps/api/core')
  return tauriInvoke<T>(cmd, args)
}

export async function saveExchangeCredentials(
  exchange: string,
  apiKey: string,
  secret: string,
  passphrase?: string,
): Promise<void> {
  await invoke('save_exchange_credentials', {
    exchange,
    apiKey: apiKey.trim(),
    secret: secret.trim(),
    passphrase: passphrase?.trim() || null,
  })
}

export async function getExchangeCredentials(
  exchange: string,
): Promise<MaskedCredentials | null> {
  return invoke<MaskedCredentials | null>('get_exchange_credentials', { exchange })
}

export async function deleteExchangeCredentials(exchange: string): Promise<void> {
  await invoke('delete_exchange_credentials', { exchange })
}

export async function saveAppConfig(config: AppConfig): Promise<void> {
  await invoke('save_app_config', { configData: config })
}

export async function getAppConfig(): Promise<AppConfig> {
  return invoke<AppConfig>('get_app_config')
}

export function tauriAvailable(): boolean {
  return isTauri()
}

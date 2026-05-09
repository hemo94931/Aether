export interface AmpProxyUpstreamKeyRoute {
  api_keys: string[]
  upstream_api_key: string
}

export interface AmpProxyConfig {
  upstream_url: string
  upstream_api_key: string
  upstream_api_keys: AmpProxyUpstreamKeyRoute[]
  fallback_to_upstream_on_model_miss: boolean
  force_legacy_worker_runtime: boolean
}

export function createDefaultAmpProxyConfig(): AmpProxyConfig {
  return {
    upstream_url: 'https://ampcode.com',
    upstream_api_key: '',
    upstream_api_keys: [],
    fallback_to_upstream_on_model_miss: false,
    force_legacy_worker_runtime: false,
  }
}

function normalizeString(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

function normalizeApiKeys(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value
    .map(item => String(item).trim())
    .filter((item, index, array) => item.length > 0 && array.indexOf(item) === index)
}

export function normalizeAmpProxyConfig(value: unknown): AmpProxyConfig {
  const defaults = createDefaultAmpProxyConfig()
  if (!value || typeof value !== 'object' || Array.isArray(value)) return defaults

  const source = value as Partial<AmpProxyConfig>
  return {
    upstream_url: normalizeString(source.upstream_url) || defaults.upstream_url,
    upstream_api_key: normalizeString(source.upstream_api_key),
    upstream_api_keys: Array.isArray(source.upstream_api_keys)
      ? source.upstream_api_keys
        .filter(route => route && typeof route === 'object' && !Array.isArray(route))
        .map(route => {
          const item = route as Partial<AmpProxyUpstreamKeyRoute>
          return {
            api_keys: normalizeApiKeys(item.api_keys),
            upstream_api_key: normalizeString(item.upstream_api_key),
          }
        })
      : [],
    fallback_to_upstream_on_model_miss: source.fallback_to_upstream_on_model_miss === true,
    force_legacy_worker_runtime: source.force_legacy_worker_runtime === true,
  }
}

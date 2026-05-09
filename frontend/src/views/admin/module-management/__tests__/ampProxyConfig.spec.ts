import { describe, expect, it } from 'vitest'

import { normalizeAmpProxyConfig } from '../ampProxyConfig'

describe('normalizeAmpProxyConfig', () => {
  it('provides defaults for missing config', () => {
    expect(normalizeAmpProxyConfig(null)).toEqual({
      upstream_url: 'https://ampcode.com',
      upstream_api_key: '',
      upstream_api_keys: [],
      fallback_to_upstream_on_model_miss: false,
      force_legacy_worker_runtime: false,
    })
  })

  it('normalizes multi-upstream key routes', () => {
    expect(normalizeAmpProxyConfig({
      upstream_url: 'https://amp.example',
      upstream_api_key: 'default-key',
      fallback_to_upstream_on_model_miss: true,
      force_legacy_worker_runtime: true,
      upstream_api_keys: [
        {
          api_keys: [' client-a ', 'client-a', '', 'client-b'],
          upstream_api_key: 'tenant-key',
        },
      ],
    })).toEqual({
      upstream_url: 'https://amp.example',
      upstream_api_key: 'default-key',
      upstream_api_keys: [
        {
          api_keys: ['client-a', 'client-b'],
          upstream_api_key: 'tenant-key',
        },
      ],
      fallback_to_upstream_on_model_miss: true,
      force_legacy_worker_runtime: true,
    })
  })
})

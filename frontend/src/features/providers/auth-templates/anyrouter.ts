/**
 * Anyrouter 认证模板
 *
 * 适用于 Anyrouter 中转站：
 * - 使用 Cookie 认证（session）
 * - 自动处理 acw_sc__v2 反爬 Cookie（后端处理）
 * - quota 单位是 1/500000 美元
 */

import type { AuthTemplate, AuthTemplateFieldGroup } from './types'
import type { SaveConfigRequest } from '@/api/providerOps'
import { PROXY_FIELD_GROUP, buildProxyUrl, parseProxyUrl } from './types'

export const anyrouterTemplate: AuthTemplate = {
  id: 'anyrouter',
  name: 'Anyrouter',
  description: '适用于 Anyrouter 中转站，使用 Cookie 认证',

  getFields(providerWebsite?: string): AuthTemplateFieldGroup[] {
    return [
      {
        fields: [
          {
            key: 'base_url',
            label: '站点地址',
            type: 'text',
            placeholder: providerWebsite || 'https://anyrouter.top',
            required: !providerWebsite,
            helpText: '通常为 https://anyrouter.top',
          },
          {
            key: 'session_cookie',
            label: 'Cookie',
            type: 'password',
            placeholder: 'session=MTc2ODc4...; acw_sc__v2=...',
            required: true,
            sensitive: true,
            helpText: '从浏览器开发者工具复制完整 Cookie，或仅填写 session 值',
          },
        ],
      },
      PROXY_FIELD_GROUP,
    ]
  },

  buildRequest(formData: Record<string, any>, providerWebsite?: string): SaveConfigRequest {
    const baseUrl = formData.base_url || providerWebsite || ''

    return {
      architecture_id: 'anyrouter',
      base_url: baseUrl,
      connector: {
        auth_type: 'cookie',
        config: {
          proxy: buildProxyUrl(formData),
        },
        credentials: {
          session_cookie: formData.session_cookie,
        },
      },
      actions: {},
      schedule: {},
    }
  },

  parseConfig(config: any): Record<string, any> {
    const proxyData = parseProxyUrl(config?.connector?.config?.proxy)
    return {
      base_url: config?.base_url || '',
      session_cookie: config?.connector?.credentials?.session_cookie || '',
      ...proxyData,
    }
  },

  validate(formData: Record<string, any>): string | null {
    if (!formData.session_cookie?.trim()) {
      return '请填写 Session Cookie'
    }
    return null
  },

  formatQuota(quota: number): string {
    // Anyrouter 的 quota 单位是 1/500000 美元
    const usd = quota / 500000
    if (usd >= 1) {
      return `$${usd.toFixed(2)}`
    }
    return `$${usd.toFixed(4)}`
  },
}

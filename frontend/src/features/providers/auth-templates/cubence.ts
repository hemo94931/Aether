/**
 * Cubence 认证模板
 *
 * 适用于 Cubence 中转站：
 * - 使用 Cookie 认证（token JWT）
 * - 余额单位直接是美元
 * - 支持窗口限额查询（5小时/每周）
 */

import type { AuthTemplate, AuthTemplateFieldGroup, BalanceExtraItem } from './types'
import type { SaveConfigRequest } from '@/api/providerOps'
import { PROXY_FIELD_GROUP, buildProxyUrl, parseProxyUrl } from './types'

/**
 * 格式化窗口限额显示（百分比格式）
 * 单位是美元（原始值除以 1000000）
 */
function formatWindowLimit(limit: {
  limit?: number
  used?: number
  remaining?: number
  resets_at?: number
}): { text: string; percent: number } {
  if (!limit || limit.remaining === undefined || limit.limit === undefined || limit.limit === 0) {
    return { text: '-', percent: 0 }
  }

  // 计算剩余百分比
  const percent = Math.round((limit.remaining / limit.limit) * 100)

  return { text: `${percent}%`, percent }
}

/**
 * 格式化窗口限额详细信息（用于 tooltip）
 */
function formatWindowLimitDetail(limit: {
  limit?: number
  used?: number
  remaining?: number
  resets_at?: number
}): string {
  if (!limit || limit.remaining === undefined || limit.limit === undefined) {
    return ''
  }

  // 转换为美元（原始值除以 1000000）
  const remaining = (limit.remaining / 1000000).toFixed(2)
  const total = (limit.limit / 1000000).toFixed(2)

  return `$${remaining} / $${total}`
}

export const cubenceTemplate: AuthTemplate = {
  id: 'cubence',
  name: 'Cubence',
  description: '适用于 Cubence 中转站，使用 Cookie 认证',

  getFields(providerWebsite?: string): AuthTemplateFieldGroup[] {
    return [
      {
        fields: [
          {
            key: 'base_url',
            label: '站点地址',
            type: 'text',
            placeholder: providerWebsite || 'https://cubence.com',
            required: !providerWebsite,
            helpText: '通常为 https://cubence.com',
          },
          {
            key: 'token_cookie',
            label: 'Token Cookie',
            type: 'password',
            placeholder: 'token=eyJhbGciOiJI...',
            required: true,
            sensitive: true,
            helpText: '从浏览器开发者工具复制 Cookie 中的 token 值（JWT 格式）',
          },
        ],
      },
      PROXY_FIELD_GROUP,
    ]
  },

  buildRequest(formData: Record<string, any>, providerWebsite?: string): SaveConfigRequest {
    const baseUrl = formData.base_url || providerWebsite || ''

    return {
      architecture_id: 'cubence',
      base_url: baseUrl,
      connector: {
        auth_type: 'cookie',
        config: {
          proxy: buildProxyUrl(formData),
        },
        credentials: {
          token_cookie: formData.token_cookie,
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
      token_cookie: config?.connector?.credentials?.token_cookie || '',
      ...proxyData,
    }
  },

  validate(formData: Record<string, any>): string | null {
    if (!formData.token_cookie?.trim()) {
      return '请填写 Token Cookie'
    }
    return null
  },

  formatQuota(quota: number): string {
    // Cubence 的余额单位直接是美元
    if (quota >= 1) {
      return `$${quota.toFixed(2)}`
    }
    return `$${quota.toFixed(4)}`
  },

  formatBalanceExtra(extra: Record<string, any>): BalanceExtraItem[] {
    const items: BalanceExtraItem[] = []

    // 5小时窗口限额
    if (extra.five_hour_limit) {
      const limit = extra.five_hour_limit
      const detail = formatWindowLimitDetail(limit)
      const { text, percent } = formatWindowLimit(limit)
      items.push({
        label: '5h',
        value: text,
        percent,
        resetsAt: limit.resets_at,
        tooltip: detail || undefined,
      })
    }

    // 每周窗口限额
    if (extra.weekly_limit) {
      const limit = extra.weekly_limit
      const detail = formatWindowLimitDetail(limit)
      const { text, percent } = formatWindowLimit(limit)
      items.push({
        label: '周',
        value: text,
        percent,
        resetsAt: limit.resets_at,
        tooltip: detail || undefined,
      })
    }

    return items
  },
}

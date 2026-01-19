/**
 * YesCode 认证模板
 *
 * 适用于 YesCode 中转站：
 * - 使用 Cookie 认证（yescode_auth JWT + yescode_csrf）
 * - 余额单位直接是美元
 * - subscription_balance: 套餐每日额度
 * - weekly_limit: 周限额（用完后套餐额度也无法使用）
 */

import type { AuthTemplate, AuthTemplateFieldGroup, BalanceExtraItem } from './types'
import type { SaveConfigRequest } from '@/api/providerOps'
import { PROXY_FIELD_GROUP, buildProxyUrl, parseProxyUrl } from './types'

/**
 * 格式化限额显示（百分比格式）
 */
function formatLimitPercent(remaining: number, limit: number): { text: string; percent: number } {
  if (!limit || limit === 0) {
    return { text: '-', percent: 0 }
  }

  const percent = Math.round((remaining / limit) * 100)
  return { text: `${percent}%`, percent }
}

export const yescodeTemplate: AuthTemplate = {
  id: 'yescode',
  name: 'YesCode',
  description: '适用于 YesCode 中转站，使用 Cookie 认证',

  getFields(providerWebsite?: string): AuthTemplateFieldGroup[] {
    return [
      {
        fields: [
          {
            key: 'base_url',
            label: '站点地址',
            type: 'text',
            placeholder: providerWebsite || 'https://co.yes.vg',
            required: !providerWebsite,
            helpText: '通常为 https://co.yes.vg',
          },
          {
            key: 'auth_cookie',
            label: 'Auth Cookie',
            type: 'password',
            placeholder: 'yescode_auth=eyJhbGciOiJI...; yescode_csrf=...',
            required: true,
            sensitive: true,
            helpText: '从浏览器开发者工具复制 Cookie（包含 yescode_auth 和 yescode_csrf）',
          },
        ],
      },
      PROXY_FIELD_GROUP,
    ]
  },

  buildRequest(formData: Record<string, any>, providerWebsite?: string): SaveConfigRequest {
    const baseUrl = formData.base_url || providerWebsite || ''

    return {
      architecture_id: 'yescode',
      base_url: baseUrl,
      connector: {
        auth_type: 'cookie',
        config: {
          proxy: buildProxyUrl(formData),
        },
        credentials: {
          auth_cookie: formData.auth_cookie,
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
      auth_cookie: config?.connector?.credentials?.auth_cookie || '',
      ...proxyData,
    }
  },

  validate(formData: Record<string, any>): string | null {
    if (!formData.auth_cookie?.trim()) {
      return '请填写 Auth Cookie'
    }
    return null
  },

  formatQuota(quota: number): string {
    // YesCode 的余额单位直接是美元
    if (quota >= 1) {
      return `$${quota.toFixed(2)}`
    }
    return `$${quota.toFixed(4)}`
  },

  formatBalanceExtra(extra: Record<string, any>): BalanceExtraItem[] {
    const items: BalanceExtraItem[] = []

    const weeklyLimit = extra.weekly_limit
    const weeklySpent = extra.weekly_spent
    const dailyLimit = extra.daily_limit
    const dailySpent = extra.daily_spent
    const dailyResetsAt = extra.daily_resets_at
    const weeklyResetsAt = extra.weekly_resets_at

    // 天限
    if (dailyLimit !== undefined && dailyLimit > 0 && dailySpent !== undefined) {
      const dailyRemaining = Math.max(0, dailyLimit - dailySpent)
      const { text, percent } = formatLimitPercent(dailyRemaining, dailyLimit)
      items.push({
        label: '天',
        value: text,
        percent,
        resetsAt: dailyResetsAt,
        tooltip: `$${dailyRemaining.toFixed(2)} / $${(dailyLimit as number).toFixed(2)}`,
      })
    }

    // 周限
    if (weeklyLimit !== undefined && weeklyLimit > 0 && weeklySpent !== undefined) {
      const weeklyRemaining = Math.max(0, weeklyLimit - weeklySpent)
      const { text, percent } = formatLimitPercent(weeklyRemaining, weeklyLimit)
      items.push({
        label: '周',
        value: text,
        percent,
        resetsAt: weeklyResetsAt,
        tooltip: `$${weeklyRemaining.toFixed(2)} / $${(weeklyLimit as number).toFixed(2)}`,
      })
    }

    return items
  },
}

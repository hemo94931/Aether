/**
 * NekoCode 认证模板
 *
 * 适用于 NekoCode 中转站：
 * - 使用 Cookie 认证（session）
 * - 显示余额、每日配额、订阅状态
 */

import type { AuthTemplate, AuthTemplateFieldGroup, BalanceExtraItem } from './types'
import type { SaveConfigRequest } from '@/api/providerOps'
import { PROXY_FIELD_GROUP, buildProxyConfig, parseProxyConfig } from './types'

export const nekocodeTemplate: AuthTemplate = {
  id: 'nekocode',
  name: 'NekoCode',
  description: '适用于 NekoCode 中转站，使用 Cookie 认证',

  getFields(providerWebsite?: string): AuthTemplateFieldGroup[] {
    return [
      {
        fields: [
          {
            key: 'base_url',
            label: '站点地址',
            type: 'text',
            placeholder: providerWebsite || 'https://nekocode.ai',
            required: !providerWebsite,
            helpText: '通常为 https://nekocode.ai',
          },
          {
            key: 'session_cookie',
            label: 'Cookie',
            type: 'password',
            placeholder: 'session=MTc2OTYx...',
            required: true,
            sensitive: true,
            helpText: '从浏览器开发者工具复制 session Cookie 值',
          },
        ],
      },
      PROXY_FIELD_GROUP,
    ]
  },

  buildRequest(formData: Record<string, any>, providerWebsite?: string): SaveConfigRequest {
    const baseUrl = formData.base_url || providerWebsite || ''

    return {
      architecture_id: 'nekocode',
      base_url: baseUrl,
      connector: {
        auth_type: 'cookie',
        config: {
          ...buildProxyConfig(formData),
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
    const proxyData = parseProxyConfig(config?.connector?.config)
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
    // NekoCode 的余额单位是美元
    if (quota >= 1) {
      return `$${quota.toFixed(2)}`
    }
    return `$${quota.toFixed(4)}`
  },

  formatBalanceExtra(extra: Record<string, any>): BalanceExtraItem[] {
    const items: BalanceExtraItem[] = []

    // 每日配额（天卡）- 显示进度条和倒计时
    if (extra.daily_quota_limit !== undefined && extra.daily_remaining_quota !== undefined) {
      const limit = Number(extra.daily_quota_limit)
      const remaining = Number(extra.daily_remaining_quota)
      const percent = limit > 0 ? Math.round((remaining / limit) * 100) : 0

      // 计算刷新时间戳
      let resetsAt: number | undefined
      if (extra.effective_start_date) {
        try {
          // effective_start_date 是订阅开始时间，每日配额在每天的这个时间刷新
          const startDate = new Date(extra.effective_start_date)
          const now = new Date()
          // 找到下一个刷新时间点（今天或明天的同一时间）
          const todayReset = new Date(now)
          todayReset.setHours(startDate.getHours(), startDate.getMinutes(), startDate.getSeconds(), 0)
          if (todayReset <= now) {
            // 已过今天的刷新时间，设为明天
            todayReset.setDate(todayReset.getDate() + 1)
          }
          resetsAt = Math.floor(todayReset.getTime() / 1000)
        } catch {
          // 忽略解析错误
        }
      }

      items.push({
        label: '天',
        value: `${percent}%`,
        percent,
        resetsAt,
      })
    }

    // 套餐到期时间 - 显示倒计时
    if (extra.effective_end_date) {
      try {
        const endDate = new Date(extra.effective_end_date)
        const now = new Date()
        const daysLeft = Math.ceil((endDate.getTime() - now.getTime()) / (1000 * 60 * 60 * 24))
        const resetsAt = Math.floor(endDate.getTime() / 1000)
        const percent = Math.min(100, Math.max(0, Math.round((daysLeft / 30) * 100)))

        items.push({
          label: '月',
          value: `${percent}%`,
          percent,
          resetsAt,
        })
      } catch {
        // 忽略解析错误
      }
    }

    return items
  },
}

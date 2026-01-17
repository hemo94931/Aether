/**
 * New API 认证模板
 *
 * 适用于 New API 风格的中转站：
 * - 使用 Bearer Token 认证
 * - 需要 New-Api-User Header 传递用户 ID
 * - quota 单位通常是 1/500000 美元
 */

import type { AuthTemplate, AuthTemplateFieldGroup } from './types'
import type { SaveConfigRequest } from '@/api/providerOps'

export const newApiTemplate: AuthTemplate = {
  id: 'new_api',
  name: 'New API',
  description: '适用于 New API 风格的中转站，使用 Bearer Token + New-Api-User Header',

  getFields(providerWebsite?: string): AuthTemplateFieldGroup[] {
    return [
      {
        fields: [
          {
            key: 'base_url',
            label: 'API 地址',
            type: 'text',
            placeholder: providerWebsite || 'https://api.example.com',
            helpText: '提供商的 API 基础地址，留空则使用提供商官网',
            required: false,
          },
          {
            key: 'api_key',
            label: '访问令牌 (API Key)',
            type: 'password',
            placeholder: 'sk-xxx',
            required: true,
            sensitive: true,
          },
          {
            key: 'user_id',
            label: '用户 ID',
            type: 'text',
            placeholder: '用户 ID',
            required: true,
          },
        ],
      },
    ]
  },

  buildRequest(formData: Record<string, any>, providerWebsite?: string): SaveConfigRequest {
    const baseUrl = formData.base_url || providerWebsite || ''

    return {
      architecture_id: 'new_api',
      base_url: baseUrl,
      connector: {
        auth_type: 'api_key',
        config: {
          auth_method: 'bearer',
        },
        credentials: {
          api_key: formData.api_key,
          user_id: formData.user_id,
        },
      },
      actions: {},
      schedule: {},
    }
  },

  parseConfig(config: any): Record<string, any> {
    return {
      base_url: config?.base_url || '',
      api_key: config?.connector?.credentials?.api_key || '',
      user_id: config?.connector?.credentials?.user_id || '',
    }
  },

  validate(formData: Record<string, any>): string | null {
    if (!formData.api_key?.trim()) {
      return '请填写访问令牌'
    }
    if (!formData.user_id?.trim()) {
      return '请填写用户 ID'
    }
    return null
  },

  formatQuota(quota: number): string {
    // New API 的 quota 单位是 1/500000 美元
    const usd = quota / 500000
    if (usd >= 1) {
      return `$${usd.toFixed(2)}`
    }
    return `$${usd.toFixed(4)}`
  },
}

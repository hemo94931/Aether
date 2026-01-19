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
import { PROXY_FIELD_GROUP, buildProxyUrl, parseProxyUrl } from './types'

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
            label: '站点地址',
            type: 'text',
            placeholder: providerWebsite
              ? `${providerWebsite}`
              : '请填写站点地址',
            required: !providerWebsite,
          },
        ],
      },
      {
        fields: [
          {
            key: 'cookie',
            label: 'Cookie',
            type: 'text',
            placeholder: '',
            required: false,
            sensitive: true,
            helpText: '填写 Cookie 后支持自动签到',
          },
        ],
      },
      {
        layout: 'inline',
        fields: [
          {
            key: 'api_key',
            label: '访问令牌 (API Key)',
            type: 'password',
            placeholder: '',
            required: false,
            sensitive: true,
            flex: 3,
          },
          {
            key: 'user_id',
            label: '用户 ID',
            type: 'text',
            placeholder: '',
            required: false,
            flex: 1,
          },
        ],
      },
      PROXY_FIELD_GROUP,
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
          proxy: buildProxyUrl(formData),
        },
        credentials: {
          // 敏感字段始终发送（空字符串会触发后端合并已保存的值）
          api_key: formData.api_key?.trim() || '',
          user_id: formData.user_id?.trim() || '',
          cookie: formData.cookie?.trim() || '',
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
      api_key: config?.connector?.credentials?.api_key || '',
      user_id: config?.connector?.credentials?.user_id || '',
      cookie: config?.connector?.credentials?.cookie || '',
      ...proxyData,
    }
  },

  validate(formData: Record<string, any>): string | null {
    const hasApiKey = !!formData.api_key?.trim()
    const hasCookie = !!formData.cookie?.trim()
    const hasUserId = !!formData.user_id?.trim()

    // api_key 和 cookie 至少需要一个
    if (!hasApiKey && !hasCookie) {
      return '访问令牌和 Cookie 至少需要填写一个'
    }

    // 使用 api_key 时必须提供 user_id，使用 cookie 时 user_id 可选
    if (hasApiKey && !hasCookie && !hasUserId) {
      return '使用访问令牌时，用户 ID 不能为空'
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

  onFieldChange(fieldKey: string, value: any, formData: Record<string, any>): void {
    // 当 cookie 变化且 user_id 为空时，尝试从 cookie 解析 user_id
    if (fieldKey === 'cookie' && value && !formData.user_id?.trim()) {
      const result = parseSessionCookie(value)
      if (result.userId) {
        formData.user_id = result.userId
      }
    }
  },
}

interface SessionCookieResult {
  userId: string | null
  username: string | null
}

/**
 * 从 Cookie 字符串中解析用户 ID 和用户名
 *
 * New API 的 session cookie 格式:
 * base64(timestamp|gob_base64|signature)
 *
 * gob 数据中包含 id 和 username 字段
 *
 * 支持两种输入：
 * 1. 完整 Cookie: "session=xxx; acw_tc=xxx; ..."
 * 2. 仅 session 值: "MTc2ODc4..."
 */
function parseSessionCookie(cookie: string): SessionCookieResult {
  const result: SessionCookieResult = { userId: null, username: null }

  try {
    // 提取 session 值
    let sessionValue = cookie.trim()
    if (sessionValue.includes('session=')) {
      const match = sessionValue.match(/session=([^;]+)/)
      if (match) {
        sessionValue = match[1]
      }
    }

    // URL-safe base64 解码
    // 补充 padding
    const padding = 4 - (sessionValue.length % 4)
    if (padding !== 4) {
      sessionValue += '='.repeat(padding)
    }

    // 将 URL-safe base64 转为标准 base64
    const standardBase64 = sessionValue.replace(/-/g, '+').replace(/_/g, '/')
    const decoded = atob(standardBase64)

    // 分割: timestamp|gob_base64|signature
    const parts = decoded.split('|')
    if (parts.length < 2) {
      return result
    }

    // 解码 gob 数据（第二部分）
    let gobB64 = parts[1]
    const gobPadding = 4 - (gobB64.length % 4)
    if (gobPadding !== 4) {
      gobB64 += '='.repeat(gobPadding)
    }
    const gobStandardB64 = gobB64.replace(/-/g, '+').replace(/_/g, '/')
    const gobData = atob(gobStandardB64)

    // 解析 gob 编码的 id 字段
    // 查找 "\x02id\x03int" 模式，后面跟着 gob 编码的整数
    const idIntPattern = '\x02id\x03int'
    const idIdx = gobData.indexOf(idIntPattern)
    if (idIdx !== -1) {
      // 跳过 "\x02id\x03int" (7字节) 和类型标记 (2字节)
      const valueStart = idIdx + 7 + 2
      if (valueStart < gobData.length) {
        // 读取第一个字节，检查是否是 00（正数标记）
        const firstByte = gobData.charCodeAt(valueStart)
        if (firstByte === 0) {
          // 下一个字节是长度标记
          const marker = gobData.charCodeAt(valueStart + 1)
          if (marker >= 0x80) {
            // 负的表示长度: 256 - marker = 字节数
            const length = 256 - marker
            if (valueStart + 2 + length <= gobData.length) {
              // 读取 length 字节，大端序转整数
              let val = 0
              for (let i = 0; i < length; i++) {
                val = (val << 8) | gobData.charCodeAt(valueStart + 2 + i)
              }
              // gob zigzag 解码：正整数用 2*n 编码
              result.userId = (val >> 1).toString()
            }
          }
        }
      }
    }

    // 解析 gob 编码的 username 字段
    // 查找 "\x08username\x06string" 模式，后面跟着长度和字符串值
    const usernamePattern = '\x08username\x06string'
    const usernameIdx = gobData.indexOf(usernamePattern)
    if (usernameIdx !== -1) {
      // 跳过 pattern (16字节) 和类型标记 (3字节)
      const lengthPos = usernameIdx + usernamePattern.length + 3
      if (lengthPos < gobData.length) {
        const lengthByte = gobData.charCodeAt(lengthPos)
        const valueStart = lengthPos + 1
        // 长度 < 128 表示直接长度编码
        if (lengthByte < 128 && valueStart + lengthByte <= gobData.length) {
          result.username = gobData.substring(valueStart, valueStart + lengthByte)
        }
      }
    }

    return result
  } catch {
    return result
  }
}

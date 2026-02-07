/**
 * 提供商认证模板类型定义
 *
 * 认证模板定义了：
 * - 需要收集的表单字段
 * - 如何构建后端请求
 * - 如何解析已有配置
 */

import type { SaveConfigRequest } from '@/api/providerOps'

/**
 * 表单字段类型
 */
export type FieldType = 'text' | 'password' | 'select' | 'textarea'

/**
 * 表单字段定义
 */
export interface AuthTemplateField {
  /** 字段 key（用于表单数据） */
  key: string
  /** 显示标签 */
  label: string
  /** 字段类型 */
  type: FieldType
  /** 占位符 */
  placeholder?: string
  /** 帮助文本 */
  helpText?: string
  /** 是否必填 */
  required?: boolean
  /** 是否为敏感字段（使用 masked 输入） */
  sensitive?: boolean
  /** select 类型的选项 */
  options?: Array<{ value: string; label: string }>
  /** 默认值 */
  defaultValue?: string
  /** inline 布局时的 flex 比例（默认 1） */
  flex?: number
}

/**
 * 表单字段分组
 */
export interface AuthTemplateFieldGroup {
  /** 分组标题（可选，为空则不显示标题） */
  title?: string
  /** 分组内的字段 */
  fields: AuthTemplateField[]
  /** 是否可折叠（默认展开） */
  collapsible?: boolean
  /** 折叠时的默认状态：true=展开，false=收起 */
  defaultExpanded?: boolean
  /** 是否有启用开关（与 collapsible 配合使用） */
  hasToggle?: boolean
  /** 启用开关对应的表单字段 key */
  toggleKey?: string
  /** 布局方式：'vertical'(默认) 或 'inline'(同行显示) */
  layout?: 'vertical' | 'inline'
}

/**
 * 验证结果数据
 */
export interface VerifyResultData {
  username?: string
  display_name?: string
  email?: string
  quota?: number
  used_quota?: number
  request_count?: number
  extra?: Record<string, any>
}

/**
 * 认证模板接口
 */
export interface AuthTemplate {
  /** 模板 ID（对应后端 architecture_id） */
  id: string
  /** 显示名称 */
  name: string
  /** 描述 */
  description: string

  /**
   * 获取表单字段定义
   * @param providerWebsite 提供商官网（用于设置默认 base_url）
   */
  getFields(providerWebsite?: string): AuthTemplateFieldGroup[]

  /**
   * 构建保存请求
   * @param formData 表单数据
   * @param providerWebsite 提供商官网
   */
  buildRequest(formData: Record<string, any>, providerWebsite?: string): SaveConfigRequest

  /**
   * 从已有配置解析表单数据
   * @param config 已有配置
   */
  parseConfig(config: any): Record<string, any>

  /**
   * 验证表单数据
   * @param formData 表单数据
   * @returns 错误消息，无错误返回 null
   */
  validate(formData: Record<string, any>): string | null

  /**
   * 格式化验证结果中的 quota 显示
   * @param quota quota 值
   */
  formatQuota?(quota: number): string

  /**
   * 格式化余额 extra 信息（如窗口限额等）
   * 返回一个数组，每个元素包含 label 和 value
   * @param extra 余额 extra 字段
   */
  formatBalanceExtra?(extra: Record<string, any>): BalanceExtraItem[]

  /**
   * 字段值变化时的回调，可用于联动填充其他字段
   * @param fieldKey 变化的字段 key
   * @param value 新值
   * @param formData 当前表单数据（可修改）
   */
  onFieldChange?(fieldKey: string, value: any, formData: Record<string, any>): void
}

/**
 * 余额附加信息项
 */
export interface BalanceExtraItem {
  /** 显示标签 */
  label: string
  /** 显示值 */
  value: string
  /** 百分比数值 (0-100)，用于进度条显示 */
  percent?: number
  /** 重置时间戳（Unix 秒），用于倒计时显示 */
  resetsAt?: number
  /** 可选的提示文本 */
  tooltip?: string
}

/**
 * 认证模板注册表类型
 */
export interface AuthTemplateRegistry {
  /** 获取所有模板 */
  getAll(): AuthTemplate[]
  /** 根据 ID 获取模板 */
  get(id: string): AuthTemplate | undefined
  /** 获取默认模板 */
  getDefault(): AuthTemplate
  /** 注册模板 */
  register(template: AuthTemplate): void
}

// ==================== 通用字段定义 ====================

/**
 * 代理节点 ID 字段
 */
export const PROXY_NODE_FIELD: AuthTemplateField = {
  key: 'proxy_node_id',
  label: '代理节点',
  type: 'select',
  placeholder: '选择代理节点...',
  required: false,
}

/**
 * 通用代理配置字段组（可折叠，带启用开关）
 * 由 ProviderAuthDialog 特殊渲染为代理节点选择器
 */
export const PROXY_FIELD_GROUP: AuthTemplateFieldGroup = {
  title: '代理配置',
  fields: [PROXY_NODE_FIELD],
  collapsible: true,
  defaultExpanded: false,
  hasToggle: true,
  toggleKey: 'proxy_enabled',
}

// 兼容旧代码的字段导出
export const PROXY_URL_FIELD: AuthTemplateField = {
  key: 'proxy_url',
  label: '代理地址',
  type: 'text',
  placeholder: 'http://proxy:port 或 socks5://',
  required: false,
}
export const PROXY_USERNAME_FIELD: AuthTemplateField = {
  key: 'proxy_username',
  label: '用户名',
  type: 'text',
  placeholder: '可选',
  required: false,
}
export const PROXY_PASSWORD_FIELD: AuthTemplateField = {
  key: 'proxy_password',
  label: '密码',
  type: 'password',
  placeholder: '可选',
  required: false,
  sensitive: true,
}

/**
 * 构建代理配置（仅代理节点模式）
 *
 * @param formData 表单数据
 * @returns 代理配置对象，展开到 connector.config 中
 */
export function buildProxyConfig(formData: Record<string, any>): { proxy_node_id?: string } {
  if (!formData.proxy_enabled || !formData.proxy_node_id) {
    return {}
  }
  return { proxy_node_id: formData.proxy_node_id }
}

/**
 * 解析代理配置
 *
 * @param config connector.config 对象
 * @returns 表单数据
 */
export function parseProxyConfig(config: any): Record<string, any> {
  // 代理节点模式
  if (config?.proxy_node_id) {
    return {
      proxy_enabled: true,
      proxy_node_id: config.proxy_node_id,
    }
  }

  // 兼容旧数据（手动 URL 模式）- 标记为启用但无节点
  if (config?.proxy) {
    return {
      proxy_enabled: true,
      proxy_node_id: '',
    }
  }

  return {
    proxy_enabled: false,
    proxy_node_id: '',
  }
}

// 兼容旧的导出
export const PROXY_FIELD: AuthTemplateField = PROXY_URL_FIELD

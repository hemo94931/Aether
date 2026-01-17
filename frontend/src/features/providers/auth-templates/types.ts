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
}

/**
 * 表单字段分组
 */
export interface AuthTemplateFieldGroup {
  /** 分组标题（可选，为空则不显示标题） */
  title?: string
  /** 分组内的字段 */
  fields: AuthTemplateField[]
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

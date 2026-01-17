/**
 * 提供商认证模板注册表
 *
 * 集中管理所有认证模板。
 *
 * ## 添加新模板的步骤
 *
 * 1. 在 `auth-templates/` 目录下创建新的模板文件（如 `my-api.ts`）
 * 2. 实现 `AuthTemplate` 接口
 * 3. 在本文件中导入并注册到 `templates` 数组
 *
 * ## 模板需要实现的内容
 *
 * - `id`: 模板唯一标识（对应后端的 architecture_id）
 * - `name`: 显示名称
 * - `description`: 描述文本
 * - `getFields()`: 返回表单字段定义
 * - `buildRequest()`: 构建后端 API 请求
 * - `parseConfig()`: 从已有配置解析表单数据
 * - `validate()`: 验证表单数据
 * - `formatQuota()`: （可选）格式化 quota 显示
 */

import type { AuthTemplate, AuthTemplateRegistry } from './types'
import { newApiTemplate } from './new-api'

// ==================== 模板注册 ====================
// 在这里添加新模板

const templates: AuthTemplate[] = [newApiTemplate]

// ==================== 注册表实现 ====================

const templateMap = new Map<string, AuthTemplate>()

// 初始化 Map
templates.forEach((template) => {
  templateMap.set(template.id, template)
})

/**
 * 认证模板注册表
 */
export const authTemplateRegistry: AuthTemplateRegistry = {
  getAll(): AuthTemplate[] {
    return templates
  },

  get(id: string): AuthTemplate | undefined {
    return templateMap.get(id)
  },

  getDefault(): AuthTemplate {
    return templates[0]
  },

  register(template: AuthTemplate): void {
    templates.push(template)
    templateMap.set(template.id, template)
  },
}

// ==================== 导出 ====================

export * from './types'
export { newApiTemplate } from './new-api'

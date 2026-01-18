import apiClient from './client'

export interface ModuleStatus {
  name: string
  available: boolean
  enabled: boolean
  active: boolean
  config_validated: boolean
  config_error: string | null
  display_name: string
  description: string
  category: 'auth' | 'monitoring' | 'security' | 'integration'
  admin_route: string | null
  admin_menu_icon: string | null
  admin_menu_group: string | null
  admin_menu_order: number
  health: 'healthy' | 'degraded' | 'unhealthy' | 'unknown'
}

export interface AuthModuleInfo {
  name: string
  display_name: string
  active: boolean
}

export const modulesApi = {
  /**
   * 获取所有模块状态（管理员）
   */
  async getAllStatus(): Promise<Record<string, ModuleStatus>> {
    const response = await apiClient.get<Record<string, ModuleStatus>>(
      '/api/admin/modules/status'
    )
    return response.data
  },

  /**
   * 获取单个模块状态（管理员）
   */
  async getStatus(moduleName: string): Promise<ModuleStatus> {
    const response = await apiClient.get<ModuleStatus>(
      `/api/admin/modules/status/${moduleName}`
    )
    return response.data
  },

  /**
   * 设置模块启用状态（管理员）
   */
  async setEnabled(moduleName: string, enabled: boolean): Promise<ModuleStatus> {
    const response = await apiClient.put<ModuleStatus>(
      `/api/admin/modules/status/${moduleName}/enabled`,
      { enabled }
    )
    return response.data
  },

  /**
   * 获取认证模块状态（公开接口，供登录页使用）
   */
  async getAuthModulesStatus(): Promise<AuthModuleInfo[]> {
    const response = await apiClient.get<AuthModuleInfo[]>('/api/modules/auth-status')
    return response.data
  },
}

import apiClient from './client'

export interface ProxyNode {
  id: string
  name: string
  ip: string
  port: number
  region: string | null
  status: 'online' | 'unhealthy' | 'offline'
  is_manual: boolean
  // 手动节点专用字段
  proxy_url?: string
  proxy_username?: string
  proxy_password?: string  // 脱敏后的密码
  registered_by: string | null
  last_heartbeat_at: string | null
  heartbeat_interval: number
  active_connections: number
  total_requests: number
  avg_latency_ms: number | null
  created_at: string
  updated_at: string
}

export interface ProxyNodeListResponse {
  items: ProxyNode[]
  total: number
  skip: number
  limit: number
}

export interface ManualProxyNodeCreateRequest {
  name: string
  proxy_url: string
  username?: string
  password?: string
  region?: string
}

export interface ManualProxyNodeUpdateRequest {
  name?: string
  proxy_url?: string
  username?: string
  password?: string
  region?: string
}

export const proxyNodesApi = {
  async listProxyNodes(params?: { status?: string; skip?: number; limit?: number }): Promise<ProxyNodeListResponse> {
    const response = await apiClient.get<ProxyNodeListResponse>('/api/admin/proxy-nodes', { params })
    return response.data
  },

  async createManualNode(data: ManualProxyNodeCreateRequest): Promise<{ node_id: string; node: ProxyNode }> {
    const response = await apiClient.post<{ node_id: string; node: ProxyNode }>('/api/admin/proxy-nodes/manual', data)
    return response.data
  },

  async updateManualNode(nodeId: string, data: ManualProxyNodeUpdateRequest): Promise<{ node_id: string; node: ProxyNode }> {
    const response = await apiClient.patch<{ node_id: string; node: ProxyNode }>(`/api/admin/proxy-nodes/${nodeId}`, data)
    return response.data
  },

  async deleteProxyNode(nodeId: string): Promise<{ message: string; node_id: string; cleared_system_proxy: boolean }> {
    const response = await apiClient.delete<{ message: string; node_id: string; cleared_system_proxy: boolean }>(`/api/admin/proxy-nodes/${nodeId}`)
    return response.data
  },
}

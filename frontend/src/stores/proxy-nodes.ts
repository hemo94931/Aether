import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { proxyNodesApi, type ProxyNode, type ManualProxyNodeCreateRequest } from '@/api/proxy-nodes'

export const useProxyNodesStore = defineStore('proxy-nodes', () => {
  const nodes = ref<ProxyNode[]>([])
  const total = ref(0)
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** 标记是否已加载过（避免重复请求） */
  const fetched = ref(false)

  /** 在线节点（可用于代理选择） */
  const onlineNodes = computed(() =>
    nodes.value.filter(n => n.status === 'online')
  )

  async function fetchNodes(params?: { status?: string }) {
    loading.value = true
    error.value = null

    try {
      const data = await proxyNodesApi.listProxyNodes({ ...params, limit: 1000 })
      nodes.value = data.items
      total.value = data.total
      fetched.value = true
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || err.response?.data?.detail || '获取代理节点列表失败'
    } finally {
      loading.value = false
    }
  }

  /** 确保节点列表已加载（懒加载，不重复请求） */
  async function ensureLoaded() {
    if (!fetched.value && !loading.value) {
      await fetchNodes()
    }
  }

  async function createManualNode(data: ManualProxyNodeCreateRequest) {
    loading.value = true
    error.value = null

    try {
      const result = await proxyNodesApi.createManualNode(data)
      // 重新获取列表以保持排序一致
      await fetchNodes()
      return result
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || err.response?.data?.detail || '创建手动代理节点失败'
      throw err
    } finally {
      loading.value = false
    }
  }

  async function deleteNode(nodeId: string) {
    loading.value = true
    error.value = null

    try {
      await proxyNodesApi.deleteProxyNode(nodeId)
      nodes.value = nodes.value.filter(n => n.id !== nodeId)
      total.value = Math.max(0, total.value - 1)
    } catch (err: any) {
      error.value = err.response?.data?.error?.message || err.response?.data?.detail || '删除代理节点失败'
      throw err
    } finally {
      loading.value = false
    }
  }

  return {
    nodes,
    total,
    loading,
    error,
    fetched,
    onlineNodes,
    fetchNodes,
    ensureLoaded,
    createManualNode,
    deleteNode,
  }
})

<template>
  <Dialog
    :model-value="internalOpen"
    title="端点管理"
    :description="`管理 ${provider?.name} 的 API 端点`"
    :icon="Settings"
    size="2xl"
    @update:model-value="handleDialogUpdate"
  >
    <div class="space-y-4">
      <!-- 已有端点列表 -->
      <div
        v-if="localEndpoints.length > 0"
        class="space-y-2"
      >
        <Label class="text-muted-foreground">已配置的端点</Label>
        <div class="space-y-2">
          <div
            v-for="endpoint in localEndpoints"
            :key="endpoint.id"
            class="rounded-md border px-3 py-2"
            :class="{ 'opacity-50': !endpoint.is_active }"
          >
            <!-- 编辑模式 -->
            <template v-if="editingEndpointId === endpoint.id">
              <div class="space-y-3">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-medium">{{ API_FORMAT_LABELS[endpoint.api_format] || endpoint.api_format }}</span>
                  <div class="flex items-center gap-1 ml-auto">
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      title="保存"
                      :disabled="savingEndpointId === endpoint.id"
                      @click="saveEndpointUrl(endpoint)"
                    >
                      <Check class="w-3.5 h-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-7 w-7"
                      title="取消"
                      @click="cancelEdit"
                    >
                      <X class="w-3.5 h-3.5" />
                    </Button>
                  </div>
                </div>
                <div class="flex items-end gap-3">
                  <div class="flex-1 min-w-0 space-y-1">
                    <Label class="text-xs text-muted-foreground">Base URL</Label>
                    <Input
                      v-model="editingUrl"
                      class="h-9"
                      :placeholder="provider?.website || 'https://api.example.com'"
                      @keyup.escape="cancelEdit"
                    />
                  </div>
                  <div class="w-52 shrink-0 space-y-1">
                    <Label class="text-xs text-muted-foreground">自定义路径（可选）</Label>
                    <Input
                      v-model="editingPath"
                      class="h-9"
                      :placeholder="editingDefaultPath || '留空使用默认路径'"
                      @keyup.escape="cancelEdit"
                    />
                  </div>
                </div>
                <!-- 请求头规则配置 -->
                <Collapsible
                  v-model:open="rulesExpanded"
                  class="mt-1"
                >
                  <CollapsibleTrigger as-child>
                    <button
                      type="button"
                      class="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
                    >
                      <ChevronRight
                        class="w-3 h-3 transition-transform"
                        :class="{ 'rotate-90': rulesExpanded }"
                      />
                      <span>请求头规则</span>
                      <span
                        v-if="editingRules.length > 0"
                        class="text-primary"
                      >
                        ({{ editingRules.length }})
                      </span>
                    </button>
                  </CollapsibleTrigger>
                  <CollapsibleContent class="pt-2">
                    <div class="space-y-2">
                      <div
                        v-for="(rule, index) in editingRules"
                        :key="index"
                        class="flex items-center gap-2"
                      >
                        <!-- 操作类型选择 -->
                        <Select
                          v-model:open="ruleSelectOpen[index]"
                          :model-value="rule.action"
                          @update:model-value="(v) => updateRuleAction(index, v as 'set' | 'drop' | 'rename')"
                        >
                          <SelectTrigger class="w-24 h-8 text-xs shrink-0">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent
                            position="popper"
                            :side-offset="4"
                          >
                            <SelectItem value="set">
                              覆写
                            </SelectItem>
                            <SelectItem value="drop">
                              删除
                            </SelectItem>
                            <SelectItem value="rename">
                              重命名
                            </SelectItem>
                          </SelectContent>
                        </Select>

                        <!-- set: key = value -->
                        <template v-if="rule.action === 'set'">
                          <Input
                            v-model="rule.key"
                            placeholder="Header 名称"
                            :class="`w-36 h-8 text-xs ${validateRuleKey(rule.key, index) ? 'border-destructive' : ''}`"
                            :title="validateRuleKey(rule.key, index) || ''"
                          />
                          <span class="text-muted-foreground text-xs">=</span>
                          <Input
                            v-model="rule.value"
                            placeholder="Header 值"
                            class="flex-1 h-8 text-xs"
                          />
                        </template>

                        <!-- drop: key -->
                        <template v-else-if="rule.action === 'drop'">
                          <Input
                            v-model="rule.key"
                            placeholder="要删除的 Header 名称"
                            :class="`flex-1 h-8 text-xs ${validateRuleKey(rule.key, index) ? 'border-destructive' : ''}`"
                            :title="validateRuleKey(rule.key, index) || ''"
                          />
                        </template>

                        <!-- rename: from -> to -->
                        <template v-else-if="rule.action === 'rename'">
                          <Input
                            v-model="rule.from"
                            placeholder="原名称"
                            :class="`flex-1 h-8 text-xs ${validateRenameFrom(rule.from, index) ? 'border-destructive' : ''}`"
                            :title="validateRenameFrom(rule.from, index) || ''"
                          />
                          <ArrowRight class="w-4 h-4 shrink-0 text-muted-foreground" />
                          <Input
                            v-model="rule.to"
                            placeholder="新名称"
                            :class="`flex-1 h-8 text-xs ${validateRenameTo(rule.to, index) ? 'border-destructive' : ''}`"
                            :title="validateRenameTo(rule.to, index) || ''"
                          />
                        </template>

                        <Button
                          variant="ghost"
                          size="icon"
                          class="h-8 w-8 shrink-0"
                          @click="removeRule(index)"
                        >
                          <X class="w-3.5 h-3.5" />
                        </Button>
                      </div>
                      <Button
                        variant="outline"
                        size="sm"
                        class="w-full h-8 text-xs"
                        @click="addRule"
                      >
                        <Plus class="w-3 h-3 mr-1" />
                        添加规则
                      </Button>
                    </div>
                  </CollapsibleContent>
                </Collapsible>
              </div>
            </template>
            <template v-else>
              <div class="flex items-center gap-3">
                <div class="w-24 shrink-0">
                  <span class="text-sm font-medium">{{ API_FORMAT_LABELS[endpoint.api_format] || endpoint.api_format }}</span>
                </div>
                <div class="flex-1 min-w-0">
                  <span class="text-sm text-muted-foreground truncate block">
                    {{ endpoint.base_url }}{{ endpoint.custom_path ? endpoint.custom_path : '' }}
                  </span>
                  <span
                    v-if="getEndpointRulesCount(endpoint) > 0"
                    class="text-xs text-muted-foreground/70"
                  >
                    {{ getEndpointRulesCount(endpoint) }} 条请求头规则
                  </span>
                </div>
                <div class="flex items-center gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7"
                    title="编辑"
                    @click="startEdit(endpoint)"
                  >
                    <Edit class="w-3.5 h-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7"
                    :title="endpoint.is_active ? '停用' : '启用'"
                    :disabled="togglingEndpointId === endpoint.id"
                    @click="handleToggleEndpoint(endpoint)"
                  >
                    <Power class="w-3.5 h-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="h-7 w-7 text-destructive hover:text-destructive"
                    title="删除"
                    :disabled="deletingEndpointId === endpoint.id"
                    @click="handleDeleteEndpoint(endpoint)"
                  >
                    <Trash2 class="w-3.5 h-3.5" />
                  </Button>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>

      <!-- 添加新端点 -->
      <div
        v-if="availableFormats.length > 0"
        class="space-y-3 pt-3 border-t"
      >
        <Label class="text-muted-foreground">添加新端点</Label>
        <div class="flex items-end gap-3">
          <div class="w-36 shrink-0 space-y-1.5">
            <Label class="text-xs">API 格式</Label>
            <Select
              v-model="newEndpoint.api_format"
              v-model:open="formatSelectOpen"
            >
              <SelectTrigger class="h-9">
                <SelectValue placeholder="选择格式" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="format in availableFormats"
                  :key="format.value"
                  :value="format.value"
                >
                  {{ format.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="flex-1 min-w-0 space-y-1.5">
            <Label class="text-xs">Base URL</Label>
            <Input
              v-model="newEndpoint.base_url"
              :placeholder="provider?.website || 'https://api.example.com'"
              class="h-9"
            />
          </div>
          <div class="w-52 shrink-0 space-y-1.5">
            <Label class="text-xs">自定义路径（可选）</Label>
            <Input
              v-model="newEndpoint.custom_path"
              :placeholder="newEndpointDefaultPath || '留空使用默认路径'"
              class="h-9"
            />
          </div>
        </div>
      </div>

      <!-- 空状态 -->
      <div
        v-if="localEndpoints.length === 0 && availableFormats.length === 0"
        class="text-center py-8 text-muted-foreground"
      >
        <p>所有 API 格式都已配置</p>
      </div>
    </div>

    <template #footer>
      <Button
        variant="outline"
        @click="handleClose"
      >
        关闭
      </Button>
      <Button
        v-if="availableFormats.length > 0"
        variant="outline"
        :disabled="!newEndpoint.api_format || (!newEndpoint.base_url?.trim() && !provider?.website?.trim()) || addingEndpoint"
        @click="handleAddEndpoint"
      >
        {{ addingEndpoint ? '添加中...' : '添加' }}
      </Button>
    </template>
  </Dialog>

  <!-- 删除端点确认弹窗 -->
  <AlertDialog
    :model-value="deleteConfirmOpen"
    title="删除端点"
    :description="deleteConfirmDescription"
    confirm-text="删除"
    cancel-text="取消"
    type="danger"
    @update:model-value="deleteConfirmOpen = $event"
    @confirm="confirmDeleteEndpoint"
    @cancel="deleteConfirmOpen = false"
  />
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import {
  Dialog,
  Button,
  Input,
  Label,
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
  Collapsible,
  CollapsibleTrigger,
  CollapsibleContent,
} from '@/components/ui'
import { Settings, Edit, Trash2, Check, X, Power, ChevronRight, Plus, ArrowRight } from 'lucide-vue-next'
import { useToast } from '@/composables/useToast'
import { log } from '@/utils/logger'
import AlertDialog from '@/components/common/AlertDialog.vue'
import {
  createEndpoint,
  updateEndpoint,
  deleteEndpoint,
  API_FORMAT_LABELS,
  type ProviderEndpoint,
  type ProviderWithEndpointsSummary,
  type HeaderRule,
} from '@/api/endpoints'
import { adminApi } from '@/api/admin'

// 编辑用的规则类型（统一的可编辑结构）
interface EditableRule {
  action: 'set' | 'drop' | 'rename'
  key: string      // set/drop 用
  value: string    // set 用
  from: string     // rename 用
  to: string       // rename 用
}

const props = defineProps<{
  modelValue: boolean
  provider: ProviderWithEndpointsSummary | null
  endpoints?: ProviderEndpoint[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  'endpointCreated': []
  'endpointUpdated': []
}>()

const { success, error: showError } = useToast()

// 状态
const addingEndpoint = ref(false)
const editingEndpointId = ref<string | null>(null)
const editingUrl = ref('')
const editingPath = ref('')
const savingEndpointId = ref<string | null>(null)
const deletingEndpointId = ref<string | null>(null)
const togglingEndpointId = ref<string | null>(null)
const formatSelectOpen = ref(false)

// 删除确认弹窗状态
const deleteConfirmOpen = ref(false)
const endpointToDelete = ref<ProviderEndpoint | null>(null)

// 请求头规则编辑状态
const editingRules = ref<EditableRule[]>([])
const rulesExpanded = ref(false)
const ruleSelectOpen = ref<Record<number, boolean>>({})  // 每个规则 Select 的打开状态

// 系统保留的 header 名称（不允许用户设置）
const RESERVED_HEADERS = new Set([
  'authorization',
  'x-api-key',
  'x-goog-api-key',
  'content-type',
  'content-length',
  'host',
])

// 内部状态
const internalOpen = computed(() => props.modelValue)

// 新端点表单
const newEndpoint = ref({
  api_format: '',
  base_url: '',
  custom_path: '',
})

// API 格式列表
const apiFormats = ref<Array<{ value: string; label: string; default_path: string }>>([])

// 本地端点列表
const localEndpoints = ref<ProviderEndpoint[]>([])

// 可用的格式（未添加的）
const availableFormats = computed(() => {
  const existingFormats = localEndpoints.value.map(e => e.api_format)
  return apiFormats.value.filter(f => !existingFormats.includes(f.value))
})

// 删除确认弹窗描述
const deleteConfirmDescription = computed(() => {
  if (!endpointToDelete.value) return ''
  const formatLabel = API_FORMAT_LABELS[endpointToDelete.value.api_format] || endpointToDelete.value.api_format
  return `确定要删除 ${formatLabel} 端点吗？关联密钥将移除对该 API 格式的支持。`
})

// 获取指定 API 格式的默认路径
function getDefaultPath(apiFormat: string): string {
  const format = apiFormats.value.find(f => f.value === apiFormat)
  return format?.default_path || ''
}

// 将 API 返回的 header_rules 转换为可编辑的规则数组
function loadRulesFromEndpoint(endpoint: ProviderEndpoint): EditableRule[] {
  const rules: EditableRule[] = []

  if (endpoint.header_rules && endpoint.header_rules.length > 0) {
    for (const rule of endpoint.header_rules) {
      if (rule.action === 'set') {
        rules.push({ action: 'set', key: rule.key, value: rule.value || '', from: '', to: '' })
      } else if (rule.action === 'drop') {
        rules.push({ action: 'drop', key: rule.key, value: '', from: '', to: '' })
      } else if (rule.action === 'rename') {
        rules.push({ action: 'rename', key: '', value: '', from: rule.from, to: rule.to })
      }
    }
  }

  return rules
}

// 将可编辑规则数组转换为 API 需要的 HeaderRule[]
// 返回 null 表示显式清除所有规则，返回数组表示设置规则
function rulesToHeaderRules(rules: EditableRule[]): HeaderRule[] | null {
  const result: HeaderRule[] = []

  for (const rule of rules) {
    if (rule.action === 'set' && rule.key.trim()) {
      result.push({ action: 'set', key: rule.key.trim(), value: rule.value })
    } else if (rule.action === 'drop' && rule.key.trim()) {
      result.push({ action: 'drop', key: rule.key.trim() })
    } else if (rule.action === 'rename' && rule.from.trim() && rule.to.trim()) {
      result.push({ action: 'rename', from: rule.from.trim(), to: rule.to.trim() })
    }
  }

  // 返回 null 而不是 undefined，确保后端能正确清除规则
  return result.length > 0 ? result : null
}

// 添加新规则
function addRule() {
  editingRules.value.push({ action: 'set', key: '', value: '', from: '', to: '' })
}

// 删除规则
function removeRule(index: number) {
  editingRules.value.splice(index, 1)
}

// 更新规则类型时重置字段
function updateRuleAction(index: number, action: 'set' | 'drop' | 'rename') {
  const rule = editingRules.value[index]
  rule.action = action
  // 重置字段
  rule.key = ''
  rule.value = ''
  rule.from = ''
  rule.to = ''
}

// 验证 set/drop 的 key
function validateRuleKey(key: string, index: number): string | null {
  const trimmedKey = key.trim().toLowerCase()
  if (!trimmedKey) return null

  // set/drop 操作都不允许操作保留头
  if (RESERVED_HEADERS.has(trimmedKey)) {
    return `"${key}" 是系统保留的请求头`
  }

  // 检查重复（在所有规则中检查同类型的 key）
  const duplicate = editingRules.value.findIndex(
    (r, i) => i !== index && (
      ((r.action === 'set' || r.action === 'drop') && r.key.trim().toLowerCase() === trimmedKey) ||
      (r.action === 'rename' && r.to.trim().toLowerCase() === trimmedKey)
    )
  )
  if (duplicate >= 0) {
    return '请求头名称重复'
  }

  return null
}

// 验证 rename 的 from
function validateRenameFrom(from: string, index: number): string | null {
  const trimmedFrom = from.trim().toLowerCase()
  if (!trimmedFrom) return null

  // 检查是否有其他规则已经修改了这个头
  const duplicate = editingRules.value.findIndex(
    (r, i) => i !== index &&
      ((r.action === 'set' && r.key.trim().toLowerCase() === trimmedFrom) ||
       (r.action === 'drop' && r.key.trim().toLowerCase() === trimmedFrom) ||
       (r.action === 'rename' && r.from.trim().toLowerCase() === trimmedFrom))
  )
  if (duplicate >= 0) {
    return '该请求头已被其他规则处理'
  }

  return null
}

// 验证 rename 的 to
function validateRenameTo(to: string, index: number): string | null {
  const trimmedTo = to.trim().toLowerCase()
  if (!trimmedTo) return null

  if (RESERVED_HEADERS.has(trimmedTo)) {
    return `"${to}" 是系统保留的请求头`
  }

  // 检查重复
  const duplicate = editingRules.value.findIndex(
    (r, i) => i !== index &&
      ((r.action === 'set' && r.key.trim().toLowerCase() === trimmedTo) ||
       (r.action === 'rename' && r.to.trim().toLowerCase() === trimmedTo))
  )
  if (duplicate >= 0) {
    return '请求头名称重复'
  }

  return null
}

// 检查所有规则是否有效（用于保存前验证）
function hasValidationErrors(): boolean {
  for (let i = 0; i < editingRules.value.length; i++) {
    const rule = editingRules.value[i]
    if (rule.action === 'set' || rule.action === 'drop') {
      if (validateRuleKey(rule.key, i)) return true
    } else if (rule.action === 'rename') {
      if (validateRenameFrom(rule.from, i)) return true
      if (validateRenameTo(rule.to, i)) return true
    }
  }
  return false
}

// 获取端点的请求头规则数量（用于查看模式显示）
function getEndpointRulesCount(endpoint: ProviderEndpoint): number {
  return endpoint.header_rules?.length || 0
}

// 当前编辑端点的默认路径
const editingDefaultPath = computed(() => {
  const endpoint = localEndpoints.value.find(e => e.id === editingEndpointId.value)
  return endpoint ? getDefaultPath(endpoint.api_format) : ''
})

// 新端点选择的格式的默认路径
const newEndpointDefaultPath = computed(() => {
  return getDefaultPath(newEndpoint.value.api_format)
})

// 加载 API 格式列表
const loadApiFormats = async () => {
  try {
    const response = await adminApi.getApiFormats()
    apiFormats.value = response.formats
  } catch (error) {
    log.error('加载API格式失败:', error)
  }
}

onMounted(() => {
  loadApiFormats()
})

// 监听 props 变化
watch(() => props.modelValue, (open) => {
  if (open) {
    localEndpoints.value = [...(props.endpoints || [])]
    // 重置编辑状态
    editingEndpointId.value = null
    editingUrl.value = ''
    editingPath.value = ''
    editingRules.value = []
    rulesExpanded.value = false
  } else {
    // 关闭对话框时完全清空新端点表单
    newEndpoint.value = { api_format: '', base_url: '', custom_path: '' }
  }
}, { immediate: true })

watch(() => props.endpoints, (endpoints) => {
  if (props.modelValue) {
    localEndpoints.value = [...(endpoints || [])]
  }
}, { deep: true })

// 开始编辑
function startEdit(endpoint: ProviderEndpoint) {
  editingEndpointId.value = endpoint.id
  editingUrl.value = endpoint.base_url
  editingPath.value = endpoint.custom_path || ''
  // 加载规则数据
  editingRules.value = loadRulesFromEndpoint(endpoint)
  rulesExpanded.value = editingRules.value.length > 0
}

// 取消编辑
function cancelEdit() {
  editingEndpointId.value = null
  editingUrl.value = ''
  editingPath.value = ''
  editingRules.value = []
  rulesExpanded.value = false
}

// 保存端点
async function saveEndpointUrl(endpoint: ProviderEndpoint) {
  if (!editingUrl.value) return

  // 检查规则是否有验证错误
  if (hasValidationErrors()) {
    showError('请修正请求头规则中的错误')
    return
  }

  savingEndpointId.value = endpoint.id
  try {
    await updateEndpoint(endpoint.id, {
      base_url: editingUrl.value,
      custom_path: editingPath.value || null,
      header_rules: rulesToHeaderRules(editingRules.value),
    })
    success('端点已更新')
    emit('endpointUpdated')
    cancelEdit()
  } catch (error: any) {
    showError(error.response?.data?.detail || '更新失败', '错误')
  } finally {
    savingEndpointId.value = null
  }
}

// 添加端点
async function handleAddEndpoint() {
  if (!props.provider || !newEndpoint.value.api_format) return

  // 如果没有输入 base_url，使用提供商的 website 作为默认值
  const baseUrl = newEndpoint.value.base_url || props.provider.website
  if (!baseUrl) {
    showError('请输入 Base URL')
    return
  }

  addingEndpoint.value = true
  try {
    await createEndpoint(props.provider.id, {
      provider_id: props.provider.id,
      api_format: newEndpoint.value.api_format,
      base_url: baseUrl,
      custom_path: newEndpoint.value.custom_path || undefined,
      is_active: true,
    })
    success(`已添加 ${API_FORMAT_LABELS[newEndpoint.value.api_format] || newEndpoint.value.api_format} 端点`)
    // 重置表单，保留 URL
    newEndpoint.value = { api_format: '', base_url: baseUrl, custom_path: '' }
    emit('endpointCreated')
  } catch (error: any) {
    showError(error.response?.data?.detail || '添加失败', '错误')
  } finally {
    addingEndpoint.value = false
  }
}

// 切换端点启用状态
async function handleToggleEndpoint(endpoint: ProviderEndpoint) {
  togglingEndpointId.value = endpoint.id
  try {
    const newStatus = !endpoint.is_active
    await updateEndpoint(endpoint.id, { is_active: newStatus })
    success(newStatus ? '端点已启用' : '端点已停用')
    emit('endpointUpdated')
  } catch (error: any) {
    showError(error.response?.data?.detail || '操作失败', '错误')
  } finally {
    togglingEndpointId.value = null
  }
}

// 删除端点 - 打开确认弹窗
function handleDeleteEndpoint(endpoint: ProviderEndpoint) {
  endpointToDelete.value = endpoint
  deleteConfirmOpen.value = true
}

// 确认删除端点
async function confirmDeleteEndpoint() {
  if (!endpointToDelete.value) return

  const endpoint = endpointToDelete.value
  deleteConfirmOpen.value = false
  deletingEndpointId.value = endpoint.id

  try {
    await deleteEndpoint(endpoint.id)
    success(`已删除 ${API_FORMAT_LABELS[endpoint.api_format] || endpoint.api_format} 端点`)
    emit('endpointUpdated')
  } catch (error: any) {
    showError(error.response?.data?.detail || '删除失败', '错误')
  } finally {
    deletingEndpointId.value = null
    endpointToDelete.value = null
  }
}

// 关闭对话框
function handleDialogUpdate(value: boolean) {
  emit('update:modelValue', value)
}

function handleClose() {
  emit('update:modelValue', false)
}
</script>

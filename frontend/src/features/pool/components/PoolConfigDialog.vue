<template>
  <Dialog
    :model-value="modelValue"
    title="号池配置"
    description="调整号池调度策略和健康检查参数"
    size="lg"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <form
      class="space-y-5"
      @submit.prevent="handleSave"
    >
      <!-- 调度策略 -->
      <div class="space-y-3">
        <h3 class="text-sm font-medium border-b pb-2">
          调度策略
        </h3>

        <div class="flex items-center justify-between p-3 border rounded-lg bg-muted/50">
          <div class="space-y-0.5">
            <span class="text-sm font-medium">LRU 调度</span>
            <p class="text-xs text-muted-foreground">
              优先选择最久未用的 Key
            </p>
          </div>
          <Switch
            :model-value="form.lru_enabled"
            @update:model-value="(v: boolean) => form.lru_enabled = v"
          />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1.5">
            <Label>
              粘性会话 TTL
              <span class="text-xs text-muted-foreground">(秒)</span>
            </Label>
            <Input
              :model-value="form.sticky_session_ttl_seconds ?? ''"
              type="number"
              min="60"
              max="86400"
              placeholder="3600 (留空禁用)"
              @update:model-value="(v) => form.sticky_session_ttl_seconds = parseNum(v)"
            />
            <p class="text-xs text-muted-foreground">
              同一对话始终路由到同一 Key
            </p>
          </div>
          <div class="space-y-1.5">
            <Label>
              全局优先级
              <span class="text-xs text-muted-foreground">(global_key)</span>
            </Label>
            <Input
              :model-value="form.global_priority ?? ''"
              type="number"
              min="0"
              max="999999"
              placeholder="留空回退 provider_priority"
              @update:model-value="(v) => form.global_priority = parseNum(v)"
            />
            <p class="text-xs text-muted-foreground">
              global_key 模式下号池整体排序值（越小越优先）
            </p>
          </div>
        </div>
      </div>

      <!-- 冷却与健康 -->
      <div class="space-y-3">
        <h3 class="text-sm font-medium border-b pb-2">
          冷却与健康
        </h3>

        <div class="flex items-center justify-between p-3 border rounded-lg bg-muted/50">
          <div class="space-y-0.5">
            <span class="text-sm font-medium">健康策略</span>
            <p class="text-xs text-muted-foreground">
              按上游错误码自动冷却/禁用 Key
            </p>
          </div>
          <Switch
            :model-value="form.health_policy_enabled"
            @update:model-value="(v: boolean) => form.health_policy_enabled = v"
          />
        </div>

        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1.5">
            <Label>
              429 冷却
              <span class="text-xs text-muted-foreground">(秒)</span>
            </Label>
            <Input
              :model-value="form.rate_limit_cooldown_seconds ?? ''"
              type="number"
              min="10"
              max="3600"
              placeholder="300"
              @update:model-value="(v) => form.rate_limit_cooldown_seconds = parseNum(v)"
            />
          </div>
          <div class="space-y-1.5">
            <Label>
              529 冷却
              <span class="text-xs text-muted-foreground">(秒)</span>
            </Label>
            <Input
              :model-value="form.overload_cooldown_seconds ?? ''"
              type="number"
              min="5"
              max="600"
              placeholder="30"
              @update:model-value="(v) => form.overload_cooldown_seconds = parseNum(v)"
            />
          </div>
        </div>
      </div>

      <!-- 成本控制 -->
      <div class="space-y-3">
        <h3 class="text-sm font-medium border-b pb-2">
          成本控制
        </h3>

        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-1.5">
            <Label>
              成本窗口
              <span class="text-xs text-muted-foreground">(秒)</span>
            </Label>
            <Input
              :model-value="form.cost_window_seconds ?? ''"
              type="number"
              min="3600"
              max="86400"
              placeholder="18000 (5 小时)"
              @update:model-value="(v) => form.cost_window_seconds = parseNum(v)"
            />
          </div>
          <div class="space-y-1.5">
            <Label>
              Key 窗口限额
              <span class="text-xs text-muted-foreground">(tokens)</span>
            </Label>
            <Input
              :model-value="form.cost_limit_per_key_tokens ?? ''"
              type="number"
              min="0"
              placeholder="留空 = 不限"
              @update:model-value="(v) => form.cost_limit_per_key_tokens = parseNum(v)"
            />
          </div>
          <div class="space-y-1.5">
            <Label>
              软阈值
              <span class="text-xs text-muted-foreground">(%)</span>
            </Label>
            <Input
              :model-value="form.cost_soft_threshold_percent ?? ''"
              type="number"
              min="0"
              max="100"
              placeholder="80"
              @update:model-value="(v) => form.cost_soft_threshold_percent = parseNum(v)"
            />
          </div>
        </div>
      </div>

      <!-- Claude Code 特有配置 -->
      <template v-if="providerType === 'claude_code'">
        <div class="space-y-3">
          <h3 class="text-sm font-medium border-b pb-2">
            Claude Code
          </h3>

          <div class="space-y-3 p-3 border rounded-lg bg-muted/50">
            <div class="flex items-center justify-between">
              <div class="space-y-0.5">
                <span class="text-sm font-medium">会话数量控制</span>
                <p class="text-xs text-muted-foreground">
                  限制同时活跃的会话数量
                </p>
              </div>
              <Switch
                :model-value="claudeForm.session_control_enabled"
                @update:model-value="(v: boolean) => claudeForm.session_control_enabled = v"
              />
            </div>

            <div
              v-if="claudeForm.session_control_enabled"
              class="grid grid-cols-2 gap-3"
            >
              <div class="space-y-1.5">
                <Label class="text-xs">最大会话数</Label>
                <Input
                  :model-value="claudeForm.max_sessions ?? ''"
                  type="number"
                  min="1"
                  max="1000"
                  placeholder="例如 20"
                  @update:model-value="(v) => claudeForm.max_sessions = parseNum(v)"
                />
              </div>
              <div class="space-y-1.5">
                <Label class="text-xs">会话空闲超时 (分钟)</Label>
                <Input
                  :model-value="claudeForm.session_idle_timeout_minutes ?? ''"
                  type="number"
                  min="1"
                  max="1440"
                  placeholder="默认 5"
                  @update:model-value="(v) => claudeForm.session_idle_timeout_minutes = parseNum(v) ?? 5"
                />
              </div>
            </div>
          </div>

          <div class="flex items-center justify-between p-3 border rounded-lg bg-muted/50">
            <div class="space-y-0.5">
              <span class="text-sm font-medium">TLS 指纹模拟</span>
              <p class="text-xs text-muted-foreground">
                模拟 Node.js / Claude Code 客户端的 TLS 指纹
              </p>
            </div>
            <Switch
              :model-value="claudeForm.enable_tls_fingerprint"
              @update:model-value="(v: boolean) => claudeForm.enable_tls_fingerprint = v"
            />
          </div>

          <div class="flex items-center justify-between p-3 border rounded-lg bg-muted/50">
            <div class="space-y-0.5">
              <span class="text-sm font-medium">会话 ID 伪装</span>
              <p class="text-xs text-muted-foreground">
                启用后在 15 分钟内固定 metadata.user_id 中的 session ID
              </p>
            </div>
            <Switch
              :model-value="claudeForm.session_id_masking_enabled"
              @update:model-value="(v: boolean) => claudeForm.session_id_masking_enabled = v"
            />
          </div>

          <div class="space-y-3 p-3 border rounded-lg bg-muted/50">
            <div class="flex items-center justify-between">
              <div class="space-y-0.5">
                <span class="text-sm font-medium">Cache TTL 统一</span>
                <p class="text-xs text-muted-foreground">
                  强制统一所有请求的 cache_control 类型，避免多人共用时行为指纹不一致
                </p>
              </div>
              <Switch
                :model-value="claudeForm.cache_ttl_override_enabled"
                @update:model-value="(v: boolean) => claudeForm.cache_ttl_override_enabled = v"
              />
            </div>
            <div
              v-if="claudeForm.cache_ttl_override_enabled"
              class="space-y-1.5"
            >
              <Label class="text-xs">目标 TTL 类型</Label>
              <select
                :value="claudeForm.cache_ttl_override_target"
                class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                @change="(e) => claudeForm.cache_ttl_override_target = (e.target as HTMLSelectElement).value"
              >
                <option value="ephemeral">
                  ephemeral (5 分钟)
                </option>
                <option value="1h">
                  1h (1 小时)
                </option>
              </select>
            </div>
          </div>

          <div class="flex items-center justify-between p-3 border rounded-lg bg-muted/50">
            <div class="space-y-0.5">
              <span class="text-sm font-medium">仅限 CLI 客户端</span>
              <p class="text-xs text-muted-foreground">
                仅允许 Claude Code CLI 客户端访问，拒绝非 CLI 流量
              </p>
            </div>
            <Switch
              :model-value="claudeForm.cli_only_enabled"
              @update:model-value="(v: boolean) => claudeForm.cli_only_enabled = v"
            />
          </div>
        </div>
      </template>
    </form>

    <template #footer>
      <Button
        variant="outline"
        :disabled="loading"
        @click="emit('update:modelValue', false)"
      >
        取消
      </Button>
      <Button
        :disabled="loading"
        @click="handleSave"
      >
        {{ loading ? '保存中...' : '保存' }}
      </Button>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { Dialog, Button, Input, Label, Switch } from '@/components/ui'
import { useToast } from '@/composables/useToast'
import { parseApiError } from '@/utils/errorParser'
import { updateProvider } from '@/api/endpoints'
import type { PoolAdvancedConfig, ClaudeCodeAdvancedConfig } from '@/api/endpoints/types/provider'

const props = defineProps<{
  modelValue: boolean
  providerId: string
  providerType?: string
  currentConfig: PoolAdvancedConfig | null
  currentClaudeConfig?: ClaudeCodeAdvancedConfig | null
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  saved: []
}>()

const { success, error: showError } = useToast()
const loading = ref(false)

const form = ref<PoolAdvancedConfig>({
  global_priority: null,
  sticky_session_ttl_seconds: null,
  lru_enabled: true,
  cost_window_seconds: null,
  cost_limit_per_key_tokens: null,
  cost_soft_threshold_percent: null,
  rate_limit_cooldown_seconds: null,
  overload_cooldown_seconds: null,
  health_policy_enabled: true,
})

interface ClaudeFormState {
  session_control_enabled: boolean
  max_sessions: number | undefined
  session_idle_timeout_minutes: number
  enable_tls_fingerprint: boolean
  session_id_masking_enabled: boolean
  cache_ttl_override_enabled: boolean
  cache_ttl_override_target: string
  cli_only_enabled: boolean
}

const claudeForm = ref<ClaudeFormState>({
  session_control_enabled: true,
  max_sessions: undefined,
  session_idle_timeout_minutes: 5,
  enable_tls_fingerprint: true,
  session_id_masking_enabled: true,
  cache_ttl_override_enabled: false,
  cache_ttl_override_target: 'ephemeral',
  cli_only_enabled: false,
})

function parseNum(v: string | number): number | undefined {
  if (v === '' || v === null || v === undefined) return undefined
  const n = Number(v)
  return isNaN(n) ? undefined : n
}

watch(() => props.modelValue, (v) => {
  if (v && props.currentConfig) {
    form.value = { ...props.currentConfig }
  } else if (v) {
    form.value = {
      global_priority: null,
      sticky_session_ttl_seconds: null,
      lru_enabled: true,
      cost_window_seconds: null,
      cost_limit_per_key_tokens: null,
      cost_soft_threshold_percent: null,
      rate_limit_cooldown_seconds: null,
      overload_cooldown_seconds: null,
      health_policy_enabled: true,
    }
  }

  // Claude Code 配置
  if (v && props.providerType === 'claude_code') {
    const cc = props.currentClaudeConfig
    if (cc) {
      // max_sessions 为 null 表示用户明确关闭了会话控制，其余情况默认开启
      const sessionOff = cc.max_sessions === null
      claudeForm.value = {
        session_control_enabled: !sessionOff,
        max_sessions: sessionOff ? undefined : (cc.max_sessions ?? undefined),
        session_idle_timeout_minutes: cc.session_idle_timeout_minutes ?? 5,
        enable_tls_fingerprint: cc.enable_tls_fingerprint ?? true,
        session_id_masking_enabled: cc.session_id_masking_enabled ?? true,
        cache_ttl_override_enabled: cc.cache_ttl_override_enabled ?? false,
        cache_ttl_override_target: cc.cache_ttl_override_target ?? 'ephemeral',
        cli_only_enabled: cc.cli_only_enabled ?? false,
      }
    } else {
      // 默认值：全部开启
      claudeForm.value = {
        session_control_enabled: true,
        max_sessions: undefined,
        session_idle_timeout_minutes: 5,
        enable_tls_fingerprint: true,
        session_id_masking_enabled: true,
        cache_ttl_override_enabled: false,
        cache_ttl_override_target: 'ephemeral',
        cli_only_enabled: false,
      }
    }
  }
})

async function handleSave() {
  loading.value = true
  try {
    const payload: Record<string, unknown> = {
      pool_advanced: {
        global_priority: form.value.global_priority ?? undefined,
        sticky_session_ttl_seconds: form.value.sticky_session_ttl_seconds ?? undefined,
        lru_enabled: form.value.lru_enabled,
        cost_window_seconds: form.value.cost_window_seconds ?? undefined,
        cost_limit_per_key_tokens: form.value.cost_limit_per_key_tokens ?? undefined,
        cost_soft_threshold_percent: form.value.cost_soft_threshold_percent ?? undefined,
        rate_limit_cooldown_seconds: form.value.rate_limit_cooldown_seconds ?? undefined,
        overload_cooldown_seconds: form.value.overload_cooldown_seconds ?? undefined,
        health_policy_enabled: form.value.health_policy_enabled,
      },
    }

    // Claude Code 特有配置
    if (props.providerType === 'claude_code') {
      payload.claude_code_advanced = {
        max_sessions: claudeForm.value.session_control_enabled
          ? (claudeForm.value.max_sessions ?? undefined)
          : null,
        session_idle_timeout_minutes: claudeForm.value.session_control_enabled
          ? (claudeForm.value.session_idle_timeout_minutes ?? 5)
          : null,
        enable_tls_fingerprint: claudeForm.value.enable_tls_fingerprint,
        session_id_masking_enabled: claudeForm.value.session_id_masking_enabled,
        cache_ttl_override_enabled: claudeForm.value.cache_ttl_override_enabled,
        cache_ttl_override_target: claudeForm.value.cache_ttl_override_enabled
          ? claudeForm.value.cache_ttl_override_target
          : 'ephemeral',
        cli_only_enabled: claudeForm.value.cli_only_enabled,
      }
    }

    await updateProvider(props.providerId, payload)
    success('号池配置已保存')
    emit('saved')
    emit('update:modelValue', false)
  } catch (err) {
    showError(parseApiError(err))
  } finally {
    loading.value = false
  }
}
</script>

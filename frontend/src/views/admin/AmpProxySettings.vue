<template>
  <PageContainer>
    <PageHeader
      title="AMP 代理"
      description="配置 Amp 上游控制面代理与 Provider API 别名入口"
    >
      <template #actions>
        <Button
          variant="outline"
          :disabled="loading"
          @click="loadConfig"
        >
          <RefreshCw
            class="w-4 h-4 mr-2"
            :class="{ 'animate-spin': loading }"
          />
          刷新
        </Button>
        <Button
          :disabled="loading || saving"
          @click="saveConfig"
        >
          <Save class="w-4 h-4 mr-2" />
          保存
        </Button>
      </template>
    </PageHeader>

    <div class="mt-6 space-y-5">
      <Card
        variant="default"
        class="p-6"
      >
        <div class="grid grid-cols-1 gap-5 lg:grid-cols-2">
          <div class="space-y-2">
            <Label for="amp-upstream-url">Upstream URL</Label>
            <Input
              id="amp-upstream-url"
              v-model="ampProxyConfig.upstream_url"
              :disabled="loading || saving"
              placeholder="https://ampcode.com"
            />
          </div>
          <div class="space-y-2">
            <Label for="amp-upstream-api-key">Upstream API Key</Label>
            <Input
              id="amp-upstream-api-key"
              v-model="ampProxyConfig.upstream_api_key"
              :disabled="loading || saving"
              type="password"
              autocomplete="off"
            />
          </div>
        </div>

        <div class="mt-5 flex items-start gap-3 rounded-lg border p-4">
          <Checkbox
            id="amp-fallback-to-upstream"
            v-model="ampProxyConfig.fallback_to_upstream_on_model_miss"
            :disabled="loading || saving"
            class="mt-0.5"
          />
          <div class="min-w-0 space-y-1">
            <Label
              for="amp-fallback-to-upstream"
              class="cursor-pointer"
            >
              模型未命中时回退到 AMP 上游
            </Label>
            <p class="text-sm text-muted-foreground">
              Aether 无可用候选时，将原始 AMP Provider 请求转发给配置的 AMP 上游。
            </p>
          </div>
        </div>

        <div class="mt-4 flex items-start gap-3 rounded-lg border p-4">
          <Checkbox
            id="amp-force-legacy-worker-runtime"
            v-model="ampProxyConfig.force_legacy_worker_runtime"
            :disabled="loading || saving"
            class="mt-0.5"
          />
          <div class="min-w-0 space-y-1">
            <Label
              for="amp-force-legacy-worker-runtime"
              class="cursor-pointer"
            >
              强制使用 legacy worker runtime
            </Label>
            <p class="text-sm text-muted-foreground">
              禁用 AMP thread-actors，让模型请求回到 Aether 的本地执行路由。
            </p>
          </div>
        </div>
      </Card>

      <Card
        variant="default"
        class="p-6"
      >
        <div class="flex items-center justify-between gap-3">
          <div>
            <h3 class="text-base font-semibold">
              多上游 API Key 路由
            </h3>
          </div>
          <Button
            variant="outline"
            :disabled="loading || saving"
            @click="addUpstreamRoute"
          >
            <Plus class="w-4 h-4 mr-2" />
            添加
          </Button>
        </div>

        <div class="mt-5 space-y-4">
          <div
            v-for="(route, index) in ampProxyConfig.upstream_api_keys"
            :key="index"
            class="rounded-lg border p-4"
          >
            <div class="flex items-start justify-between gap-3">
              <div class="grid min-w-0 flex-1 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
                <div class="space-y-2">
                  <Label :for="`amp-client-keys-${index}`">入站 API Key</Label>
                  <Textarea
                    :id="`amp-client-keys-${index}`"
                    v-model="routeApiKeyText[index]"
                    :disabled="loading || saving"
                    class="min-h-24 font-mono text-xs"
                    @update:model-value="syncRouteApiKeys(index)"
                  />
                </div>
                <div class="space-y-2">
                  <Label :for="`amp-route-upstream-key-${index}`">上游 API Key</Label>
                  <Input
                    :id="`amp-route-upstream-key-${index}`"
                    v-model="route.upstream_api_key"
                    :disabled="loading || saving"
                    type="password"
                    autocomplete="off"
                  />
                </div>
              </div>
              <Button
                variant="ghost"
                size="icon"
                class="h-9 w-9 shrink-0 text-muted-foreground"
                :disabled="loading || saving"
                @click="removeUpstreamRoute(index)"
              >
                <Trash2 class="w-4 h-4" />
              </Button>
            </div>
          </div>

          <div
            v-if="ampProxyConfig.upstream_api_keys.length === 0"
            class="rounded-lg border border-dashed py-8 text-center text-sm text-muted-foreground"
          >
            暂未配置多上游路由
          </div>
        </div>
      </Card>
    </div>
  </PageContainer>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { Plus, RefreshCw, Save, Trash2 } from 'lucide-vue-next'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import Checkbox from '@/components/ui/checkbox.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import Textarea from '@/components/ui/textarea.vue'
import { PageContainer, PageHeader } from '@/components/layout'
import { adminApi } from '@/api/admin'
import { useToast } from '@/composables/useToast'
import { log } from '@/utils/logger'
import { getErrorMessage } from '@/types/api-error'
import {
  createDefaultAmpProxyConfig,
  normalizeAmpProxyConfig,
  type AmpProxyConfig,
} from './module-management/ampProxyConfig'

const { success, error } = useToast()

const ampProxyConfig = ref<AmpProxyConfig>(createDefaultAmpProxyConfig())
const routeApiKeyText = ref<string[]>([])
const loading = ref(false)
const saving = ref(false)

function syncRouteTextFromConfig() {
  routeApiKeyText.value = ampProxyConfig.value.upstream_api_keys.map(route =>
    route.api_keys.join('\n')
  )
}

function splitApiKeys(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map(item => item.trim())
    .filter((item, index, array) => item.length > 0 && array.indexOf(item) === index)
}

function syncRouteApiKeys(index: number) {
  const route = ampProxyConfig.value.upstream_api_keys[index]
  if (!route) return
  route.api_keys = splitApiKeys(routeApiKeyText.value[index] ?? '')
}

function syncAllRouteApiKeys() {
  ampProxyConfig.value.upstream_api_keys.forEach((_, index) => syncRouteApiKeys(index))
}

function addUpstreamRoute() {
  ampProxyConfig.value.upstream_api_keys.push({
    api_keys: [],
    upstream_api_key: '',
  })
  routeApiKeyText.value.push('')
}

function removeUpstreamRoute(index: number) {
  ampProxyConfig.value.upstream_api_keys.splice(index, 1)
  routeApiKeyText.value.splice(index, 1)
}

async function loadConfig() {
  loading.value = true
  try {
    const response = await adminApi.getSystemConfig('amp_proxy')
    ampProxyConfig.value = normalizeAmpProxyConfig(response.value)
    syncRouteTextFromConfig()
  } catch (err) {
    error('获取 AMP 代理配置失败')
    log.error('获取 AMP 代理配置失败:', err)
  } finally {
    loading.value = false
  }
}

async function saveConfig() {
  saving.value = true
  try {
    syncAllRouteApiKeys()
    const normalized = normalizeAmpProxyConfig(ampProxyConfig.value)
    ampProxyConfig.value = normalized
    syncRouteTextFromConfig()
    await adminApi.updateSystemConfig('amp_proxy', normalized, 'AMP 代理配置')
    success('AMP 代理配置已保存')
  } catch (err) {
    error(getErrorMessage(err, '保存 AMP 代理配置失败'))
    log.error('保存 AMP 代理配置失败:', err)
  } finally {
    saving.value = false
  }
}

onMounted(() => {
  loadConfig()
})
</script>

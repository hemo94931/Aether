<template>
  <Dialog
    :model-value="isOpen"
    title="添加账号"
    :icon="UserPlus"
    size="md"
    @update:model-value="handleDialogUpdate"
  >
    <div class="space-y-5">
      <!-- 加载中 -->
      <div
        v-if="oauth.starting && !oauth.authorization_url"
        class="py-12 text-center"
      >
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary mx-auto mb-4" />
        <p class="text-sm text-muted-foreground">
          正在准备授权...
        </p>
      </div>

      <!-- 授权流程 -->
      <template v-else-if="oauth.authorization_url">
        <!-- 步骤 1: 打开授权链接 -->
        <div class="space-y-2">
          <p class="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            第一步 · 前往授权
          </p>
          <p class="text-xs text-muted-foreground">
            点击下方按钮在浏览器中完成登录授权
          </p>
          <div class="flex gap-2 pt-1">
            <Button
              size="sm"
              :disabled="oauthBusy"
              @click="openAuthorizationUrl"
            >
              <ExternalLink class="w-3.5 h-3.5 mr-1.5" />
              前往授权
            </Button>
            <Button
              size="sm"
              variant="outline"
              :disabled="oauthBusy"
              @click="copyToClipboard(oauth.authorization_url)"
            >
              <Copy class="w-3.5 h-3.5 mr-1.5" />
              复制链接
            </Button>
          </div>
        </div>

        <Separator />

        <!-- 步骤 2: 粘贴回调地址 -->
        <div class="space-y-2">
          <p class="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            第二步 · 粘贴回调
          </p>
          <p class="text-xs text-muted-foreground">
            授权完成后，复制浏览器地址栏的完整 URL 并粘贴到下方
          </p>
          <div class="pt-1">
            <Textarea
              v-model="oauth.callback_url"
              :disabled="oauthBusy"
              placeholder="http://localhost:xxx/callback?code=..."
              class="min-h-[80px] text-xs font-mono break-all !rounded-xl"
              spellcheck="false"
            />
          </div>
        </div>
      </template>
    </div>

    <template #footer>
      <Button
        variant="outline"
        @click="handleClose"
      >
        取消
      </Button>
      <Button
        :disabled="!canCompleteOAuth"
        @click="handleCompleteOAuth"
      >
        {{ oauth.completing ? '验证中...' : '验证' }}
      </Button>
    </template>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { Dialog, Button, Textarea, Separator } from '@/components/ui'
import { UserPlus, Copy, ExternalLink } from 'lucide-vue-next'
import { useToast } from '@/composables/useToast'
import { useClipboard } from '@/composables/useClipboard'
import { parseApiError } from '@/utils/errorParser'
import {
  startProviderLevelOAuth,
  completeProviderLevelOAuth,
} from '@/api/endpoints'

const props = defineProps<{
  open: boolean
  providerId: string | null
}>()

const emit = defineEmits<{
  close: []
  saved: []
}>()

const { success, error: showError } = useToast()
const { copyToClipboard } = useClipboard()

// OAuth 状态
interface OAuthState {
  authorization_url: string
  redirect_uri: string
  instructions: string
  provider_type: string
  callback_url: string
  starting: boolean
  completing: boolean
}

function createInitialOAuthState(): OAuthState {
  return {
    authorization_url: '',
    redirect_uri: '',
    instructions: '',
    provider_type: '',
    callback_url: '',
    starting: false,
    completing: false,
  }
}

const oauth = ref<OAuthState>(createInitialOAuthState())

const isOpen = computed(() => props.open)

const oauthBusy = computed(() =>
  oauth.value.starting || oauth.value.completing
)

const canCompleteOAuth = computed(() => {
  if (!oauth.value.authorization_url) return false
  if (!oauth.value.callback_url.trim()) return false
  return !oauthBusy.value
})

function resetForm() {
  oauth.value = createInitialOAuthState()
}

function handleDialogUpdate(value: boolean) {
  if (!value) {
    handleClose()
  }
}

function handleClose() {
  resetForm()
  emit('close')
}

function openAuthorizationUrl() {
  const url = oauth.value.authorization_url
  if (!url) return
  window.open(url, '_blank', 'noopener,noreferrer')
}

// 对话框打开时获取授权 URL（不创建 key）
async function initOAuth() {
  if (!props.providerId) return

  oauth.value.starting = true
  try {
    const resp = await startProviderLevelOAuth(props.providerId)
    oauth.value.authorization_url = resp.authorization_url
    oauth.value.redirect_uri = resp.redirect_uri
    oauth.value.instructions = resp.instructions
    oauth.value.provider_type = resp.provider_type
  } catch (err: any) {
    const errorMessage = parseApiError(err, '初始化授权失败')
    showError(errorMessage, '错误')
    handleClose()
  } finally {
    oauth.value.starting = false
  }
}

// 完成授权（此时才创建 key）
async function handleCompleteOAuth() {
  if (!canCompleteOAuth.value || !props.providerId) return
  oauth.value.completing = true
  try {
    await completeProviderLevelOAuth(props.providerId, {
      callback_url: oauth.value.callback_url.trim(),
    })
    success('授权成功，账号已添加')
    emit('saved')
    handleClose()
  } catch (err: any) {
    const errorMessage = parseApiError(err, '完成授权失败')
    showError(errorMessage, '错误')
  } finally {
    oauth.value.completing = false
  }
}

// 监听对话框打开
watch(() => props.open, (newOpen) => {
  if (newOpen) {
    initOAuth()
  } else {
    resetForm()
  }
})
</script>

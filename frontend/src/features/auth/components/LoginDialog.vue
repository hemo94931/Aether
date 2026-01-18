<template>
  <Dialog
    v-model="isOpen"
    size="md"
    no-padding
  >
    <div class="px-6 py-6 sm:px-8 sm:py-8">
      <!-- Logo 和标题 -->
      <div class="flex flex-col items-center text-center mb-6">
        <img
          src="/aether_adaptive.svg"
          alt="Aether"
          class="h-10 w-10 mb-3"
        >
        <h2 class="text-xl font-semibold text-foreground">
          登录到 Aether
        </h2>
      </div>

      <!-- Demo 模式提示 -->
      <div
        v-if="isDemo"
        class="rounded-lg border border-primary/20 bg-primary/5 p-3 mb-5"
      >
        <p class="text-xs font-medium text-foreground mb-2">
          演示模式
        </p>
        <div class="space-y-1.5">
          <button
            type="button"
            class="flex items-center gap-2 text-xs text-muted-foreground hover:text-foreground transition-colors w-full"
            @click="fillDemoAccount('admin')"
          >
            <span class="inline-flex items-center justify-center w-4 h-4 rounded bg-primary/20 text-primary text-[10px] font-bold">A</span>
            <span>admin@demo.aether.io / demo123</span>
          </button>
          <button
            type="button"
            class="flex items-center gap-2 text-xs text-muted-foreground hover:text-foreground transition-colors w-full"
            @click="fillDemoAccount('user')"
          >
            <span class="inline-flex items-center justify-center w-4 h-4 rounded bg-muted text-muted-foreground text-[10px] font-bold">U</span>
            <span>user@demo.aether.io / demo123</span>
          </button>
        </div>
      </div>

      <!-- OAuth 登录按钮 -->
      <div
        v-if="oauthProviders.length > 0"
        class="mb-5"
      >
        <!-- 单个 provider: 完整按钮 -->
        <div
          v-if="oauthProviders.length === 1"
          class="space-y-2"
        >
          <button
            type="button"
            class="oauth-btn"
            @click="handleOAuthLogin(oauthProviders[0].provider_type)"
          >
            <span
              class="oauth-icon"
              v-html="getOAuthIcon(oauthProviders[0].provider_type)"
            />
            <span>使用 {{ oauthProviders[0].display_name }} 登录</span>
          </button>
        </div>

        <!-- 多个 provider: 图标按钮组 -->
        <div
          v-else
          class="flex flex-col items-center gap-3"
        >
          <span class="text-xs text-muted-foreground">使用以下方式登录</span>
          <div class="flex items-center justify-center gap-3">
            <button
              v-for="p in oauthProviders"
              :key="p.provider_type"
              type="button"
              class="oauth-icon-btn"
              :title="p.display_name"
              @click="handleOAuthLogin(p.provider_type)"
            >
              <span
                class="oauth-icon-lg"
                v-html="getOAuthIcon(p.provider_type)"
              />
            </button>
          </div>
        </div>
      </div>

      <!-- 分隔线 -->
      <div
        v-if="oauthProviders.length > 0"
        class="flex items-center gap-3 mb-5"
      >
        <div class="flex-1 h-px bg-border" />
        <span class="text-xs text-muted-foreground px-2">或使用账号密码</span>
        <div class="flex-1 h-px bg-border" />
      </div>

      <!-- 认证方式切换 -->
      <div
        v-if="showAuthTypeTabs"
        class="auth-type-tabs mb-4"
      >
        <button
          type="button"
          class="auth-tab"
          :class="[authType === 'local' && 'active']"
          @click="authType = 'local'"
        >
          本地登录
        </button>
        <button
          type="button"
          class="auth-tab"
          :class="[authType === 'ldap' && 'active']"
          @click="authType = 'ldap'"
        >
          LDAP 登录
        </button>
      </div>

      <!-- 登录表单 -->
      <form
        class="space-y-4"
        @submit.prevent="handleLogin"
      >
        <div class="space-y-1.5">
          <div class="flex items-center justify-between">
            <Label
              for="login-email"
              class="text-sm"
            >
              {{ emailLabel }}
            </Label>
            <button
              v-if="ldapExclusive && authType === 'ldap'"
              type="button"
              class="text-xs text-muted-foreground/60 hover:text-muted-foreground transition-colors"
              @click="authType = 'local'"
            >
              管理员本地登录
            </button>
            <button
              v-if="ldapExclusive && authType === 'local'"
              type="button"
              class="text-xs text-muted-foreground/60 hover:text-muted-foreground transition-colors"
              @click="authType = 'ldap'"
            >
              返回 LDAP 登录
            </button>
          </div>
          <Input
            id="login-email"
            v-model="form.email"
            type="text"
            required
            placeholder="用户名或邮箱"
            autocomplete="off"
          />
        </div>

        <div class="space-y-1.5">
          <Label
            for="login-password"
            class="text-sm"
          >
            密码
          </Label>
          <Input
            id="login-password"
            v-model="form.password"
            type="password"
            required
            placeholder="输入密码"
            autocomplete="off"
            @keyup.enter="handleLogin"
          />
        </div>

        <!-- 登录按钮 -->
        <Button
          type="submit"
          :disabled="authStore.loading"
          class="w-full h-10"
        >
          {{ authStore.loading ? '登录中...' : '登录' }}
        </Button>

        <!-- 提示信息 -->
        <p
          v-if="!isDemo && !allowRegistration"
          class="text-xs text-muted-foreground text-center"
        >
          如需开通账户，请联系管理员
        </p>
      </form>

      <!-- 注册链接 -->
      <div
        v-if="allowRegistration"
        class="mt-5 pt-5 border-t border-border text-center text-sm text-muted-foreground"
      >
        还没有账户？
        <button
          type="button"
          class="text-primary hover:text-primary/80 font-medium transition-colors"
          @click="handleSwitchToRegister"
        >
          立即注册
        </button>
      </div>
    </div>
  </Dialog>

  <!-- Register Dialog -->
  <RegisterDialog
    v-model:open="showRegisterDialog"
    :require-email-verification="requireEmailVerification"
    :email-configured="emailConfigured"
    @success="handleRegisterSuccess"
    @switch-to-login="handleSwitchToLogin"
  />
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { Dialog } from '@/components/ui'
import Button from '@/components/ui/button.vue'
import Input from '@/components/ui/input.vue'
import Label from '@/components/ui/label.vue'
import { useAuthStore } from '@/stores/auth'
import { useToast } from '@/composables/useToast'
import { isDemoMode, DEMO_ACCOUNTS } from '@/config/demo'
import RegisterDialog from './RegisterDialog.vue'
import { authApi } from '@/api/auth'
import { oauthApi, type OAuthProviderInfo } from '@/api/oauth'
import { getApiUrl } from '@/utils/url'

// OAuth provider icons
const OAUTH_ICONS: Record<string, string> = {
  linuxdo: `<svg viewBox="0 0 120 120" xmlns="http://www.w3.org/2000/svg"><clipPath id="ld"><circle cx="60" cy="60" r="47"/></clipPath><circle fill="#f0f0f0" cx="60" cy="60" r="50"/><rect fill="#1c1c1e" clip-path="url(#ld)" x="10" y="10" width="100" height="30"/><rect fill="#f0f0f0" clip-path="url(#ld)" x="10" y="40" width="100" height="40"/><rect fill="#ffb003" clip-path="url(#ld)" x="10" y="80" width="100" height="30"/></svg>`,
  github: `<svg viewBox="0 0 24 24" fill="currentColor"><path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/></svg>`,
  google: `<svg viewBox="0 0 24 24"><path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/><path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/><path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/><path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/></svg>`,
}

function getOAuthIcon(providerType: string): string {
  return OAUTH_ICONS[providerType] || OAUTH_ICONS.github
}

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
}>()

const router = useRouter()
const authStore = useAuthStore()
const { success: showSuccess, warning: showWarning, error: showError } = useToast()

const isOpen = ref(props.modelValue)
const isDemo = computed(() => isDemoMode())
const showRegisterDialog = ref(false)
const requireEmailVerification = ref(false)
const emailConfigured = ref(true) // 邮箱服务是否已配置
const allowRegistration = ref(false) // 由系统配置控制，默认关闭

// LDAP authentication settings
const PREFERRED_AUTH_TYPE_KEY = 'aether_preferred_auth_type'
function getStoredAuthType(): 'local' | 'ldap' {
  const stored = localStorage.getItem(PREFERRED_AUTH_TYPE_KEY)
  return (stored === 'ldap' || stored === 'local') ? stored : 'local'
}
const authType = ref<'local' | 'ldap'>(getStoredAuthType())
const localEnabled = ref(true)
const ldapEnabled = ref(false)
const ldapExclusive = ref(false)

const oauthProviders = ref<OAuthProviderInfo[]>([])

// 保存用户的认证类型偏好
watch(authType, (newType) => {
  localStorage.setItem(PREFERRED_AUTH_TYPE_KEY, newType)
})

const showAuthTypeTabs = computed(() => {
  return localEnabled.value && ldapEnabled.value && !ldapExclusive.value
})

const emailLabel = computed(() => {
  return '用户名/邮箱'
})

watch(() => props.modelValue, (val) => {
  isOpen.value = val
  // 打开对话框时重置表单
  if (val) {
    form.value = {
      email: '',
      password: ''
    }
  }
})

watch(isOpen, (val) => {
  emit('update:modelValue', val)
})

const form = ref({
  email: '',
  password: ''
})

function fillDemoAccount(type: 'admin' | 'user') {
  const account = DEMO_ACCOUNTS[type]
  form.value.email = account.email
  form.value.password = account.password
}

async function handleLogin() {
  if (!form.value.email || !form.value.password) {
    showWarning('请输入邮箱和密码')
    return
  }

  const success = await authStore.login(form.value.email, form.value.password, authType.value)
  if (success) {
    showSuccess('登录成功，正在跳转...')

    // 关闭对话框
    isOpen.value = false

    // 延迟一下让用户看到成功消息
    setTimeout(() => {
      // 根据用户角色跳转到不同的仪表盘
      const targetPath = authStore.user?.role === 'admin' ? '/admin/dashboard' : '/dashboard'
      router.push(targetPath)
    }, 1000)
  } else {
    showError(authStore.error || '登录失败，请检查邮箱和密码')
  }
}

function handleOAuthLogin(providerType: string) {
  // 如果 sessionStorage 中没有 redirectPath（用户直接点击登录而非被守卫拦截），
  // 则不设置，让 AuthCallback 使用默认跳转逻辑
  window.location.href = getApiUrl(`/api/oauth/${providerType}/authorize`)
}

function handleSwitchToRegister() {
  isOpen.value = false
  showRegisterDialog.value = true
}

function handleRegisterSuccess() {
  showRegisterDialog.value = false
  showSuccess('注册成功！请登录')
  isOpen.value = true
}

function handleSwitchToLogin() {
  showRegisterDialog.value = false
  isOpen.value = true
}

// Load authentication and registration settings on mount
onMounted(async () => {
  try {
    const [regSettings, authSettings, providers] = await Promise.all([
      authApi.getRegistrationSettings(),
      authApi.getAuthSettings(),
      oauthApi.getProviders().catch(() => []),
    ])

    allowRegistration.value = !!regSettings.enable_registration
    requireEmailVerification.value = !!regSettings.require_email_verification
    emailConfigured.value = !!regSettings.email_configured

    localEnabled.value = authSettings.local_enabled
    ldapEnabled.value = authSettings.ldap_enabled
    ldapExclusive.value = authSettings.ldap_exclusive
    // 若仅允许 LDAP 登录，则禁用本地注册入口
    if (ldapExclusive.value) {
      allowRegistration.value = false
    }

    // Set default auth type based on settings
    if (authSettings.ldap_exclusive) {
      authType.value = 'ldap'
    } else if (!authSettings.local_enabled && authSettings.ldap_enabled) {
      authType.value = 'ldap'
    } else {
      authType.value = 'local'
    }

    oauthProviders.value = providers
  } catch {
    // If获取失败，保持默认：关闭注册 & 关闭邮箱验证 & 使用本地认证
    allowRegistration.value = false
    requireEmailVerification.value = false
    emailConfigured.value = false
    localEnabled.value = true
    ldapEnabled.value = false
    ldapExclusive.value = false
    authType.value = 'local'
    oauthProviders.value = []
  }
})
</script>

<style scoped>
.oauth-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  width: 100%;
  padding: 0.625rem 1rem;
  font-size: 0.875rem;
  font-weight: 500;
  color: hsl(var(--foreground));
  background: hsl(var(--background));
  border: 1px solid hsl(var(--border));
  border-radius: 0.5rem;
  cursor: pointer;
  transition: all 0.15s ease;
}

.oauth-btn:hover {
  background: hsl(var(--muted));
  border-color: hsl(var(--primary) / 0.5);
}

.oauth-icon {
  width: 1.25rem;
  height: 1.25rem;
  flex-shrink: 0;
}

.oauth-icon :deep(svg) {
  width: 100%;
  height: 100%;
}

.oauth-icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 3rem;
  height: 3rem;
  background: hsl(var(--background));
  border: 1px solid hsl(var(--border));
  border-radius: 0.75rem;
  cursor: pointer;
  transition: all 0.15s ease;
}

.oauth-icon-btn:hover {
  background: hsl(var(--muted));
  border-color: hsl(var(--primary) / 0.5);
  transform: translateY(-1px);
}

.oauth-icon-lg {
  width: 1.5rem;
  height: 1.5rem;
}

.oauth-icon-lg :deep(svg) {
  width: 100%;
  height: 100%;
}

.auth-type-tabs {
  display: flex;
  border-bottom: 1px solid hsl(var(--border));
}

.auth-tab {
  flex: 1;
  padding: 0.5rem 1rem;
  font-size: 0.875rem;
  font-weight: 500;
  color: hsl(var(--muted-foreground));
  background: transparent;
  border: none;
  cursor: pointer;
  transition: color 0.15s ease;
  position: relative;
}

.auth-tab::after {
  content: '';
  position: absolute;
  bottom: -1px;
  left: 0;
  right: 0;
  height: 2px;
  background: transparent;
  transition: background 0.15s ease;
}

.auth-tab:hover:not(.active) {
  color: hsl(var(--foreground));
}

.auth-tab.active {
  color: hsl(var(--primary));
  font-weight: 600;
}

.auth-tab.active::after {
  background: hsl(var(--primary));
}
</style>

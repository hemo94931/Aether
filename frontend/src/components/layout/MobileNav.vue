<template>
  <div class="lg:hidden">
    <div class="sticky top-0 z-40 space-y-4 pb-4">
      <!-- Logo头部 - 移动端优化 -->
      <div class="flex items-center gap-3 rounded-2xl bg-card/90 px-4 py-3 shadow-lg shadow-primary/20 ring-1 ring-border backdrop-blur">
        <div class="flex h-10 w-10 sm:h-12 sm:w-12 items-center justify-center rounded-2xl bg-background shadow-md shadow-primary/20 flex-shrink-0">
          <img
            src="/aether_adaptive.svg"
            alt="Logo"
            class="h-8 w-8 sm:h-10 sm:w-10"
          >
        </div>
        <!-- 文字部分 - 小屏隐藏 -->
        <div class="hidden sm:block">
          <p class="text-sm font-semibold text-foreground">
            Aether
          </p>
          <p class="text-xs text-muted-foreground">
            AI 控制中心
          </p>
        </div>
      </div>

      <button
        type="button"
        class="flex w-full items-center gap-3 rounded-2xl bg-card/80 px-4 py-3 shadow-lg shadow-primary/20 ring-1 ring-border backdrop-blur transition hover:ring-primary/30"
        @click="toggleMenu"
      >
        <div class="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10 text-primary flex-shrink-0">
          <Menu class="h-5 w-5" />
        </div>
        <div class="flex flex-1 flex-col text-left min-w-0">
          <span class="text-sm font-semibold text-foreground truncate">
            快速导航
          </span>
          <span class="text-xs text-muted-foreground truncate">
            {{ activeItem ? `当前：${activeItem.name}` : '选择功能页面' }}
          </span>
        </div>
        <ChevronDown
          class="h-4 w-4 text-muted-foreground transition-transform duration-200 flex-shrink-0"
          :class="{ 'rotate-180': isOpen }"
        />
      </button>

      <Transition
        enter-active-class="transition duration-200 ease-out"
        enter-from-class="opacity-0 -translate-y-2 scale-95"
        enter-to-class="opacity-100 translate-y-0 scale-100"
        leave-active-class="transition duration-150 ease-in"
        leave-from-class="opacity-100 translate-y-0 scale-100"
        leave-to-class="opacity-0 -translate-y-1 scale-95"
      >
        <div
          v-if="isOpen"
          class="space-y-3 rounded-3xl bg-card/95 p-4 shadow-2xl ring-1 ring-border backdrop-blur-xl"
        >
          <SidebarNav
            :items="props.items"
            :is-active="isLinkActive"
            :active-path="props.activePath"
            list-class="space-y-2"
            @navigate="handleNavigate"
          />
        </div>
      </Transition>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { Component } from 'vue'
import { computed, ref, watch } from 'vue'
import { ChevronDown, Menu } from 'lucide-vue-next'
import SidebarNav from '@/components/layout/SidebarNav.vue'

export interface NavigationItem {
  name: string
  href: string
  icon: Component
  description?: string
}

export interface NavigationGroup {
  title?: string
  items: NavigationItem[]
}

const props = defineProps<{
  items: NavigationGroup[]
  activePath?: string
  isActive?: (href: string) => boolean
  isDark?: boolean
}>()

const isOpen = ref(false)

const activeItem = computed(() => {
  for (const group of props.items) {
    const found = group.items.find(item => isLinkActive(item.href))
    if (found) return found
  }
  return null
})

function isLinkActive(href: string) {
  if (props.isActive) {
    return props.isActive(href)
  }
  if (props.activePath) {
    return props.activePath === href || props.activePath.startsWith(`${href}/`)
  }
  return false
}

function toggleMenu() {
  isOpen.value = !isOpen.value
}

function handleNavigate() {
  isOpen.value = false
}

watch(() => props.activePath, () => {
  isOpen.value = false
})
</script>

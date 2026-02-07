<template>
  <div class="space-y-1.5">
    <Select
      :model-value="modelValue"
      :disabled="proxyNodesStore.loading || nodeOptions.length === 0"
      @update:model-value="(v: string) => $emit('update:modelValue', v)"
    >
      <SelectTrigger :class="triggerClass">
        <SelectValue
          :placeholder="proxyNodesStore.loading
            ? '加载节点列表中...'
            : nodeOptions.length === 0
              ? '暂无可用节点'
              : '选择代理节点...'"
        />
      </SelectTrigger>
      <SelectContent>
        <SelectItem
          v-for="node in nodeOptions"
          :key="node.id"
          :value="node.id"
        >
          {{ node.name }}{{ node.region ? ` · ${node.region}` : '' }} ({{ node.ip }}:{{ node.port }})
        </SelectItem>
      </SelectContent>
    </Select>
    <p
      v-if="!proxyNodesStore.loading && nodeOptions.length === 0"
      class="text-xs text-muted-foreground"
    >
      暂无在线代理节点，请在「代理节点」页面添加
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from '@/components/ui'
import { useProxyNodesStore } from '@/stores/proxy-nodes'

const props = defineProps<{
  modelValue: string
  triggerClass?: string
}>()

defineEmits<{
  'update:modelValue': [value: string]
}>()

const proxyNodesStore = useProxyNodesStore()

/** 在线节点 + 保留当前已选节点（可能已离线） */
const nodeOptions = computed(() => {
  const online = proxyNodesStore.onlineNodes
  if (props.modelValue) {
    const found = online.find(n => n.id === props.modelValue)
    if (!found) {
      const allNode = proxyNodesStore.nodes.find(n => n.id === props.modelValue)
      if (allNode) return [allNode, ...online]
    }
  }
  return online
})

/** 供父组件调用：启用代理时懒加载节点列表 */
function ensureLoaded() {
  proxyNodesStore.ensureLoaded()
}

defineExpose({ ensureLoaded })
</script>

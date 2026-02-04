<template>
  <div class="flex flex-wrap items-center gap-2">
    <Select
      v-model:open="presetSelectOpen"
      v-model="selectedPreset"
    >
      <SelectTrigger class="h-8 w-32 text-xs border-border/60">
        <SelectValue placeholder="选择时间段" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="today">
          今天
        </SelectItem>
        <SelectItem value="yesterday">
          昨天
        </SelectItem>
        <SelectItem value="last7days">
          最近7天
        </SelectItem>
        <SelectItem value="last30days">
          最近30天
        </SelectItem>
        <SelectItem value="last90days">
          最近90天
        </SelectItem>
        <SelectItem value="this_week">
          本周
        </SelectItem>
        <SelectItem value="last_week">
          上周
        </SelectItem>
        <SelectItem value="this_month">
          本月
        </SelectItem>
        <SelectItem value="last_month">
          上月
        </SelectItem>
        <SelectItem value="this_year">
          今年
        </SelectItem>
        <SelectItem value="custom">
          自定义
        </SelectItem>
      </SelectContent>
    </Select>

    <div
      v-if="selectedPreset === 'custom'"
      class="flex items-center gap-2"
    >
      <Input
        v-model="startDate"
        type="date"
        class="h-8 w-36 text-xs border-border/60"
      />
      <span class="text-xs text-muted-foreground">至</span>
      <Input
        v-model="endDate"
        type="date"
        class="h-8 w-36 text-xs border-border/60"
      />
    </div>

    <Select
      v-if="showGranularity"
      v-model:open="granularitySelectOpen"
      v-model="selectedGranularity"
    >
      <SelectTrigger class="h-8 w-24 text-xs border-border/60">
        <SelectValue placeholder="粒度" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem
          v-if="allowHourly && canUseHourly"
          value="hour"
        >
          小时
        </SelectItem>
        <SelectItem value="day">
          天
        </SelectItem>
        <SelectItem value="week">
          周
        </SelectItem>
        <SelectItem value="month">
          月
        </SelectItem>
      </SelectContent>
    </Select>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Input
} from '@/components/ui'
import type { DateRangeParams } from '@/features/usage/types'

const props = defineProps<{
  modelValue: DateRangeParams
  showGranularity?: boolean
  allowHourly?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: DateRangeParams]
}>()

const selectedPreset = ref(props.modelValue.preset || 'last7days')
const startDate = ref(props.modelValue.start_date || '')
const endDate = ref(props.modelValue.end_date || '')
const selectedGranularity = ref(props.modelValue.granularity || 'day')
const presetSelectOpen = ref(false)
const granularitySelectOpen = ref(false)

const showGranularity = computed(() => props.showGranularity !== false)
const allowHourly = computed(() => props.allowHourly === true)

const canUseHourly = computed(() => {
  if (selectedPreset.value === 'today' || selectedPreset.value === 'yesterday') return true
  if (selectedPreset.value === 'custom' && startDate.value && endDate.value) {
    return startDate.value === endDate.value
  }
  return false
})

watch(() => props.modelValue, (value) => {
  if (value.preset) selectedPreset.value = value.preset
  if (value.start_date !== undefined) startDate.value = value.start_date || ''
  if (value.end_date !== undefined) endDate.value = value.end_date || ''
  if (value.granularity) selectedGranularity.value = value.granularity
}, { deep: true })

watch([selectedPreset, startDate, endDate, selectedGranularity], () => {
  if (!allowHourly.value || !canUseHourly.value) {
    if (selectedGranularity.value === 'hour') {
      selectedGranularity.value = 'day'
    }
  }

  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone
  const tz_offset_minutes = -new Date().getTimezoneOffset()

  if (selectedPreset.value === 'custom') {
    if (!startDate.value || !endDate.value) return
    const start = startDate.value <= endDate.value ? startDate.value : endDate.value
    const end = endDate.value >= startDate.value ? endDate.value : startDate.value
    emit('update:modelValue', {
      start_date: start,
      end_date: end,
      granularity: selectedGranularity.value,
      timezone,
      tz_offset_minutes
    })
    return
  }

  emit('update:modelValue', {
    preset: selectedPreset.value,
    granularity: selectedGranularity.value,
    timezone,
    tz_offset_minutes
  })
}, { immediate: true })
</script>

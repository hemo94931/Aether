<template>
  <div class="space-y-6 px-4 sm:px-6 lg:px-0">
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
      <div>
        <h1 class="text-lg font-semibold">
          成本分析
        </h1>
        <p class="text-xs text-muted-foreground">
          成本趋势、预测与节省统计
        </p>
      </div>
      <TimeRangePicker v-model="timeRange" />
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
      <Card class="p-4 space-y-2">
        <div class="text-xs text-muted-foreground">
          缓存节省
        </div>
        <div class="text-lg font-semibold">
          {{ formatCurrency(costSavings?.cache_savings ?? 0) }}
        </div>
        <div class="text-xs text-muted-foreground">
          读取成本 {{ formatCurrency(costSavings?.cache_read_cost ?? 0) }}
        </div>
      </Card>
      <Card class="p-4 space-y-2">
        <div class="text-xs text-muted-foreground">
          缓存读取 Tokens
        </div>
        <div class="text-lg font-semibold">
          {{ formatTokens(costSavings?.cache_read_tokens ?? 0) }}
        </div>
        <div class="text-xs text-muted-foreground">
          预计全额成本 {{ formatCurrency(costSavings?.estimated_full_cost ?? 0) }}
        </div>
      </Card>
      <Card class="p-4 space-y-2">
        <div class="text-xs text-muted-foreground">
          缓存创建成本
        </div>
        <div class="text-lg font-semibold">
          {{ formatCurrency(costSavings?.cache_creation_cost ?? 0) }}
        </div>
        <div class="text-xs text-muted-foreground">
          基于当前时间范围
        </div>
      </Card>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <Card class="p-4">
        <CostForecastChart
          title="成本趋势预测"
          :history="forecastHistory"
          :forecast="forecastFuture"
          :loading="forecastLoading"
        />
      </Card>
      <QuotaProgressCard
        title="月卡消耗进度"
        :providers="quotaProviders"
        :loading="quotaLoading"
      />
    </div>

    <UsageProviderTable
      :data="providerStats"
      :is-admin="true"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import Card from '@/components/ui/card.vue'
import { TimeRangePicker } from '@/components/common'
import { CostForecastChart, QuotaProgressCard } from '@/components/stats'
import { UsageProviderTable } from '@/features/usage/components'
import { adminApi, type CostForecastResponse, type CostSavingsResponse, type QuotaUsageProvider } from '@/api/admin'
import { usageApi } from '@/api/usage'
import { formatCurrency, formatTokens } from '@/utils/format'
import { getDateRangeFromPeriod } from '@/features/usage/composables'
import type { DateRangeParams } from '@/features/usage/types'
import type { ProviderStatsItem } from '@/features/usage/types'

const timeRange = ref<DateRangeParams>(getDateRangeFromPeriod('last30days'))

const forecast = ref<CostForecastResponse | null>(null)
const costSavings = ref<CostSavingsResponse | null>(null)
const quotaProviders = ref<QuotaUsageProvider[]>([])
const providerStats = ref<ProviderStatsItem[]>([])

const forecastLoading = ref(false)
const quotaLoading = ref(false)

const forecastHistory = computed(() => forecast.value?.history || [])
const forecastFuture = computed(() => forecast.value?.forecast || [])

function buildTimeRangeParams() {
  return {
    start_date: timeRange.value.start_date,
    end_date: timeRange.value.end_date,
    preset: timeRange.value.preset,
    timezone: timeRange.value.timezone,
    tz_offset_minutes: timeRange.value.tz_offset_minutes
  }
}

async function loadForecast() {
  forecastLoading.value = true
  try {
    forecast.value = await adminApi.getCostForecast(buildTimeRangeParams())
  } finally {
    forecastLoading.value = false
  }
}

async function loadSavings() {
  costSavings.value = await adminApi.getCostSavings(buildTimeRangeParams())
}

async function loadQuotaUsage() {
  quotaLoading.value = true
  try {
    const response = await adminApi.getQuotaUsage()
    quotaProviders.value = response.providers
  } finally {
    quotaLoading.value = false
  }
}

async function loadProviderStats() {
  providerStats.value = await usageApi.getUsageByProvider({
    ...buildTimeRangeParams(),
    limit: 8
  })
}

async function loadAll() {
  await Promise.all([loadForecast(), loadSavings(), loadQuotaUsage(), loadProviderStats()])
}

watch(timeRange, loadAll, { deep: true })

onMounted(loadAll)
</script>

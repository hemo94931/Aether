<template>
  <div class="space-y-6 px-4 sm:px-6 lg:px-0">
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
      <div>
        <h1 class="text-lg font-semibold">
          性能分析
        </h1>
        <p class="text-xs text-muted-foreground">
          延迟分布与错误统计
        </p>
      </div>
      <TimeRangePicker v-model="timeRange" />
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <Card class="p-4">
        <PercentileChart
          title="响应延迟百分位"
          :series="percentiles"
          mode="response"
          :loading="percentileLoading"
        />
      </Card>
      <Card class="p-4">
        <PercentileChart
          title="首字节延迟百分位"
          :series="percentiles"
          mode="ttfb"
          :loading="percentileLoading"
        />
      </Card>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <Card class="p-4">
        <ErrorDistributionChart
          title="错误分布"
          :distribution="errorDistribution"
          :loading="errorLoading"
        />
      </Card>
      <Card class="p-4 space-y-3">
        <h3 class="text-sm font-semibold">
          错误趋势
        </h3>
        <div
          v-if="errorLoading"
          class="p-6"
        >
          <LoadingState />
        </div>
        <div
          v-else
          class="h-[260px]"
        >
          <LineChart :data="errorTrendChartData" />
        </div>
      </Card>
    </div>

    <Card class="p-4 space-y-3">
      <h3 class="text-sm font-semibold">
        提供商健康度
      </h3>
      <div
        v-if="providerLoading"
        class="p-4"
      >
        <LoadingState />
      </div>
      <div
        v-else
        class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 text-sm"
      >
        <div
          v-for="provider in providerStatus"
          :key="provider.name"
          class="p-3 border rounded-lg"
        >
          <div class="flex items-center justify-between">
            <span class="font-medium">{{ provider.name }}</span>
            <span class="text-xs text-muted-foreground">{{ provider.requests }} 请求</span>
          </div>
          <div class="text-xs text-muted-foreground mt-1">
            状态: {{ provider.status }}
          </div>
        </div>
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import Card from '@/components/ui/card.vue'
import { LoadingState, TimeRangePicker } from '@/components/common'
import { ErrorDistributionChart, PercentileChart } from '@/components/stats'
import LineChart from '@/components/charts/LineChart.vue'
import { adminApi, type ErrorDistributionResponse, type PercentileItem } from '@/api/admin'
import { dashboardApi, type ProviderStatus } from '@/api/dashboard'
import { getDateRangeFromPeriod } from '@/features/usage/composables'
import type { DateRangeParams } from '@/features/usage/types'

const timeRange = ref<DateRangeParams>(getDateRangeFromPeriod('last30days'))

const percentiles = ref<PercentileItem[]>([])
const percentileLoading = ref(false)

const errorDistribution = ref<ErrorDistributionResponse['distribution']>([])
const errorTrend = ref<ErrorDistributionResponse['trend']>([])
const errorLoading = ref(false)

const providerStatus = ref<ProviderStatus[]>([])
const providerLoading = ref(false)

function buildTimeRangeParams() {
  return {
    start_date: timeRange.value.start_date,
    end_date: timeRange.value.end_date,
    preset: timeRange.value.preset,
    timezone: timeRange.value.timezone,
    tz_offset_minutes: timeRange.value.tz_offset_minutes
  }
}

async function loadPercentiles() {
  percentileLoading.value = true
  try {
    percentiles.value = await adminApi.getPercentiles(buildTimeRangeParams())
  } finally {
    percentileLoading.value = false
  }
}

async function loadErrors() {
  errorLoading.value = true
  try {
    const response = await adminApi.getErrorDistribution(buildTimeRangeParams())
    errorDistribution.value = response.distribution
    errorTrend.value = response.trend
  } finally {
    errorLoading.value = false
  }
}

async function loadProviders() {
  providerLoading.value = true
  try {
    providerStatus.value = await dashboardApi.getProviderStatus()
  } finally {
    providerLoading.value = false
  }
}

const errorTrendChartData = computed(() => ({
  labels: errorTrend.value.map(item => item.date),
  datasets: [
    {
      label: '错误数',
      data: errorTrend.value.map(item => item.total),
      borderColor: 'rgb(239, 68, 68)',
      tension: 0.25,
      pointRadius: 2
    }
  ]
}))

async function loadAll() {
  await Promise.all([loadPercentiles(), loadErrors(), loadProviders()])
}

watch(timeRange, loadAll, { deep: true })

onMounted(loadAll)
</script>

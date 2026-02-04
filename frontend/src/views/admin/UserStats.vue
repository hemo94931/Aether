<template>
  <div class="space-y-6 px-4 sm:px-6 lg:px-0">
    <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3">
      <div>
        <h1 class="text-lg font-semibold">
          用户统计
        </h1>
        <p class="text-xs text-muted-foreground">
          查看用户排行榜与使用趋势
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <TimeRangePicker
          v-model="timeRange"
          :allow-hourly="true"
        />
        <Select
          v-model:open="userSelectOpen"
          v-model="selectedUserId"
        >
          <SelectTrigger class="h-8 text-xs w-52">
            <SelectValue placeholder="选择用户" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="user in users"
              :key="user.id"
              :value="user.id"
            >
              {{ user.username || user.email }}
            </SelectItem>
          </SelectContent>
        </Select>
        <Select
          v-model:open="compareUserSelectOpen"
          v-model="compareUserId"
        >
          <SelectTrigger class="h-8 text-xs w-52">
            <SelectValue placeholder="对比用户（可选）" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__none__">
              不对比
            </SelectItem>
            <SelectItem
              v-for="user in users"
              :key="`compare-${user.id}`"
              :value="user.id"
            >
              {{ user.username || user.email }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-4">
      <LeaderboardTable
        title="用户排行榜"
        :items="leaderboard"
        :metric="metric"
        :loading="leaderboardLoading"
        @update:metric="metric = $event"
      />

      <Card class="p-4 space-y-3">
        <h3 class="text-sm font-semibold">
          用户摘要
        </h3>
        <div
          v-if="summaryLoading"
          class="p-6"
        >
          <LoadingState />
        </div>
        <div
          v-else
          class="grid grid-cols-2 gap-3 text-sm"
        >
          <div>
            <div class="text-xs text-muted-foreground">
              请求数
            </div>
            <div class="font-semibold">
              {{ userSummary?.total_requests ?? 0 }}
            </div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">
              Tokens
            </div>
            <div class="font-semibold">
              {{ formatTokens(userSummary?.total_tokens ?? 0) }}
            </div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">
              成本
            </div>
            <div class="font-semibold">
              {{ formatCurrency(userSummary?.total_cost ?? 0) }}
            </div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">
              错误率
            </div>
            <div class="font-semibold">
              {{ userSummary?.error_rate ?? 0 }}%
            </div>
          </div>
        </div>
      </Card>
    </div>

    <Card class="p-4 space-y-4">
      <h3 class="text-sm font-semibold">
        用户使用趋势
      </h3>
      <div
        v-if="seriesLoading"
        class="p-6"
      >
        <LoadingState />
      </div>
      <div
        v-else
        class="h-[280px]"
      >
        <LineChart :data="seriesChartData" />
      </div>
    </Card>

    <Card
      v-if="comparisonSeries.length > 0"
      class="p-4 space-y-4"
    >
      <h3 class="text-sm font-semibold">
        用户对比趋势
      </h3>
      <div class="h-[280px]">
        <LineChart :data="comparisonChartData" />
      </div>
    </Card>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { Card, Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui'
import LineChart from '@/components/charts/LineChart.vue'
import { LoadingState, TimeRangePicker } from '@/components/common'
import { LeaderboardTable } from '@/components/stats'
import { adminApi, type LeaderboardItem } from '@/api/admin'
import { usersApi, type User } from '@/api/users'
import { usageApi } from '@/api/usage'
import { formatCurrency, formatTokens } from '@/utils/format'
import { getDateRangeFromPeriod } from '@/features/usage/composables'
import type { DateRangeParams } from '@/features/usage/types'

const timeRange = ref<DateRangeParams>(getDateRangeFromPeriod('last7days'))
const metric = ref<'requests' | 'tokens' | 'cost'>('requests')

const users = ref<User[]>([])
const selectedUserId = ref<string | null>(null)
const compareUserId = ref<string>('__none__')
const userSelectOpen = ref(false)
const compareUserSelectOpen = ref(false)

const leaderboard = ref<LeaderboardItem[]>([])
const leaderboardLoading = ref(false)

const userSummary = ref<any | null>(null)
const summaryLoading = ref(false)

const series = ref<any[]>([])
const comparisonSeries = ref<any[]>([])
const seriesLoading = ref(false)

function buildTimeRangeParams() {
  return {
    start_date: timeRange.value.start_date,
    end_date: timeRange.value.end_date,
    preset: timeRange.value.preset,
    timezone: timeRange.value.timezone,
    tz_offset_minutes: timeRange.value.tz_offset_minutes,
    granularity: timeRange.value.granularity || 'day'
  }
}

async function loadUsers() {
  users.value = await usersApi.getAllUsers()
  if (!selectedUserId.value && users.value.length > 0) {
    selectedUserId.value = users.value[0].id
  }
}

async function loadLeaderboard() {
  leaderboardLoading.value = true
  try {
    const response = await adminApi.getLeaderboardUsers({
      ...buildTimeRangeParams(),
      metric: metric.value,
      limit: 10
    })
    leaderboard.value = response.items
  } finally {
    leaderboardLoading.value = false
  }
}

async function loadSummary() {
  if (!selectedUserId.value) return
  summaryLoading.value = true
  try {
    userSummary.value = await usageApi.getUsageStats({
      ...buildTimeRangeParams(),
      user_id: selectedUserId.value
    })
  } finally {
    summaryLoading.value = false
  }
}

async function loadSeries() {
  if (!selectedUserId.value) return
  seriesLoading.value = true
  try {
    series.value = await adminApi.getTimeSeries({
      ...buildTimeRangeParams(),
      user_id: selectedUserId.value
    })

    comparisonSeries.value = []
    if (compareUserId.value && compareUserId.value !== '__none__') {
      comparisonSeries.value = await adminApi.getTimeSeries({
        ...buildTimeRangeParams(),
        user_id: compareUserId.value
      })
    }
  } finally {
    seriesLoading.value = false
  }
}

const seriesChartData = computed(() => ({
  labels: series.value.map(item => item.date),
  datasets: [
    {
      label: '成本',
      data: series.value.map(item => item.total_cost),
      borderColor: 'rgb(59, 130, 246)',
      tension: 0.25,
      pointRadius: 2
    }
  ]
}))

const comparisonChartData = computed(() => ({
  labels: series.value.map(item => item.date),
  datasets: [
    {
      label: '当前用户',
      data: series.value.map(item => item.total_cost),
      borderColor: 'rgb(59, 130, 246)',
      tension: 0.25,
      pointRadius: 2
    },
    {
      label: '对比用户',
      data: comparisonSeries.value.map(item => item.total_cost),
      borderColor: 'rgb(234, 179, 8)',
      tension: 0.25,
      pointRadius: 2
    }
  ]
}))

watch([timeRange, metric], loadLeaderboard, { deep: true })
watch([timeRange, selectedUserId, compareUserId], () => {
  loadSummary()
  loadSeries()
}, { deep: true })

onMounted(async () => {
  await loadUsers()
  await loadLeaderboard()
  await loadSummary()
  await loadSeries()
})
</script>

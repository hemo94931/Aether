<template>
  <div class="space-y-3">
    <div class="flex items-center justify-between">
      <h3 class="text-sm font-semibold">
        {{ title }}
      </h3>
      <span
        v-if="subtitle"
        class="text-xs text-muted-foreground"
      >{{ subtitle }}</span>
    </div>
    <div
      v-if="loading"
      class="p-6"
    >
      <LoadingState />
    </div>
    <div
      v-else
      class="h-[260px]"
    >
      <DoughnutChart
        :data="chartData"
        :options="chartOptions"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import DoughnutChart from '@/components/charts/DoughnutChart.vue'
import { LoadingState } from '@/components/common'
import type { ErrorDistributionItem } from '@/api/admin'

interface Props {
  title: string
  subtitle?: string
  distribution: ErrorDistributionItem[]
  loading?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  loading: false
})

const chartData = computed(() => ({
  labels: props.distribution.map(item => item.category),
  datasets: [
    {
      data: props.distribution.map(item => item.count),
      backgroundColor: [
        'rgba(239, 68, 68, 0.7)',
        'rgba(59, 130, 246, 0.7)',
        'rgba(234, 179, 8, 0.7)',
        'rgba(34, 197, 94, 0.7)',
        'rgba(148, 163, 184, 0.7)'
      ],
      borderWidth: 0
    }
  ]
}))

const chartOptions = computed(() => ({
  plugins: {
    legend: {
      position: 'bottom' as const
    }
  }
}))
</script>

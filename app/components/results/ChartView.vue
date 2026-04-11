<script setup lang="ts">
import { Button } from '~/components/ui/button'
import type { ChartConfig, QueryResult } from '~/types/query'

const props = defineProps<{
  chartConfig: ChartConfig | null
  result: QueryResult | null
  naturalQuery: string
  engine: string
  isGenerating?: boolean
}>()

const emit = defineEmits<{
  generate: []
}>()

const chartData = computed(() => {
  if (!props.result || !props.chartConfig) return []
  const { x_axis, y_axis } = props.chartConfig

  return props.result.rows.map(row => ({
    label: String(row[x_axis.field] ?? ''),
    value: Number(row[y_axis.field] ?? 0),
  }))
})

const maxValue = computed(() => {
  if (chartData.value.length === 0) return 1
  return Math.max(...chartData.value.map(d => d.value))
})
</script>

<template>
  <div class="flex flex-col gap-4 h-full">
    <!-- No chart yet -->
    <div v-if="!chartConfig && !isGenerating" class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground gap-3">
      <Icon name="lucide:bar-chart-3" class="size-8" />
      <div class="text-center">
        <p class="text-sm font-medium text-foreground">Visualize your data</p>
        <p class="text-xs mt-1">AI will pick the best chart type for your results</p>
      </div>
      <Button :disabled="!result || isGenerating" @click="emit('generate')">
        <Icon name="lucide:sparkles" class="size-4" />
        Generate Chart
      </Button>
    </div>

    <!-- Loading -->
    <div v-else-if="isGenerating" class="flex flex-col items-center justify-center flex-1 py-12 gap-3 text-muted-foreground">
      <Icon name="lucide:loader-2" class="size-8 animate-spin" />
      <p class="text-sm">AI is analyzing your data...</p>
    </div>

    <!-- Chart -->
    <div v-else-if="chartConfig && chartData.length > 0" class="flex flex-col gap-3 flex-1">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="font-medium text-sm">{{ chartConfig.title }}</h3>
          <p class="text-xs text-muted-foreground mt-0.5">{{ chartConfig.explanation }}</p>
        </div>
        <Button size="sm" variant="outline" @click="emit('generate')">
          <Icon name="lucide:refresh-cw" class="size-3.5" />
          Regenerate
        </Button>
      </div>

      <!-- Simple bar chart using CSS/SVG for now (Unovis can be added later) -->
      <div class="flex-1 rounded-md border border-border p-4 overflow-auto">
        <div
          v-if="chartConfig.chart_type === 'bar' || chartConfig.chart_type === 'area' || chartConfig.chart_type === 'line'"
          class="h-full flex items-end gap-1.5 min-h-[200px]"
        >
          <div
            v-for="(item, i) in chartData.slice(0, 30)"
            :key="i"
            class="flex-1 flex flex-col items-center gap-1 min-w-8"
          >
            <span class="text-xs text-muted-foreground">{{ item.value.toLocaleString() }}</span>
            <div
              class="w-full bg-primary/80 hover:bg-primary transition-all rounded-sm"
              :style="{ height: `${Math.max(4, (item.value / maxValue) * 180)}px` }"
              :title="`${item.label}: ${item.value}`"
            />
            <span class="text-xs text-muted-foreground truncate max-w-full" :title="item.label">
              {{ item.label }}
            </span>
          </div>
        </div>

        <!-- Pie chart (simple donut) -->
        <div v-else-if="chartConfig.chart_type === 'pie'" class="flex items-center gap-8 h-full">
          <svg viewBox="0 0 100 100" class="size-48 shrink-0">
            <circle cx="50" cy="50" r="40" fill="none" stroke="var(--border)" stroke-width="2" />
            <template v-for="(item, i) in chartData.slice(0, 8)" :key="i">
              <!-- simplified pie segments using hsl colors -->
            </template>
          </svg>
          <div class="flex flex-col gap-2">
            <div v-for="(item, i) in chartData.slice(0, 8)" :key="i" class="flex items-center gap-2 text-sm">
              <div class="size-3 rounded-sm" :style="{ backgroundColor: `hsl(${(i * 45) % 360}, 70%, 60%)` }" />
              <span class="text-muted-foreground">{{ item.label }}</span>
              <span class="font-medium ml-auto pl-4">{{ item.value.toLocaleString() }}</span>
            </div>
          </div>
        </div>

        <!-- Fallback table view -->
        <div v-else class="overflow-auto">
          <table class="w-full text-sm">
            <thead>
              <tr class="border-b border-border">
                <th class="pb-2 text-left text-muted-foreground">{{ chartConfig.x_axis.label }}</th>
                <th class="pb-2 text-right text-muted-foreground">{{ chartConfig.y_axis.label }}</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-border">
              <tr v-for="(item, i) in chartData" :key="i">
                <td class="py-1.5">{{ item.label }}</td>
                <td class="py-1.5 text-right font-mono">{{ item.value.toLocaleString() }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  </div>
</template>

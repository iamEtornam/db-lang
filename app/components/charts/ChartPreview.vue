<script setup lang="ts">
// Local SVG/HTML chart renderer (issue #10). Mirrors the dependency-free
// rendering approach in components/results/ChartView.vue but draws from
// pre-computed ChartPoint[] so the charts page can drive it directly.
import type { ChartPoint } from '~/lib/chart'
import type { ChartType } from '~/types/database'

const props = defineProps<{
  points: ChartPoint[]
  chartType: ChartType
  categoryLabel?: string
  valueLabel?: string
}>()

const visible = computed(() => props.points.slice(0, 50))

const maxValue = computed(() =>
  visible.value.length ? Math.max(...visible.value.map(d => d.value), 0) || 1 : 1,
)

const total = computed(() => props.points.reduce((a, d) => a + d.value, 0))

function color(i: number) {
  return `hsl(${(i * 47) % 360}, 65%, 58%)`
}

// --- Line / area path geometry (viewBox 0..100 x, 0..40 y) ---
const linePath = computed(() => {
  const pts = visible.value
  if (pts.length < 2) return ''
  const stepX = 100 / (pts.length - 1)
  return pts
    .map((p, i) => {
      const x = i * stepX
      const y = 40 - (p.value / maxValue.value) * 38
      return `${i === 0 ? 'M' : 'L'}${x.toFixed(2)},${y.toFixed(2)}`
    })
    .join(' ')
})

const areaPath = computed(() => {
  if (!linePath.value) return ''
  return `${linePath.value} L100,40 L0,40 Z`
})

// --- Pie geometry ---
const pieSegments = computed(() => {
  const slice = props.points.slice(0, 10)
  const sum = slice.reduce((a, d) => a + Math.max(0, d.value), 0) || 1
  let angle = -Math.PI / 2
  return slice.map((d, i) => {
    const frac = Math.max(0, d.value) / sum
    const start = angle
    const end = angle + frac * Math.PI * 2
    angle = end
    const large = end - start > Math.PI ? 1 : 0
    const x1 = 50 + 40 * Math.cos(start)
    const y1 = 50 + 40 * Math.sin(start)
    const x2 = 50 + 40 * Math.cos(end)
    const y2 = 50 + 40 * Math.sin(end)
    return {
      d: `M50,50 L${x1.toFixed(2)},${y1.toFixed(2)} A40,40 0 ${large} 1 ${x2.toFixed(2)},${y2.toFixed(2)} Z`,
      color: color(i),
      label: d.label,
      value: d.value,
    }
  })
})

// --- Scatter geometry ---
const scatterPoints = computed(() =>
  visible.value.map((p, i) => ({
    cx: visible.value.length > 1 ? (i / (visible.value.length - 1)) * 100 : 50,
    cy: 40 - (p.value / maxValue.value) * 38,
    label: p.label,
    value: p.value,
  })),
)

// --- KPI (single value: sum of all points) ---
const kpiValue = computed(() => total.value)
</script>

<template>
  <div class="h-full w-full overflow-auto">
    <div v-if="points.length === 0" class="flex h-full items-center justify-center text-sm text-muted-foreground">
      No data to plot. Run a query and map your columns.
    </div>

    <!-- Bar -->
    <div
      v-else-if="chartType === 'bar'"
      class="flex h-full min-h-[220px] items-end gap-1.5"
    >
      <div
        v-for="(item, i) in visible"
        :key="i"
        class="flex min-w-8 flex-1 flex-col items-center gap-1"
      >
        <span class="text-xs text-muted-foreground">{{ item.value.toLocaleString() }}</span>
        <div
          class="w-full rounded-sm bg-primary/80 transition-all hover:bg-primary"
          :style="{ height: `${Math.max(4, (item.value / maxValue) * 180)}px` }"
          :title="`${item.label}: ${item.value}`"
        />
        <span class="max-w-full truncate text-xs text-muted-foreground" :title="item.label">
          {{ item.label }}
        </span>
      </div>
    </div>

    <!-- Line / Area -->
    <div v-else-if="chartType === 'line' || chartType === 'area'" class="flex h-full flex-col gap-2">
      <svg viewBox="0 0 100 40" preserveAspectRatio="none" class="min-h-[200px] w-full flex-1">
        <path
          v-if="chartType === 'area'"
          :d="areaPath"
          fill="var(--primary)"
          fill-opacity="0.18"
        />
        <path
          :d="linePath"
          fill="none"
          stroke="var(--primary)"
          stroke-width="0.7"
          vector-effect="non-scaling-stroke"
        />
      </svg>
      <div class="flex justify-between text-xs text-muted-foreground">
        <span class="truncate">{{ visible[0]?.label }}</span>
        <span class="truncate">{{ visible[visible.length - 1]?.label }}</span>
      </div>
    </div>

    <!-- Pie -->
    <div v-else-if="chartType === 'pie'" class="flex h-full items-center gap-8">
      <svg viewBox="0 0 100 100" class="size-48 shrink-0">
        <path v-for="(seg, i) in pieSegments" :key="i" :d="seg.d" :fill="seg.color">
          <title>{{ seg.label }}: {{ seg.value }}</title>
        </path>
      </svg>
      <div class="flex flex-col gap-2 overflow-auto">
        <div v-for="(seg, i) in pieSegments" :key="i" class="flex items-center gap-2 text-sm">
          <div class="size-3 rounded-sm" :style="{ backgroundColor: seg.color }" />
          <span class="text-muted-foreground">{{ seg.label }}</span>
          <span class="ml-auto pl-4 font-medium">{{ seg.value.toLocaleString() }}</span>
        </div>
      </div>
    </div>

    <!-- Scatter -->
    <div v-else-if="chartType === 'scatter'" class="flex h-full flex-col gap-2">
      <svg viewBox="0 0 100 40" preserveAspectRatio="none" class="min-h-[200px] w-full flex-1">
        <circle
          v-for="(p, i) in scatterPoints"
          :key="i"
          :cx="p.cx"
          :cy="p.cy"
          r="0.8"
          fill="var(--primary)"
          vector-effect="non-scaling-stroke"
        >
          <title>{{ p.label }}: {{ p.value }}</title>
        </circle>
      </svg>
    </div>

    <!-- KPI / single value -->
    <div v-else-if="chartType === 'kpi'" class="flex h-full flex-col items-center justify-center gap-2">
      <span class="text-5xl font-semibold tabular-nums">{{ kpiValue.toLocaleString() }}</span>
      <span class="text-sm text-muted-foreground">{{ valueLabel || 'Total' }}</span>
    </div>

    <!-- Table of aggregates -->
    <div v-else class="overflow-auto">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b border-border">
            <th class="pb-2 text-left text-muted-foreground">{{ categoryLabel || 'Category' }}</th>
            <th class="pb-2 text-right text-muted-foreground">{{ valueLabel || 'Value' }}</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-border">
          <tr v-for="(item, i) in points" :key="i">
            <td class="py-1.5">{{ item.label }}</td>
            <td class="py-1.5 text-right font-mono">{{ item.value.toLocaleString() }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

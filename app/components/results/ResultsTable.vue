<script setup lang="ts">
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Badge } from '~/components/ui/badge'
import type { QueryResult } from '~/types/query'

const props = defineProps<{
  result: QueryResult | null
  isLoading?: boolean
}>()

const emit = defineEmits<{
  'load-page': [page: number]
}>()

const searchTerm = ref('')

const filteredRows = computed(() => {
  if (!props.result) return []
  if (!searchTerm.value) return props.result.rows

  const term = searchTerm.value.toLowerCase()
  return props.result.rows.filter(row =>
    Object.values(row).some(val =>
      String(val).toLowerCase().includes(term),
    ),
  )
})

function formatCellValue(val: unknown): string {
  if (val === null || val === undefined) return ''
  if (typeof val === 'object') return JSON.stringify(val)
  return String(val)
}

function isTruncated(val: string): boolean {
  return val.length > 100
}

function copyCell(val: string) {
  navigator.clipboard.writeText(val)
}
</script>

<template>
  <div class="flex flex-col gap-3 h-full">
    <!-- Toolbar -->
    <div v-if="result" class="flex items-center gap-3">
      <div class="relative flex-1 max-w-xs">
        <Icon name="lucide:search" class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input
          v-model="searchTerm"
          placeholder="Filter results..."
          class="pl-8 h-8 text-sm"
        />
      </div>
      <div class="flex items-center gap-2 text-sm text-muted-foreground ml-auto">
        <span v-if="result.total_count !== null">
          {{ result.total_count.toLocaleString() }} rows
        </span>
        <Badge variant="outline" class="text-xs">
          {{ result.execution_time_ms }}ms
        </Badge>
        <Badge variant="outline" class="text-xs">
          {{ result.columns.length }} cols
        </Badge>
      </div>
    </div>

    <!-- Loading state -->
    <div v-if="isLoading" class="flex flex-col gap-2 flex-1">
      <div v-for="i in 5" :key="i" class="h-10 bg-muted/50 rounded animate-pulse" />
    </div>

    <!-- Table -->
    <div v-else-if="result && filteredRows.length > 0" class="flex flex-col flex-1 overflow-hidden rounded-md border border-border">
      <div class="overflow-auto flex-1">
        <table class="w-full text-sm">
          <thead class="sticky top-0 bg-muted/80 backdrop-blur-sm border-b border-border">
            <tr>
              <th
                v-for="col in result.columns"
                :key="col"
                class="px-3 py-2 text-left font-medium text-muted-foreground whitespace-nowrap"
              >
                {{ col }}
              </th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            <tr
              v-for="(row, ri) in filteredRows"
              :key="ri"
              class="hover:bg-muted/30 transition-colors"
            >
              <td
                v-for="col in result.columns"
                :key="col"
                class="px-3 py-2 text-foreground"
              >
                <template v-if="row[col] === null">
                  <span class="text-muted-foreground italic text-xs">null</span>
                </template>
                <template v-else>
                  <span
                    v-if="isTruncated(formatCellValue(row[col]))"
                    class="cursor-pointer"
                    :title="formatCellValue(row[col])"
                    @dblclick="copyCell(formatCellValue(row[col]))"
                  >
                    {{ formatCellValue(row[col]).slice(0, 100) }}<span class="text-muted-foreground">…</span>
                  </span>
                  <span v-else>{{ formatCellValue(row[col]) }}</span>
                </template>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Pagination -->
      <div v-if="result.total_count && result.total_count > result.page_size" class="flex items-center justify-between border-t border-border px-3 py-2 text-sm text-muted-foreground">
        <span>
          Page {{ result.page }} · Showing {{ Math.min(result.page * result.page_size, result.total_count) }} of {{ result.total_count.toLocaleString() }}
        </span>
        <div class="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            class="h-7 px-2"
            :disabled="result.page <= 1"
            @click="emit('load-page', result.page - 1)"
          >
            <Icon name="lucide:chevron-left" class="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            class="h-7 px-2"
            :disabled="!result.has_more"
            @click="emit('load-page', result.page + 1)"
          >
            <Icon name="lucide:chevron-right" class="size-4" />
          </Button>
        </div>
      </div>
    </div>

    <!-- Empty state -->
    <div v-else-if="result && filteredRows.length === 0" class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground">
      <Icon name="lucide:table" class="size-8 mb-3" />
      <p class="text-sm">{{ searchTerm ? 'No rows match your filter' : 'Query returned no results' }}</p>
    </div>

    <!-- Pre-query state -->
    <div v-else-if="!result && !isLoading" class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground">
      <Icon name="lucide:table-2" class="size-8 mb-3" />
      <p class="text-sm">Run a query to see results</p>
    </div>
  </div>
</template>

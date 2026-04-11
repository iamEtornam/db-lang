<script setup lang="ts">
import { Badge } from '~/components/ui/badge'
import type { QueryExplanation } from '~/types/query'

defineProps<{
  explanation: QueryExplanation
  sql: string
  dbType?: string
}>()
</script>

<template>
  <div class="rounded-md border border-border bg-card p-4 space-y-3 text-sm">
    <div class="flex items-center gap-2 font-medium">
      <Icon name="lucide:lightbulb" class="size-4 text-yellow-500" />
      Query Analysis
    </div>

    <p class="text-muted-foreground">{{ explanation.summary }}</p>

    <div v-if="explanation.tables_involved.length > 0" class="flex items-center gap-1.5 flex-wrap">
      <span class="text-xs text-muted-foreground">Tables:</span>
      <Badge v-for="t in explanation.tables_involved" :key="t" variant="secondary" class="text-xs">
        {{ t }}
      </Badge>
    </div>

    <div v-if="explanation.potential_issues.length > 0" class="space-y-1">
      <p class="text-xs font-medium text-destructive">Potential Issues</p>
      <ul class="list-disc list-inside text-xs text-muted-foreground space-y-0.5">
        <li v-for="issue in explanation.potential_issues" :key="issue">{{ issue }}</li>
      </ul>
    </div>

    <div v-if="explanation.optimization_tips.length > 0" class="space-y-1">
      <p class="text-xs font-medium text-foreground">Optimization Tips</p>
      <ul class="list-disc list-inside text-xs text-muted-foreground space-y-0.5">
        <li v-for="tip in explanation.optimization_tips" :key="tip">{{ tip }}</li>
      </ul>
    </div>
  </div>
</template>

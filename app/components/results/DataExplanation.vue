<script setup lang="ts">
import { Button } from '~/components/ui/button'

const props = defineProps<{
  explanation: string
  isLoading?: boolean
  hasData: boolean
}>()

const emit = defineEmits<{
  generate: []
}>()
</script>

<template>
  <div class="flex flex-col gap-4 h-full">
    <div v-if="!explanation && !isLoading" class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground gap-3">
      <Icon name="lucide:message-square-text" class="size-8" />
      <div class="text-center">
        <p class="text-sm font-medium text-foreground">AI Data Insights</p>
        <p class="text-xs mt-1">Get a plain-English explanation of your query results</p>
      </div>
      <Button :disabled="!hasData || isLoading" @click="emit('generate')">
        <Icon name="lucide:sparkles" class="size-4" />
        Explain Data
      </Button>
    </div>

    <div v-else-if="isLoading" class="flex flex-col items-center justify-center flex-1 py-12 gap-3 text-muted-foreground">
      <Icon name="lucide:loader-2" class="size-8 animate-spin" />
      <p class="text-sm">AI is analyzing your results...</p>
    </div>

    <div v-else-if="explanation" class="flex flex-col gap-3 flex-1">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-medium flex items-center gap-2">
          <Icon name="lucide:sparkles" class="size-4 text-primary" />
          AI Analysis
        </h3>
        <Button size="sm" variant="ghost" @click="emit('generate')">
          <Icon name="lucide:refresh-cw" class="size-3.5" />
          Refresh
        </Button>
      </div>

      <div class="flex-1 rounded-md border border-border bg-muted/20 p-4">
        <p class="text-sm text-foreground leading-relaxed whitespace-pre-wrap">{{ explanation }}</p>
      </div>
    </div>
  </div>
</template>

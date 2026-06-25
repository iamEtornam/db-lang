<script setup lang="ts">
import { Button } from '~/components/ui/button'
import { Textarea } from '~/components/ui/textarea'
import type { ResultExplanation } from '~/types/database'

const props = defineProps<{
  explanation: ResultExplanation | null
  isLoading?: boolean
  hasData: boolean
}>()

const emit = defineEmits<{
  /** Generate the first explanation (no follow-up question). */
  generate: []
  /** Re-generate, bypassing the cache (Refresh button). */
  refresh: []
  /** Ask a follow-up question about the existing result. */
  ask: [question: string]
  /** Populate the query input from a suggested follow-up. */
  useFollowup: [followup: string]
}>()

const question = ref('')

function submitQuestion() {
  const q = question.value.trim()
  if (!q || props.isLoading)
    return
  emit('ask', q)
  question.value = ''
}

function onQuestionKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    submitQuestion()
  }
}
</script>

<template>
  <div class="flex flex-col gap-3 h-full">
    <!-- Empty state -->
    <div
      v-if="!explanation && !isLoading"
      class="flex flex-col items-center justify-center flex-1 py-12 text-muted-foreground gap-3"
    >
      <Icon name="lucide:message-square-text" class="size-8" />
      <div class="text-center">
        <p class="text-sm font-medium text-foreground">
          AI Result Insights
        </p>
        <p class="text-xs mt-1">
          Get a structured interpretation of your query results
        </p>
      </div>
      <Button :disabled="!hasData || isLoading" @click="emit('generate')">
        <Icon name="lucide:sparkles" class="size-4" />
        Explain result
      </Button>
    </div>

    <!-- Loading -->
    <div
      v-else-if="isLoading && !explanation"
      class="flex flex-col items-center justify-center flex-1 py-12 gap-3 text-muted-foreground"
    >
      <Icon name="lucide:loader-2" class="size-8 animate-spin text-primary" />
      <p class="text-sm">
        AI is analyzing your results...
      </p>
    </div>

    <!-- Result -->
    <div v-else-if="explanation" class="flex flex-col gap-4 flex-1 min-h-0 overflow-auto p-1">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-medium flex items-center gap-2">
          <Icon name="lucide:sparkles" class="size-4 text-primary" />
          AI Analysis
        </h3>
        <Button size="sm" variant="ghost" :disabled="isLoading" @click="emit('refresh')">
          <Icon name="lucide:refresh-cw" class="size-3.5" />
          Refresh
        </Button>
      </div>

      <!-- Summary -->
      <div class="rounded-md border border-border bg-muted/20 p-4">
        <p class="text-sm text-foreground leading-relaxed whitespace-pre-wrap">
          {{ explanation.summary }}
        </p>
      </div>

      <!-- Key findings -->
      <div v-if="explanation.key_findings.length" class="space-y-1.5">
        <h4 class="text-xs font-semibold text-muted-foreground uppercase tracking-wide flex items-center gap-1.5">
          <Icon name="lucide:list-checks" class="size-3.5" />
          Key findings
        </h4>
        <ul class="space-y-1">
          <li
            v-for="(f, i) in explanation.key_findings"
            :key="i"
            class="text-sm flex items-start gap-2"
          >
            <Icon name="lucide:check" class="size-3.5 mt-0.5 shrink-0 text-green-500" />
            <span>{{ f }}</span>
          </li>
        </ul>
      </div>

      <!-- Anomalies -->
      <div v-if="explanation.anomalies.length" class="space-y-1.5">
        <h4 class="text-xs font-semibold text-yellow-600 dark:text-yellow-400 uppercase tracking-wide flex items-center gap-1.5">
          <Icon name="lucide:alert-triangle" class="size-3.5" />
          Anomalies
        </h4>
        <ul class="space-y-1">
          <li
            v-for="(a, i) in explanation.anomalies"
            :key="i"
            class="text-sm flex items-start gap-2 rounded-md bg-yellow-500/10 border border-yellow-500/20 px-2.5 py-1.5"
          >
            <Icon name="lucide:flag" class="size-3.5 mt-0.5 shrink-0 text-yellow-600 dark:text-yellow-400" />
            <span>{{ a }}</span>
          </li>
        </ul>
      </div>

      <!-- Suggested follow-ups (one-click → populate query input) -->
      <div v-if="explanation.suggested_followups.length" class="space-y-1.5">
        <h4 class="text-xs font-semibold text-muted-foreground uppercase tracking-wide flex items-center gap-1.5">
          <Icon name="lucide:lightbulb" class="size-3.5" />
          Suggested follow-ups
        </h4>
        <div class="flex flex-col gap-1.5">
          <button
            v-for="(s, i) in explanation.suggested_followups"
            :key="i"
            type="button"
            class="text-left text-sm rounded-md border border-border px-3 py-2 hover:bg-muted/40 transition-colors flex items-center gap-2 group"
            @click="emit('useFollowup', s)"
          >
            <Icon name="lucide:corner-down-right" class="size-3.5 shrink-0 text-muted-foreground group-hover:text-primary" />
            <span class="flex-1">{{ s }}</span>
            <Icon name="lucide:arrow-up-left" class="size-3.5 shrink-0 opacity-0 group-hover:opacity-100 text-muted-foreground" />
          </button>
        </div>
      </div>

      <!-- Chat-style follow-up box (does NOT re-run the query) -->
      <div class="mt-auto pt-2 border-t border-border space-y-2">
        <label class="text-xs font-medium text-muted-foreground">Ask about this result</label>
        <div class="relative">
          <Textarea
            v-model="question"
            placeholder="e.g. Why are some values missing? What's the highest one?"
            class="min-h-[56px] resize-none pr-12 text-sm"
            :disabled="isLoading"
            @keydown="onQuestionKeydown"
          />
          <Button
            size="sm"
            class="absolute bottom-2 right-2 h-7 w-7 p-0"
            :disabled="!question.trim() || isLoading"
            @click="submitQuestion"
          >
            <Icon v-if="isLoading" name="lucide:loader-2" class="size-3.5 animate-spin" />
            <Icon v-else name="lucide:send" class="size-3.5" />
          </Button>
        </div>
      </div>
    </div>
  </div>
</template>

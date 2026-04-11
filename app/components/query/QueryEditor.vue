<script setup lang="ts">
import { toast } from 'vue-sonner'
import { Button } from '~/components/ui/button'
import { Textarea } from '~/components/ui/textarea'
import { Badge } from '~/components/ui/badge'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '~/components/ui/collapsible'
import { Separator } from '~/components/ui/separator'

const props = defineProps<{
  naturalQuery: string
  generatedQuery: string
  queryLanguage: string
  tablesUsed: string[]
  explanation: string
  confidence: number
  isTranslating: boolean
  isExecuting: boolean
  hasConnection: boolean
}>()

const emit = defineEmits<{
  'update:naturalQuery': [value: string]
  'update:generatedQuery': [value: string]
  translate: []
  execute: []
  explain: []
}>()

const naturalQueryModel = useVModel(props, 'naturalQuery', emit)
const generatedQueryModel = useVModel(props, 'generatedQuery', emit)

const showContext = ref(false)

function onKeyDown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
    e.preventDefault()
    emit('execute')
  }
}

const confidenceColor = computed(() => {
  if (props.confidence >= 0.8) return 'text-green-500'
  if (props.confidence >= 0.5) return 'text-yellow-500'
  return 'text-red-500'
})

const queryLanguageLabel = computed(() => {
  switch (props.queryLanguage) {
    case 'sql': return 'SQL'
    case 'mql': return 'MongoDB'
    case 'redis': return 'Redis'
    default: return 'Query'
  }
})

function copyQuery() {
  navigator.clipboard.writeText(props.generatedQuery)
  toast.success('Copied to clipboard')
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <!-- Natural language input -->
    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <label class="text-sm font-medium text-foreground">Ask in plain English</label>
        <span class="text-xs text-muted-foreground">
          {{ hasConnection ? 'Connected' : 'No connection selected' }}
        </span>
      </div>

      <div class="relative">
        <Textarea
          v-model="naturalQueryModel"
          placeholder="e.g. List all students between age 20 and 30 who have paid more than 50% of their fees"
          class="min-h-[80px] resize-none pr-24 font-medium"
          :disabled="!hasConnection"
          @keydown="onKeyDown"
          @keyup.enter.exact="emit('translate')"
        />
        <div class="absolute bottom-2 right-2 flex gap-1.5">
          <Button
            size="sm"
            variant="ghost"
            class="h-7 px-2 text-xs"
            :disabled="!naturalQueryModel || isTranslating || !hasConnection"
            @click="emit('translate')"
          >
            <Icon v-if="isTranslating" name="lucide:loader-2" class="size-3.5 animate-spin" />
            <Icon v-else name="lucide:sparkles" class="size-3.5" />
            <span class="ml-1">Generate</span>
          </Button>
        </div>
      </div>
    </div>

    <!-- Generated query -->
    <div v-if="generatedQuery" class="space-y-2">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <label class="text-sm font-medium">Generated {{ queryLanguageLabel }}</label>
          <Badge variant="outline" class="text-xs">{{ queryLanguageLabel }}</Badge>
          <template v-if="tablesUsed.length > 0">
            <Separator orientation="vertical" class="h-4" />
            <div class="flex items-center gap-1">
              <Badge
                v-for="table in tablesUsed"
                :key="table"
                variant="secondary"
                class="text-xs"
              >
                {{ table }}
              </Badge>
            </div>
          </template>
          <template v-if="confidence > 0">
            <Separator orientation="vertical" class="h-4" />
            <span class="text-xs" :class="confidenceColor">
              {{ Math.round(confidence * 100) }}% confidence
            </span>
          </template>
        </div>
        <div class="flex items-center gap-1">
          <Button size="sm" variant="ghost" class="h-7 px-2 text-xs" @click="copyQuery">
            <Icon name="lucide:copy" class="size-3.5" />
          </Button>
          <Button size="sm" variant="ghost" class="h-7 px-2 text-xs" @click="emit('explain')">
            <Icon name="lucide:lightbulb" class="size-3.5" />
            Explain
          </Button>
        </div>
      </div>

      <Textarea
        v-model="generatedQueryModel"
        class="min-h-[100px] resize-none font-mono text-sm"
        placeholder="Generated query will appear here..."
      />

      <!-- AI explanation inline -->
      <div v-if="explanation" class="rounded-md bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
        <Icon name="lucide:info" class="inline size-3.5 mr-1.5 align-middle" />
        {{ explanation }}
      </div>

      <!-- Execute button -->
      <div class="flex items-center gap-2">
        <Button
          :disabled="!generatedQuery || isExecuting"
          class="gap-2"
          @click="emit('execute')"
        >
          <Icon v-if="isExecuting" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:play" class="size-4" />
          Run Query
          <kbd class="hidden sm:inline-flex ml-1 pointer-events-none h-5 select-none items-center gap-1 rounded border bg-muted/50 px-1.5 font-mono text-[10px] text-muted-foreground">
            ⌘↵
          </kbd>
        </Button>
        <Button variant="ghost" size="sm" @click="generatedQueryModel = ''">
          Clear
        </Button>
      </div>
    </div>

    <!-- No connection state -->
    <div v-if="!hasConnection" class="rounded-lg border border-dashed border-border p-8 text-center">
      <Icon name="lucide:database" class="mx-auto size-8 text-muted-foreground mb-3" />
      <h3 class="font-medium text-foreground mb-1">No connection selected</h3>
      <p class="text-sm text-muted-foreground mb-4">
        Add a database connection from the sidebar to start querying.
      </p>
    </div>
  </div>
</template>

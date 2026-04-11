<script setup lang="ts">
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Badge } from '~/components/ui/badge'
import { useHistoryStore } from '~/stores/history'
import { formatRelativeTime } from '~/lib/utils'

useHead({ title: 'History' })

const historyStore = useHistoryStore()
const { history, isLoading, hasMore } = storeToRefs(historyStore)

const searchTerm = ref('')
const searchResults = ref<typeof history.value>([])
const isSearching = ref(false)

const displayedHistory = computed(() =>
  searchTerm.value ? searchResults.value : history.value,
)

onMounted(() => historyStore.loadHistory(true))

const debouncedSearch = useDebounceFn(async (term: string) => {
  if (!term) {
    searchResults.value = []
    return
  }
  isSearching.value = true
  searchResults.value = await historyStore.searchHistory(term)
  isSearching.value = false
}, 300)

watch(searchTerm, debouncedSearch)

const router = useRouter()
function useHistoryItem(item: { natural_query: string; sql_query: string }) {
  router.push({ path: '/', query: { nl: item.natural_query, sql: item.sql_query } })
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-lg font-semibold">Query History</h1>
        <p class="text-sm text-muted-foreground">Your recent queries</p>
      </div>

      <div class="relative w-64">
        <Icon name="lucide:search" class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input v-model="searchTerm" placeholder="Search history..." class="pl-8" />
      </div>
    </div>

    <!-- Loading -->
    <div v-if="isLoading && history.length === 0" class="flex flex-col gap-2">
      <div v-for="i in 5" :key="i" class="h-20 bg-muted/50 rounded-md animate-pulse" />
    </div>

    <!-- History list -->
    <div v-else-if="displayedHistory.length > 0" class="flex flex-col gap-2">
      <div
        v-for="item in displayedHistory"
        :key="item.id"
        class="group flex flex-col gap-1.5 rounded-lg border border-border bg-card p-3 hover:border-border/80 transition-colors cursor-pointer"
        @click="useHistoryItem(item)"
      >
        <div class="flex items-start justify-between gap-2">
          <p class="text-sm font-medium text-foreground line-clamp-1">
            {{ item.natural_query || 'Direct SQL query' }}
          </p>
          <div class="flex items-center gap-1.5 shrink-0">
            <Badge
              :variant="item.status === 'success' ? 'default' : 'destructive'"
              class="text-xs"
            >
              {{ item.status }}
            </Badge>
            <span class="text-xs text-muted-foreground">{{ formatRelativeTime(item.created_at) }}</span>
          </div>
        </div>
        <p class="text-xs font-mono text-muted-foreground line-clamp-2">{{ item.sql_query }}</p>
        <div class="flex items-center gap-3 text-xs text-muted-foreground">
          <span v-if="item.result_count !== null">
            <Icon name="lucide:rows" class="inline size-3 mr-1" />
            {{ item.result_count.toLocaleString() }} rows
          </span>
          <span v-if="item.execution_time_ms !== null">
            <Icon name="lucide:timer" class="inline size-3 mr-1" />
            {{ item.execution_time_ms }}ms
          </span>
        </div>
      </div>

      <!-- Load more -->
      <Button
        v-if="hasMore && !searchTerm"
        variant="outline"
        class="w-full"
        :disabled="isLoading"
        @click="historyStore.loadHistory()"
      >
        <Icon v-if="isLoading" name="lucide:loader-2" class="size-4 animate-spin" />
        Load more
      </Button>
    </div>

    <!-- Empty state -->
    <div v-else class="flex flex-col items-center justify-center py-16 text-muted-foreground gap-3">
      <Icon name="lucide:clock" class="size-8" />
      <p class="text-sm">{{ searchTerm ? 'No results found' : 'No query history yet' }}</p>
    </div>
  </div>
</template>

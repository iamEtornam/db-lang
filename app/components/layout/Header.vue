<script setup lang="ts">
import { SidebarTrigger } from '~/components/ui/sidebar'
import { Separator } from '~/components/ui/separator'

const route = useRoute()

const breadcrumbs = computed(() => {
  const map: Record<string, string> = {
    '/': 'Query',
    '/schema': 'Schema',
    '/history': 'History',
    '/settings': 'Settings',
  }
  return [{ label: map[route.path] ?? 'Query Studio' }]
})

const showCommandPalette = ref(false)

function onKeyDown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
    e.preventDefault()
    showCommandPalette.value = true
  }
}

onMounted(() => window.addEventListener('keydown', onKeyDown))
onUnmounted(() => window.removeEventListener('keydown', onKeyDown))
</script>

<template>
  <header class="flex h-14 shrink-0 items-center gap-2 border-b border-border px-4">
    <div class="flex flex-1 items-center gap-2">
      <SidebarTrigger class="-ml-1" />
      <Separator orientation="vertical" class="mr-2 h-4" />

      <!-- Breadcrumb -->
      <nav aria-label="breadcrumb">
        <ol class="flex items-center gap-1 text-sm">
          <li
            v-for="(crumb, i) in breadcrumbs"
            :key="i"
            class="text-foreground font-medium"
          >
            {{ crumb.label }}
          </li>
        </ol>
      </nav>
    </div>

    <!-- Right side actions -->
    <div class="flex items-center gap-2">
      <!-- Command palette trigger -->
      <button
        class="hidden md:flex items-center gap-2 px-3 py-1.5 rounded-md border border-border bg-background text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        @click="showCommandPalette = true"
      >
        <Icon name="lucide:search" class="size-3.5" />
        <span>Search...</span>
        <kbd class="ml-2 pointer-events-none inline-flex h-5 select-none items-center gap-1 rounded border bg-muted px-1.5 font-mono text-[10px] font-medium text-muted-foreground">
          <span class="text-xs">⌘</span>K
        </kbd>
      </button>
    </div>

    <!-- Command Palette -->
    <SharedCommandPalette v-model:open="showCommandPalette" />
  </header>
</template>

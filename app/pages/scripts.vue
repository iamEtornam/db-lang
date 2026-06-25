<script setup lang="ts">
import { toast } from 'vue-sonner'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { Badge } from '~/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '~/components/ui/dialog'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '~/components/ui/alert-dialog'
import { useScriptsStore, parseScriptParams } from '~/stores/scripts'
import { useConnectionsStore } from '~/stores/connections'
import type { Script, ScriptParam } from '~/types/database'

useHead({ title: 'Scripts' })

const scriptsStore = useScriptsStore()
const connectionsStore = useConnectionsStore()
const { scripts, isLoading } = storeToRefs(scriptsStore)
const { activeConnection } = storeToRefs(connectionsStore)
const router = useRouter()

const searchTerm = ref('')

// mariadb connections run the mysql library
function normalizeEngine(engine: string): string {
  return engine === 'mariadb' ? 'mysql' : engine
}

const activeEngine = computed(() =>
  activeConnection.value ? normalizeEngine(activeConnection.value.db_type) : null,
)

const filteredScripts = computed(() => {
  const term = searchTerm.value.toLowerCase()
  return scripts.value.filter((s) => {
    // Only show scripts matching the active connection's engine (or all if none).
    if (activeEngine.value && normalizeEngine(s.engine) !== activeEngine.value) return false
    if (!term) return true
    return (
      s.name.toLowerCase().includes(term)
      || (s.description ?? '').toLowerCase().includes(term)
      || s.tags.toLowerCase().includes(term)
    )
  })
})

onMounted(() => scriptsStore.loadScripts())

// ---- Run flow ----
const runDialogOpen = ref(false)
const activeScript = ref<Script | null>(null)
const paramValues = ref<Record<string, string>>({})
const runResults = ref<unknown[] | null>(null)
const isRunning = ref(false)

const destructiveConfirmOpen = ref(false)

const activeParams = computed<ScriptParam[]>(() =>
  activeScript.value ? parseScriptParams(activeScript.value) : [],
)

function openRun(script: Script) {
  if (!activeConnection.value) {
    toast.error('Connect to a database first')
    return
  }
  activeScript.value = script
  runResults.value = null
  paramValues.value = {}
  for (const p of parseScriptParams(script)) {
    paramValues.value[p.name] = p.default ?? ''
  }
  runDialogOpen.value = true
}

async function doRun() {
  const script = activeScript.value
  const conn = activeConnection.value
  if (!script || !conn) return
  isRunning.value = true
  runResults.value = null
  try {
    const rows = await scriptsStore.runScript(conn.id, script.id, paramValues.value)
    runResults.value = rows
    toast.success(`${rows.length} row(s) returned`)
  }
  catch (err) {
    const msg = String(err)
    // Backend gates destructive scripts with the same check as AI queries.
    if (msg.includes('DestructiveQuery')) {
      destructiveConfirmOpen.value = true
    }
    else {
      toast.error('Script failed', { description: msg })
    }
  }
  finally {
    isRunning.value = false
  }
}

const resultColumns = computed(() => {
  const rows = runResults.value
  if (!rows || rows.length === 0) return []
  const first = rows[0]
  return first && typeof first === 'object' ? Object.keys(first as object) : []
})

function openInWorkspace(script: Script) {
  // Substitute current param values for a ready-to-run body.
  let body = script.body
  for (const [k, v] of Object.entries(paramValues.value)) {
    body = body.replaceAll(`{{${k}}}`, v).replaceAll(`{{ ${k} }}`, v)
  }
  router.push({ path: '/', query: { sql: body } })
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-lg font-semibold">Scripts</h1>
        <p class="text-sm text-muted-foreground">
          Saved and built-in scripts
          <span v-if="activeEngine"> for {{ activeEngine }}</span>
        </p>
      </div>
      <div class="relative w-64">
        <Icon name="lucide:search" class="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
        <Input v-model="searchTerm" placeholder="Search scripts..." class="pl-8" />
      </div>
    </div>

    <div v-if="!activeConnection" class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
      Connect to a database to run scripts. Showing all scripts below.
    </div>

    <!-- Loading -->
    <div v-if="isLoading && scripts.length === 0" class="flex flex-col gap-2">
      <div v-for="i in 4" :key="i" class="h-16 bg-muted/50 rounded-md animate-pulse" />
    </div>

    <!-- Script list -->
    <div v-else-if="filteredScripts.length > 0" class="flex flex-col gap-2">
      <div
        v-for="script in filteredScripts"
        :key="script.id"
        class="group flex items-start justify-between gap-3 rounded-lg border border-border bg-card p-3 hover:border-border/80 transition-colors"
      >
        <div class="flex flex-col gap-1 min-w-0">
          <div class="flex items-center gap-2">
            <p class="text-sm font-medium text-foreground truncate">{{ script.name }}</p>
            <Badge v-if="script.is_builtin" variant="secondary" class="text-xs gap-1">
              <Icon name="lucide:lock" class="size-3" /> Built-in
            </Badge>
            <Badge variant="outline" class="text-xs">{{ script.engine }}</Badge>
          </div>
          <p v-if="script.description" class="text-xs text-muted-foreground line-clamp-2">
            {{ script.description }}
          </p>
        </div>
        <div class="flex items-center gap-1.5 shrink-0">
          <Button size="sm" variant="default" @click="openRun(script)">
            <Icon name="lucide:play" class="size-3.5" /> Run
          </Button>
          <Button
            v-if="!script.is_builtin"
            size="sm"
            variant="ghost"
            @click="scriptsStore.deleteScript(script.id)"
          >
            <Icon name="lucide:trash-2" class="size-3.5" />
          </Button>
        </div>
      </div>
    </div>

    <!-- Empty -->
    <div v-else class="flex flex-col items-center justify-center py-16 text-muted-foreground gap-3">
      <Icon name="lucide:scroll-text" class="size-8" />
      <p class="text-sm">No scripts found</p>
    </div>

    <!-- Run dialog -->
    <Dialog v-model:open="runDialogOpen">
      <DialogContent class="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{{ activeScript?.name }}</DialogTitle>
          <DialogDescription>{{ activeScript?.description }}</DialogDescription>
        </DialogHeader>

        <div class="flex flex-col gap-3">
          <div v-for="p in activeParams" :key="p.name" class="flex flex-col gap-1.5">
            <Label :for="`param-${p.name}`">{{ p.label ?? p.name }}</Label>
            <Input
              :id="`param-${p.name}`"
              v-model="paramValues[p.name]"
              :type="p.type === 'number' ? 'number' : 'text'"
              :placeholder="p.type"
            />
          </div>

          <pre class="rounded-md bg-muted/50 p-2 text-xs font-mono overflow-x-auto max-h-32">{{ activeScript?.body }}</pre>

          <!-- Results -->
          <div v-if="runResults" class="rounded-md border border-border overflow-auto max-h-64">
            <table v-if="runResults.length > 0" class="w-full text-xs">
              <thead class="bg-muted/50 sticky top-0">
                <tr>
                  <th v-for="col in resultColumns" :key="col" class="px-2 py-1 text-left font-medium">{{ col }}</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(row, i) in runResults" :key="i" class="border-t border-border">
                  <td v-for="col in resultColumns" :key="col" class="px-2 py-1 font-mono">
                    {{ (row && typeof row === 'object') ? (row as Record<string, unknown>)[col] : '' }}
                  </td>
                </tr>
              </tbody>
            </table>
            <p v-else class="p-3 text-xs text-muted-foreground">No rows returned</p>
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" @click="activeScript && openInWorkspace(activeScript)">
            Open in workspace
          </Button>
          <Button :disabled="isRunning" @click="doRun">
            <Icon v-if="isRunning" name="lucide:loader-2" class="size-4 animate-spin" />
            <Icon v-else name="lucide:play" class="size-4" />
            Run
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Destructive confirmation -->
    <AlertDialog v-model:open="destructiveConfirmOpen">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Destructive script blocked</AlertDialogTitle>
          <AlertDialogDescription>
            This script contains operations that modify data (e.g. INSERT, UPDATE, DELETE, DROP).
            For safety, Query Studio runs scripts read-only. Open it in the workspace to review and
            run it manually.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction @click="activeScript && openInWorkspace(activeScript)">
            Open in workspace
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </div>
</template>

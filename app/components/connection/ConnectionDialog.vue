<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '~/components/ui/dialog'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { Separator } from '~/components/ui/separator'
import { engines, buildConnectionString, isMongoUri, type DatabaseEngine } from '~/constants/engines'
import { useConnectionsStore } from '~/stores/connections'
import type { Connection } from '~/types/database'

const router = useRouter()

const props = defineProps<{
  editConnection?: Connection | null
}>()

const open = defineModel<boolean>('open', { default: false })

const connectionsStore = useConnectionsStore()
const isEditMode = computed(() => !!props.editConnection)

const selectedEngine = ref<DatabaseEngine>(engines[0])
const isTesting = ref(false)
const isSaving = ref(false)
const saveError = ref<string | null>(null)
const testResult = ref<'success' | 'error' | null>(null)
const testMessage = ref('')

function defaultFormForEngine(engine: DatabaseEngine) {
  return {
    name: '',
    host: engine.id === 'sqlite' ? '' : engine.placeholder.host,
    port: engine.defaultPort?.toString() ?? '',
    database: engine.defaultDatabase,
    username: engine.placeholder.username,
    password: '',
    ssl_enabled: false,
    auth_json: '',
  }
}

const form = ref(defaultFormForEngine(engines[0]))

// When editing, populate form with existing data
watch(() => props.editConnection, (conn) => {
  if (conn) {
    const engine = engines.find(e => e.id === conn.db_type) ?? engines[0]
    selectedEngine.value = engine
    form.value = {
      name: conn.name,
      host: conn.host,
      port: conn.port,
      database: conn.database,
      username: conn.username,
      password: conn.password,
      ssl_enabled: conn.ssl_enabled,
      auth_json: conn.auth_json || '',
    }
    if (conn.auth_json) {
      parseServiceAccountJson(conn.auth_json)
    }
    if (engine.id === 'mongodb') {
      mongoMode.value = isMongoUri(conn.host) ? 'uri' : 'manual'
    }
    testResult.value = null
    saveError.value = null
  }
}, { immediate: true })

// When switching engine type in new-connection mode, reset host/port/db/user defaults
function onEngineChange(engine: DatabaseEngine) {
  selectedEngine.value = engine
  if (!isEditMode.value) {
    const defaults = defaultFormForEngine(engine)
    form.value.host = defaults.host
    form.value.port = defaults.port
    form.value.database = defaults.database
    form.value.username = defaults.username
  }
  testResult.value = null
  saveError.value = null
}

const connectionPreview = computed(() => {
  if (!form.value.host) return ''
  return buildConnectionString({
    db_type: selectedEngine.value.id,
    host: form.value.host,
    port: form.value.port,
    database: form.value.database,
    username: form.value.username,
    password: form.value.password,
  })
})

const isSqlite = computed(() => selectedEngine.value.id === 'sqlite')
const isFirebase = computed(() => selectedEngine.value.id === 'firestore' || selectedEngine.value.id === 'firebase_rtdb')
const isFirestoreOnly = computed(() => selectedEngine.value.id === 'firestore')
const isRtdbOnly = computed(() => selectedEngine.value.id === 'firebase_rtdb')
const isMongo = computed(() => selectedEngine.value.id === 'mongodb')
const firebaseProjectId = ref('')

// MongoDB supports two entry modes: paste a full URI (mongodb:// or
// mongodb+srv://) or fill in host/port/user/password manually. URI mode is
// detected from the stored `host` value so existing connections round-trip.
const mongoMode = ref<'uri' | 'manual'>('manual')

watch(selectedEngine, (engine) => {
  if (engine.id === 'mongodb') {
    mongoMode.value = isMongoUri(form.value.host) ? 'uri' : 'manual'
  }
}, { immediate: true })

function setMongoMode(mode: 'uri' | 'manual') {
  mongoMode.value = mode
  if (mode === 'uri') {
    // Clear out the manual host placeholder so the URI textarea starts fresh
    if (!isMongoUri(form.value.host)) {
      form.value.host = ''
      form.value.port = ''
      form.value.username = ''
    }
  }
  else if (isMongoUri(form.value.host)) {
    // Switching back to manual: restore engine defaults
    const defaults = defaultFormForEngine(selectedEngine.value)
    form.value.host = defaults.host
    form.value.port = defaults.port
    form.value.username = defaults.username
  }
  testResult.value = null
  saveError.value = null
}

function parseServiceAccountJson(raw: string) {
  try {
    const parsed = JSON.parse(raw)
    if (parsed.project_id) {
      firebaseProjectId.value = parsed.project_id
      form.value.username = parsed.project_id
    }
    form.value.auth_json = raw
  }
  catch {
    form.value.auth_json = raw
    firebaseProjectId.value = ''
  }
}

async function testConnection() {
  if (isFirebase.value && !form.value.auth_json) {
    toast.error('Please provide the service account JSON')
    return
  }
  if (!isFirebase.value && !form.value.host) {
    toast.error('Please fill in the connection details')
    return
  }

  isTesting.value = true
  testResult.value = null

  try {
    let connStr = connectionPreview.value
    if (isFirebase.value) {
      // Preview is display-only; ask the backend to build the real base64 blob
      // that the firestore / firebase_rtdb drivers expect.
      connStr = await invoke<string>('build_firebase_conn_str', {
        authJson: form.value.auth_json,
        databaseUrl: isRtdbOnly.value ? form.value.host : null,
        firestoreDbId: isFirestoreOnly.value ? form.value.database : null,
      })
    }

    await invoke('test_connection', {
      engine: selectedEngine.value.id,
      connStr,
    })
    testResult.value = 'success'
    testMessage.value = 'Connection successful!'
    toast.success('Connection successful!')
  }
  catch (err) {
    testResult.value = 'error'
    testMessage.value = err as string
    toast.error('Connection failed', { description: err as string })
  }
  finally {
    isTesting.value = false
  }
}

async function saveConnection() {
  saveError.value = null

  if (!form.value.name.trim()) {
    saveError.value = 'Please provide a connection name.'
    return
  }
  if (isFirebase.value) {
    if (!form.value.auth_json.trim()) {
      saveError.value = 'Please provide the service account JSON.'
      return
    }
    if (isRtdbOnly.value && !form.value.host.trim()) {
      saveError.value = 'Please provide the Realtime Database URL.'
      return
    }
  }
  else if (!form.value.host.trim()) {
    saveError.value = selectedEngine.value.id === 'sqlite'
      ? 'Please provide the database file path.'
      : 'Please provide the host.'
    return
  }

  isSaving.value = true

  try {
    if (isEditMode.value && props.editConnection) {
      const result = await connectionsStore.updateConnection({
        ...props.editConnection,
        name: form.value.name,
        db_type: selectedEngine.value.id,
        host: form.value.host,
        port: form.value.port,
        database: form.value.database,
        username: form.value.username,
        password: form.value.password,
        ssl_enabled: form.value.ssl_enabled,
        auth_json: form.value.auth_json || '',
      })

      if (result) {
        toast.success('Connection updated')
        open.value = false
      }
    }
    else {
      const result = await connectionsStore.addConnection({
        name: form.value.name,
        db_type: selectedEngine.value.id,
        host: form.value.host,
        port: form.value.port,
        database: form.value.database,
        username: form.value.username,
        password: form.value.password,
        ssl_enabled: form.value.ssl_enabled,
        auth_json: form.value.auth_json || '',
      })

      // Close dialog first
      open.value = false
      resetForm()

      toast.loading(`Connecting to ${result.name}...`, { id: 'connecting' })

      try {
        await invoke<boolean>('test_connection_by_id', {
          connectionId: result.id,
        })

        toast.success(`Connected to ${result.name}`, { id: 'connecting' })

        // Load schema in background
        connectionsStore.loadSchema().then(() => {
          if (connectionsStore.tables.length > 0) {
            toast.success(`Schema loaded: ${connectionsStore.tables.length} tables`)
          }
        })

        router.push('/')
      }
      catch (err) {
        toast.error(`Saved but could not connect to ${result.name}`, {
          id: 'connecting',
          description: err as string,
        })
      }
    }
  }
  catch (err) {
    saveError.value = err as string
  }
  finally {
    isSaving.value = false
  }
}

function resetForm() {
  selectedEngine.value = engines[0]
  form.value = defaultFormForEngine(engines[0])
  testResult.value = null
  saveError.value = null
}

watch(open, (val) => {
  if (val && !isEditMode.value) {
    // Pre-fill defaults when opening for a new connection
    resetForm()
  }
  if (!val && !isEditMode.value) {
    resetForm()
  }
})
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="max-w-lg">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Icon :name="isEditMode ? 'lucide:pencil' : 'lucide:plug'" class="size-5" />
          {{ isEditMode ? 'Edit Connection' : 'New Connection' }}
        </DialogTitle>
        <DialogDescription>
          {{ isEditMode ? 'Update your database connection details.' : 'Connect to a SQL or NoSQL database to start querying with AI.' }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-4 py-2">
        <!-- Engine selector -->
        <div class="space-y-2">
          <Label>Database Type</Label>
          <div class="grid grid-cols-4 gap-1.5">
            <button
              v-for="engine in engines"
              :key="engine.id"
              class="flex flex-col items-center gap-1 rounded-lg border p-2.5 text-center transition-colors hover:bg-accent"
              :class="selectedEngine.id === engine.id
                ? 'border-primary bg-accent text-foreground'
                : 'border-border text-muted-foreground'"
              @click="onEngineChange(engine)"
            >
              <Icon :name="engine.icon" class="size-5" />
              <span class="text-xs font-medium leading-none">{{ engine.name }}</span>
            </button>
          </div>
        </div>

        <Separator />

        <!-- Connection name -->
        <div class="space-y-2">
          <Label for="conn-name">Connection Name</Label>
          <Input
            id="conn-name"
            v-model="form.name"
            :placeholder="`My ${selectedEngine.name} DB`"
            autocomplete="off"
            autocorrect="off"
            autocapitalize="off"
            spellcheck="false"
          />
        </div>

        <!-- SQLite: file path only -->
        <template v-if="isSqlite">
          <div class="space-y-2">
            <Label for="sqlite-path">Database File Path</Label>
            <Input
              id="sqlite-path"
              v-model="form.host"
              placeholder="/path/to/database.db"
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck="false"
            />
          </div>
        </template>

        <!-- Firebase connection fields -->
        <template v-else-if="isFirebase">
          <div class="space-y-2">
            <Label for="auth-json">Service Account JSON</Label>
            <textarea
              id="auth-json"
              :value="form.auth_json"
              class="flex min-h-[120px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 font-mono"
              placeholder='Paste your service-account.json contents here...'
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck="false"
              @input="parseServiceAccountJson(($event.target as HTMLTextAreaElement).value)"
            />
            <p v-if="firebaseProjectId" class="text-xs text-muted-foreground">
              Project: <span class="font-mono font-medium text-foreground">{{ firebaseProjectId }}</span>
            </p>
          </div>

          <div v-if="isRtdbOnly" class="space-y-2">
            <Label for="rtdb-url">Database URL</Label>
            <Input
              id="rtdb-url"
              v-model="form.host"
              placeholder="https://my-app-default-rtdb.firebaseio.com"
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck="false"
            />
          </div>

          <div v-if="isFirestoreOnly" class="space-y-2">
            <Label for="firestore-db-id">Database ID</Label>
            <Input
              id="firestore-db-id"
              v-model="form.database"
              placeholder="(default)"
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck="false"
            />
          </div>

          <div class="rounded-md border border-amber-500/30 bg-amber-500/10 p-2.5">
            <div class="flex items-start gap-2 text-xs text-amber-600 dark:text-amber-400">
              <Icon name="lucide:shield-alert" class="size-4 shrink-0 mt-0.5" />
              <span>Service account auth bypasses Firebase security rules. Use only for trusted, internal access.</span>
            </div>
          </div>
        </template>

        <!-- SQL/NoSQL connection fields -->
        <template v-else>
          <!-- MongoDB: choose between connection URI or manual fields -->
          <div v-if="isMongo" class="space-y-2">
            <Label>Connection Mode</Label>
            <div class="grid grid-cols-2 gap-1.5 rounded-md border p-1">
              <button
                type="button"
                class="rounded px-2 py-1.5 text-xs font-medium transition-colors"
                :class="mongoMode === 'uri'
                  ? 'bg-accent text-foreground'
                  : 'text-muted-foreground hover:bg-accent/50'"
                @click="setMongoMode('uri')"
              >
                Connection URI
              </button>
              <button
                type="button"
                class="rounded px-2 py-1.5 text-xs font-medium transition-colors"
                :class="mongoMode === 'manual'
                  ? 'bg-accent text-foreground'
                  : 'text-muted-foreground hover:bg-accent/50'"
                @click="setMongoMode('manual')"
              >
                Manual
              </button>
            </div>
          </div>

          <!-- MongoDB URI mode: paste full mongodb:// or mongodb+srv:// URI -->
          <template v-if="isMongo && mongoMode === 'uri'">
            <div class="space-y-2">
              <Label for="mongo-uri">Connection URI</Label>
              <textarea
                id="mongo-uri"
                v-model="form.host"
                class="flex min-h-[80px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 font-mono"
                placeholder="mongodb+srv://user:<db_password>@cluster0.xxxxx.mongodb.net/?appName=Cluster0"
                autocomplete="off"
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
              />
              <p class="text-xs text-muted-foreground">
                Supports <code class="font-mono">mongodb://</code> and <code class="font-mono">mongodb+srv://</code>. The
                <code class="font-mono">&lt;db_password&gt;</code> placeholder will be replaced by the password below.
              </p>
            </div>
            <div class="space-y-2">
              <Label for="mongo-uri-password">Password</Label>
              <Input
                id="mongo-uri-password"
                v-model="form.password"
                type="password"
                :placeholder="isEditMode ? '(unchanged)' : 'Substituted into <db_password>'"
                autocomplete="new-password"
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
              />
            </div>
          </template>

          <!-- Manual host/port/database/credentials -->
          <template v-else>
            <div class="grid grid-cols-3 gap-3">
              <div class="col-span-2 space-y-2">
                <Label for="host">Host</Label>
                <Input
                  id="host"
                  v-model="form.host"
                  :placeholder="selectedEngine.placeholder.host"
                  autocomplete="off"
                  autocorrect="off"
                  autocapitalize="off"
                  spellcheck="false"
                />
              </div>
              <div class="space-y-2">
                <Label for="port">Port</Label>
                <Input
                  id="port"
                  v-model="form.port"
                  type="number"
                  :placeholder="selectedEngine.defaultPort?.toString()"
                  autocomplete="off"
                />
              </div>
            </div>

            <div class="space-y-2">
              <Label for="database">
                {{ selectedEngine.id === 'redis' ? 'Database Index' : 'Database' }}
              </Label>
              <Input
                id="database"
                v-model="form.database"
                :placeholder="selectedEngine.defaultDatabase"
                autocomplete="off"
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
              />
            </div>

            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-2">
                <Label for="username">Username</Label>
                <Input
                  id="username"
                  v-model="form.username"
                  :placeholder="selectedEngine.placeholder.username"
                  autocomplete="off"
                  autocorrect="off"
                  autocapitalize="off"
                  spellcheck="false"
                />
              </div>
              <div class="space-y-2">
                <Label for="password">Password</Label>
                <Input
                  id="password"
                  v-model="form.password"
                  type="password"
                  :placeholder="isEditMode ? '(unchanged)' : '••••••••'"
                  autocomplete="new-password"
                  autocorrect="off"
                  autocapitalize="off"
                  spellcheck="false"
                />
              </div>
            </div>
          </template>
        </template>

        <!-- Connection preview -->
        <div v-if="connectionPreview" class="rounded-md bg-muted p-2.5">
          <p class="text-xs text-muted-foreground font-mono break-all">{{ connectionPreview.replace(/:([^:@]+)@/, ':***@') }}</p>
        </div>

        <!-- Test result -->
        <div
          v-if="testResult"
          class="flex items-center gap-2 rounded-md p-2.5 text-sm"
          :class="testResult === 'success' ? 'bg-green-500/10 text-green-600 dark:text-green-400' : 'bg-destructive/10 text-destructive'"
        >
          <Icon :name="testResult === 'success' ? 'lucide:check-circle' : 'lucide:alert-circle'" class="size-4 shrink-0" />
          <span class="truncate">{{ testMessage }}</span>
        </div>
      </div>

      <!-- Inline error -->
      <div v-if="saveError" class="flex items-start gap-2 rounded-md bg-destructive/10 border border-destructive/20 px-3 py-2 text-sm text-destructive">
        <Icon name="lucide:alert-circle" class="size-4 shrink-0 mt-0.5" />
        <span>{{ saveError }}</span>
      </div>

      <DialogFooter class="gap-2">
        <Button variant="outline" :disabled="isTesting" @click="testConnection">
          <Icon v-if="isTesting" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:zap" class="size-4" />
          Test
        </Button>
        <Button :disabled="isSaving" @click="saveConnection">
          <Icon v-if="isSaving" name="lucide:loader-2" class="size-4 animate-spin" />
          <Icon v-else name="lucide:save" class="size-4" />
          {{ isEditMode ? 'Save Changes' : 'Save & Connect' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

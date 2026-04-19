<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { toast } from 'vue-sonner'
import { Button } from '~/components/ui/button'
import { Input } from '~/components/ui/input'
import { Label } from '~/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '~/components/ui/select'
import { Separator } from '~/components/ui/separator'
import type { LlmConfig } from '~/types/database'

useHead({ title: 'Settings' })

const llmConfig = ref<LlmConfig>({
  provider: 'gemini',
  model: 'gemini-pro',
  api_key: '',
  api_url: null,
  created_at: '',
  updated_at: '',
})

const isTesting = ref(false)
const isSaving = ref(false)

const providers = [
  {
    id: 'gemini',
    name: 'Google Gemini',
    models: [
      'gemini-2.5-pro',
      'gemini-2.5-flash',
      'gemini-2.5-flash-lite',
      'gemini-3.1-pro-preview',
      'gemini-3-flash-preview',
      'gemini-3.1-flash-lite-preview',
    ],
  },
  {
    id: 'openai',
    name: 'OpenAI',
    models: [
      'gpt-5.4',
      'gpt-5.4-mini',
      'gpt-5.4-nano',
      'gpt-4o',
      'gpt-4o-mini',
      'o3-mini',
    ],
  },
  {
    id: 'anthropic',
    name: 'Anthropic',
    models: [
      'claude-opus-4-6',
      'claude-sonnet-4-6',
      'claude-haiku-4-5',
      'claude-sonnet-4-5',
      'claude-opus-4-5',
      'claude-sonnet-4-0',
    ],
  },
  {
    id: 'ollama',
    name: 'Ollama (Local)',
    models: [
      'qwen3',
      'qwen3.5',
      'llama3.3',
      'llama3.1',
      'deepseek-r1',
      'gemma3',
      'gemma4',
      'mistral',
      'mistral-small',
      'qwen2.5-coder',
      'qwen3-coder',
      'codellama',
      'phi4',
      'gpt-oss',
    ],
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    models: ['deepseek-chat', 'deepseek-reasoner'],
  },
  {
    id: 'groq',
    name: 'Groq',
    models: [
      'openai/gpt-oss-120b',
      'openai/gpt-oss-20b',
      'llama-3.3-70b-versatile',
      'llama-3.1-8b-instant',
      'qwen/qwen3-32b',
      'meta-llama/llama-4-scout-17b-16e-instruct',
    ],
  },
  {
    id: 'custom',
    name: 'Custom (OpenAI-compatible)',
    models: [],
  },
]

const ollamaModels = ref<string[]>([])
const isFetchingOllamaModels = ref(false)

const currentProvider = computed(() =>
  providers.find(p => p.id === llmConfig.value.provider),
)

const availableModels = computed(() => {
  if (llmConfig.value.provider === 'ollama' && ollamaModels.value.length > 0) {
    return ollamaModels.value
  }
  return currentProvider.value?.models ?? []
})

const showApiUrl = computed(() =>
  llmConfig.value.provider === 'ollama' || llmConfig.value.provider === 'custom',
)

async function fetchOllamaModels() {
  const baseUrl = llmConfig.value.api_url || 'http://localhost:11434'
  isFetchingOllamaModels.value = true

  try {
    const response = await fetch(`${baseUrl}/api/tags`)
    if (response.ok) {
      const data = await response.json()
      const models = (data.models ?? []).map((m: { name: string }) => m.name)
      ollamaModels.value = models
      if (models.length > 0 && !models.includes(llmConfig.value.model)) {
        llmConfig.value.model = models[0]
      }
      toast.success(`Found ${models.length} local models`)
    }
  }
  catch {
    ollamaModels.value = []
    toast.error('Could not reach Ollama', { description: `Make sure Ollama is running at ${baseUrl}` })
  }
  finally {
    isFetchingOllamaModels.value = false
  }
}

onMounted(async () => {
  try {
    llmConfig.value = await invoke<LlmConfig>('get_llm_config')
    if (llmConfig.value.provider === 'ollama') {
      fetchOllamaModels()
    }
  }
  catch (err) {
    console.error('Failed to load LLM config:', err)
  }
})

watch(() => llmConfig.value.provider, (provider) => {
  if (provider === 'ollama') {
    fetchOllamaModels()
    return
  }
  ollamaModels.value = []
  const p = providers.find(pr => pr.id === provider)
  if (p?.models.length) {
    llmConfig.value.model = p.models[0]
  }
})

const {
  state: updaterState,
  progress: updaterProgress,
  currentVersion: updaterCurrentVersion,
  latestVersion: updaterLatestVersion,
  lastChecked: updaterLastChecked,
  errorMessage: updaterError,
  loadCurrentVersion,
  checkForUpdate,
  installAndRelaunch,
} = useAppUpdater()

onMounted(() => {
  loadCurrentVersion()
})

async function saveConfig() {
  isSaving.value = true

  try {
    await invoke('update_llm_config', {
      config: {
        provider: llmConfig.value.provider,
        model: llmConfig.value.model,
        api_key: llmConfig.value.api_key,
        api_url: llmConfig.value.api_url,
      },
    })
    toast.success('Settings saved')
  }
  catch (err) {
    toast.error('Failed to save settings', { description: err as string })
  }
  finally {
    isSaving.value = false
  }
}
</script>

<template>
  <div class="max-w-2xl mx-auto space-y-6">
    <div>
      <h1 class="text-lg font-semibold">Settings</h1>
      <p class="text-sm text-muted-foreground">Configure your AI model and preferences</p>
    </div>

    <Separator />

    <!-- AI Model Config -->
    <div class="space-y-4">
      <h2 class="text-sm font-semibold">AI Model</h2>

      <div class="space-y-2">
        <Label>Provider</Label>
        <Select v-model="llmConfig.provider">
          <SelectTrigger>
            <SelectValue placeholder="Select provider" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="p in providers" :key="p.id" :value="p.id">
              {{ p.name }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <Label>Model</Label>
          <Button
            v-if="llmConfig.provider === 'ollama'"
            size="sm"
            variant="ghost"
            class="h-6 px-2 text-xs gap-1"
            :disabled="isFetchingOllamaModels"
            @click="fetchOllamaModels"
          >
            <Icon v-if="isFetchingOllamaModels" name="lucide:loader-2" class="size-3 animate-spin" />
            <Icon v-else name="lucide:refresh-cw" class="size-3" />
            {{ ollamaModels.length > 0 ? `${ollamaModels.length} local models` : 'Fetch models' }}
          </Button>
        </div>
        <template v-if="availableModels.length > 0">
          <Select v-model="llmConfig.model">
            <SelectTrigger>
              <SelectValue placeholder="Select model" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="m in availableModels" :key="m" :value="m">
                {{ m }}
              </SelectItem>
            </SelectContent>
          </Select>
        </template>
        <template v-else>
          <Input v-model="llmConfig.model" placeholder="e.g. my-custom-model" />
        </template>
      </div>

      <div class="space-y-2">
        <Label>API Key</Label>
        <Input
          v-model="llmConfig.api_key"
          type="password"
          :placeholder="llmConfig.provider === 'ollama' ? 'Not required for Ollama' : 'sk-...'"
          :disabled="llmConfig.provider === 'ollama'"
        />
        <p class="text-xs text-muted-foreground">
          Your API key is stored locally and never sent anywhere except to the AI provider.
        </p>
      </div>

      <div v-if="showApiUrl" class="space-y-2">
        <Label>API URL</Label>
        <Input
          v-model="llmConfig.api_url"
          :placeholder="llmConfig.provider === 'ollama' ? 'http://localhost:11434' : 'https://your-api.com/v1'"
        />
      </div>

      <Button :disabled="isSaving" @click="saveConfig">
        <Icon v-if="isSaving" name="lucide:loader-2" class="size-4 animate-spin" />
        <Icon v-else name="lucide:save" class="size-4" />
        Save Settings
      </Button>
    </div>

    <Separator />

    <!-- Updates -->
    <div class="space-y-3">
      <div>
        <h2 class="text-sm font-semibold">Updates</h2>
        <p class="text-xs text-muted-foreground">
          Query Studio checks for new releases automatically a few times per day.
        </p>
      </div>

      <div class="flex items-center justify-between rounded-md border border-border p-3">
        <div class="space-y-0.5">
          <div class="text-sm font-medium">
            Current version
            <span class="text-muted-foreground">v{{ updaterCurrentVersion || '…' }}</span>
          </div>
          <div class="text-xs text-muted-foreground">
            <template v-if="updaterState === 'checking'">Checking for updates…</template>
            <template v-else-if="updaterState === 'available'">
              Update available: v{{ updaterLatestVersion }}
            </template>
            <template v-else-if="updaterState === 'downloading'">
              Downloading update… {{ updaterProgress }}%
            </template>
            <template v-else-if="updaterState === 'ready'">
              Update ready — restart to apply
            </template>
            <template v-else-if="updaterState === 'upToDate'">
              You're on the latest version.
            </template>
            <template v-else-if="updaterState === 'error'">
              <span class="text-destructive">{{ updaterError || 'Update failed' }}</span>
            </template>
            <template v-else-if="updaterLastChecked">
              Last checked {{ updaterLastChecked.toLocaleString() }}
            </template>
            <template v-else>
              Never checked.
            </template>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <Button
            v-if="updaterState === 'available' || updaterState === 'ready'"
            size="sm"
            @click="installAndRelaunch"
          >
            <Icon name="lucide:rotate-cw" class="size-4" />
            {{ updaterState === 'ready' ? 'Restart now' : 'Install & restart' }}
          </Button>
          <Button
            size="sm"
            variant="outline"
            :disabled="updaterState === 'checking' || updaterState === 'downloading'"
            @click="checkForUpdate()"
          >
            <Icon
              v-if="updaterState === 'checking' || updaterState === 'downloading'"
              name="lucide:loader-2"
              class="size-4 animate-spin"
            />
            <Icon v-else name="lucide:refresh-cw" class="size-4" />
            Check for updates
          </Button>
        </div>
      </div>
    </div>

    <Separator />

    <div class="space-y-2">
      <h2 class="text-sm font-semibold text-muted-foreground">About</h2>
      <p class="text-sm text-muted-foreground">
        QueryStudio — AI-powered database management tool.
        Built with Tauri, Nuxt 4, and Shadcn Vue.
      </p>
    </div>
  </div>
</template>

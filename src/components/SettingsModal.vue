<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const emit = defineEmits<{
  close: [];
  save: [];
}>();

interface CacheStats {
  entries: number;
  total_hits: number;
  max_entries: number;
  max_age_secs: number;
}

interface UserSettings {
  theme: string;
  default_page_size: number;
  query_timeout: number;
  auto_save_queries: boolean;
  show_row_numbers: boolean;
  date_format: string;
  null_display: string;
}

interface LlmConfig {
  provider: string;
  model: string;
  api_key: string;
  api_url: string | null;
  created_at: string;
  updated_at: string;
}

const defaultSettings: UserSettings = {
  theme: 'dark',
  default_page_size: 50,
  query_timeout: 30,
  auto_save_queries: true,
  show_row_numbers: true,
  date_format: 'YYYY-MM-DD HH:mm:ss',
  null_display: 'NULL',
};

const settings = ref<UserSettings>({ ...defaultSettings });
const llmConfig = ref<LlmConfig>({
  provider: 'gemini',
  model: 'gemini-pro',
  api_key: '',
  api_url: null,
  created_at: '',
  updated_at: '',
});

const loading = ref(false);
const saving = ref(false);
const error = ref('');
const successMessage = ref('');
const showApiKey = ref(false);
const testingLlm = ref(false);
const llmTestResult = ref<{ success: boolean; message: string } | null>(null);

const activeTab = ref<'aimodel' | 'general' | 'display' | 'query' | 'performance'>('aimodel');

const pageSizeOptions = [10, 25, 50, 100, 200, 500];
const timeoutOptions = [10, 30, 60, 120, 300];
const dateFormatOptions = [
  'YYYY-MM-DD HH:mm:ss',
  'DD/MM/YYYY HH:mm:ss',
  'MM/DD/YYYY HH:mm:ss',
  'YYYY-MM-DD',
  'DD/MM/YYYY',
];

// Supported LLM providers
const llmProviders = [
  { id: 'gemini', name: 'Google Gemini', icon: 'auto_awesome', color: 'text-blue-400' },
  { id: 'openai', name: 'OpenAI', icon: 'smart_toy', color: 'text-emerald-400' },
  { id: 'anthropic', name: 'Anthropic Claude', icon: 'psychology', color: 'text-amber-400' },
  { id: 'deepseek', name: 'DeepSeek', icon: 'rocket_launch', color: 'text-purple-400' },
  { id: 'groq', name: 'Groq', icon: 'bolt', color: 'text-orange-400' },
  { id: 'ollama', name: 'Ollama (Local)', icon: 'home', color: 'text-cyan-400' },
  { id: 'custom', name: 'Custom / Self-hosted', icon: 'tune', color: 'text-pink-400' },
];

// Suggested models per provider
const suggestedModels: Record<string, string[]> = {
  gemini: ['gemini-pro', 'gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-2.0-flash'],
  openai: ['gpt-4', 'gpt-4-turbo', 'gpt-4o', 'gpt-4o-mini', 'gpt-3.5-turbo', 'o1', 'o1-mini', 'o3-mini'],
  anthropic: ['claude-sonnet-4-20250514', 'claude-3-5-sonnet-20241022', 'claude-3-haiku-20240307', 'claude-3-opus-20240229'],
  deepseek: ['deepseek-chat', 'deepseek-coder', 'deepseek-reasoner'],
  groq: ['llama-3.3-70b-versatile', 'llama-3.1-8b-instant', 'mixtral-8x7b-32768', 'gemma2-9b-it'],
  ollama: ['llama3', 'llama3:70b', 'codellama', 'mistral', 'mixtral', 'deepseek-coder-v2', 'qwen2.5-coder'],
  custom: [],
};

const showUrlField = computed(() => {
  return ['ollama', 'custom'].includes(llmConfig.value.provider);
});

const needsApiKey = computed(() => {
  return llmConfig.value.provider !== 'ollama';
});

const currentProviderModels = computed(() => {
  return suggestedModels[llmConfig.value.provider] || [];
});

// Cache stats
const cacheStats = ref<CacheStats | null>(null);
const clearingCache = ref(false);

async function loadCacheStats() {
  try {
    cacheStats.value = await invoke<CacheStats>('get_cache_stats');
  } catch {
    cacheStats.value = null;
  }
}

async function clearCache() {
  clearingCache.value = true;
  try {
    await invoke('clear_query_cache');
    await loadCacheStats();
  } finally {
    clearingCache.value = false;
  }
}

async function loadSettings() {
  loading.value = true;
  error.value = '';
  
  try {
    // Load general settings
    const result = await invoke<any>('get_settings');
    if (result) {
      settings.value = { ...defaultSettings, ...result };
    }
  } catch {
    settings.value = { ...defaultSettings };
  }

  try {
    // Load LLM config
    const config = await invoke<LlmConfig>('get_llm_config');
    if (config) {
      llmConfig.value = config;
    }
  } catch {
    // Use defaults
  }

  loading.value = false;
}

async function saveSettings() {
  saving.value = true;
  error.value = '';
  successMessage.value = '';
  
  try {
    // Save general settings
    await invoke('update_settings', {
      settings: {
        theme: settings.value.theme,
        default_page_size: settings.value.default_page_size,
        query_timeout_seconds: settings.value.query_timeout,
        auto_save_history: settings.value.auto_save_queries,
      },
    });

    // Save LLM config
    await invoke('update_llm_config', {
      config: {
        provider: llmConfig.value.provider,
        model: llmConfig.value.model,
        api_key: llmConfig.value.api_key,
        api_url: llmConfig.value.api_url || null,
      },
    });

    successMessage.value = 'Settings saved successfully';
    emit('save');
    
    setTimeout(() => {
      successMessage.value = '';
    }, 3000);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
}

async function testLlmConnection() {
  testingLlm.value = true;
  llmTestResult.value = null;

  try {
    // Save config first so the backend uses it
    await invoke('update_llm_config', {
      config: {
        provider: llmConfig.value.provider,
        model: llmConfig.value.model,
        api_key: llmConfig.value.api_key,
        api_url: llmConfig.value.api_url || null,
      },
    });

    // Try a simple translation to test the connection
    await invoke<string>('translate_to_sql', {
      query: 'show all tables',
    });

    llmTestResult.value = { success: true, message: 'Connection successful! Model responded correctly.' };
  } catch (e) {
    llmTestResult.value = { 
      success: false, 
      message: `Connection failed: ${e instanceof Error ? e.message : String(e)}`
    };
  } finally {
    testingLlm.value = false;
  }
}

function selectProvider(providerId: string) {
  llmConfig.value.provider = providerId;
  // Set a reasonable default model
  const models = suggestedModels[providerId];
  if (models && models.length > 0) {
    llmConfig.value.model = models[0];
  } else {
    llmConfig.value.model = '';
  }
  // Set default URL for Ollama
  if (providerId === 'ollama') {
    llmConfig.value.api_url = 'http://localhost:11434';
    llmConfig.value.api_key = 'ollama'; // Ollama doesn't need a key but field can't be empty
  } else if (providerId === 'custom') {
    llmConfig.value.api_url = '';
  } else {
    llmConfig.value.api_url = null;
  }
  llmTestResult.value = null;
}

function resetToDefaults() {
  settings.value = { ...defaultSettings };
}

onMounted(() => {
  loadSettings();
  loadCacheStats();
});
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50" @click.self="emit('close')">
    <div class="bg-surface-dark border border-[#3d4f51] rounded-2xl w-full max-w-2xl shadow-2xl overflow-hidden">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637]">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary">settings</span>
          <h2 class="text-lg font-semibold text-white">Settings</h2>
        </div>
        <button
          @click="emit('close')"
          class="p-1.5 hover:bg-white/10 rounded-lg transition-colors"
        >
          <span class="material-symbols-outlined text-[#9fb4b7]">close</span>
        </button>
      </div>

      <div class="flex">
        <!-- Sidebar tabs -->
        <div class="w-48 border-r border-[#2a3637] py-4 shrink-0">
          <button
            @click="activeTab = 'aimodel'"
            class="w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors"
            :class="activeTab === 'aimodel' ? 'bg-primary/20 text-primary border-r-2 border-primary' : 'text-[#9fb4b7] hover:bg-white/5'"
          >
            <span class="material-symbols-outlined text-[20px]">auto_awesome</span>
            <span class="text-sm font-medium">AI Model</span>
          </button>
          <button
            @click="activeTab = 'general'"
            class="w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors"
            :class="activeTab === 'general' ? 'bg-primary/20 text-primary border-r-2 border-primary' : 'text-[#9fb4b7] hover:bg-white/5'"
          >
            <span class="material-symbols-outlined text-[20px]">tune</span>
            <span class="text-sm font-medium">General</span>
          </button>
          <button
            @click="activeTab = 'display'"
            class="w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors"
            :class="activeTab === 'display' ? 'bg-primary/20 text-primary border-r-2 border-primary' : 'text-[#9fb4b7] hover:bg-white/5'"
          >
            <span class="material-symbols-outlined text-[20px]">display_settings</span>
            <span class="text-sm font-medium">Display</span>
          </button>
          <button
            @click="activeTab = 'query'"
            class="w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors"
            :class="activeTab === 'query' ? 'bg-primary/20 text-primary border-r-2 border-primary' : 'text-[#9fb4b7] hover:bg-white/5'"
          >
            <span class="material-symbols-outlined text-[20px]">code</span>
            <span class="text-sm font-medium">Query</span>
          </button>
          <button
            @click="activeTab = 'performance'; loadCacheStats()"
            class="w-full flex items-center gap-3 px-4 py-2.5 text-left transition-colors"
            :class="activeTab === 'performance' ? 'bg-primary/20 text-primary border-r-2 border-primary' : 'text-[#9fb4b7] hover:bg-white/5'"
          >
            <span class="material-symbols-outlined text-[20px]">speed</span>
            <span class="text-sm font-medium">Performance</span>
          </button>
        </div>

        <!-- Content -->
        <div class="flex-1 p-6 max-h-[500px] overflow-y-auto">
          <!-- Loading state -->
          <div v-if="loading" class="flex items-center justify-center py-12">
            <div class="w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
          </div>

          <!-- AI Model Tab -->
          <div v-else-if="activeTab === 'aimodel'" class="space-y-6">
            <!-- Provider Selection -->
            <div>
              <label class="block text-sm font-medium text-white mb-3">LLM Provider</label>
              <div class="grid grid-cols-2 gap-2">
                <button
                  v-for="provider in llmProviders"
                  :key="provider.id"
                  @click="selectProvider(provider.id)"
                  class="flex items-center gap-2.5 px-3 py-2.5 rounded-lg text-left transition-all text-sm"
                  :class="llmConfig.provider === provider.id 
                    ? 'bg-primary/20 border border-primary text-white' 
                    : 'border border-[#3d4f51] text-[#9fb4b7] hover:text-white hover:border-[#5d6f71]'"
                >
                  <span class="material-symbols-outlined text-[18px]" :class="provider.color">{{ provider.icon }}</span>
                  <span>{{ provider.name }}</span>
                </button>
              </div>
            </div>

            <!-- Model Selection -->
            <div>
              <label class="block text-sm font-medium text-white mb-2">Model</label>
              <div v-if="currentProviderModels.length > 0" class="space-y-2">
                <select
                  v-model="llmConfig.model"
                  class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 text-white text-sm focus:outline-none focus:border-primary"
                >
                  <option v-for="model in currentProviderModels" :key="model" :value="model">
                    {{ model }}
                  </option>
                </select>
                <p class="text-xs text-[#5d6f71]">Or type a custom model name below</p>
              </div>
              <input
                v-model="llmConfig.model"
                type="text"
                class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 text-white text-sm focus:outline-none focus:border-primary"
                :class="{ 'mt-2': currentProviderModels.length > 0 }"
                placeholder="e.g. gpt-4, claude-3-sonnet, llama3..."
              />
            </div>

            <!-- API Key -->
            <div v-if="needsApiKey">
              <label class="block text-sm font-medium text-white mb-2">API Key</label>
              <div class="relative">
                <input
                  v-model="llmConfig.api_key"
                  :type="showApiKey ? 'text' : 'password'"
                  class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 pr-10 text-white text-sm focus:outline-none focus:border-primary font-mono"
                  placeholder="Enter your API key..."
                />
                <button
                  @click="showApiKey = !showApiKey"
                  class="absolute right-2 top-1/2 -translate-y-1/2 p-1 hover:bg-white/10 rounded transition-colors"
                >
                  <span class="material-symbols-outlined text-[18px] text-[#5d6f71]">
                    {{ showApiKey ? 'visibility_off' : 'visibility' }}
                  </span>
                </button>
              </div>
              <p class="text-xs text-[#5d6f71] mt-1.5">
                Your API key is stored locally on your machine and never sent anywhere except the provider's API.
              </p>
            </div>

            <!-- Custom URL -->
            <div v-if="showUrlField">
              <label class="block text-sm font-medium text-white mb-2">API URL</label>
              <input
                v-model="llmConfig.api_url"
                type="text"
                class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 text-white text-sm focus:outline-none focus:border-primary font-mono"
                :placeholder="llmConfig.provider === 'ollama' ? 'http://localhost:11434' : 'https://your-api-endpoint.com/v1/chat/completions'"
              />
              <p class="text-xs text-[#5d6f71] mt-1.5">
                <template v-if="llmConfig.provider === 'ollama'">
                  The URL where your Ollama instance is running.
                </template>
                <template v-else>
                  Full URL for your custom OpenAI-compatible API endpoint.
                </template>
              </p>
            </div>

            <!-- Test Connection -->
            <div class="flex items-center gap-3">
              <button
                @click="testLlmConnection"
                :disabled="testingLlm || (!llmConfig.api_key && needsApiKey)"
                class="flex items-center gap-2 px-4 py-2 bg-surface-dark border border-[#3d4f51] rounded-lg text-sm font-medium hover:bg-white/5 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <span v-if="testingLlm" class="w-4 h-4 border-2 border-primary border-t-transparent rounded-full animate-spin"></span>
                <span v-else class="material-symbols-outlined text-[18px] text-primary">network_check</span>
                {{ testingLlm ? 'Testing...' : 'Test Connection' }}
              </button>
              <div v-if="llmTestResult" class="flex items-center gap-2 text-sm">
                <span
                  class="material-symbols-outlined text-[18px]"
                  :class="llmTestResult.success ? 'text-emerald-400' : 'text-red-400'"
                >
                  {{ llmTestResult.success ? 'check_circle' : 'error' }}
                </span>
                <span :class="llmTestResult.success ? 'text-emerald-400' : 'text-red-400'">
                  {{ llmTestResult.message }}
                </span>
              </div>
            </div>
          </div>

          <!-- General Tab -->
          <div v-else-if="activeTab === 'general'" class="space-y-6">
            <div>
              <label class="block text-sm font-medium text-white mb-2">Theme</label>
              <div class="flex gap-3">
                <button
                  @click="settings.theme = 'dark'"
                  class="flex-1 flex items-center justify-center gap-2 py-3 px-4 rounded-lg border transition-all"
                  :class="settings.theme === 'dark' ? 'border-primary bg-primary/20 text-white' : 'border-[#3d4f51] text-[#9fb4b7] hover:border-[#5d6f71]'"
                >
                  <span class="material-symbols-outlined text-[20px]">dark_mode</span>
                  <span class="text-sm">Dark</span>
                </button>
                <button
                  @click="settings.theme = 'light'"
                  class="flex-1 flex items-center justify-center gap-2 py-3 px-4 rounded-lg border transition-all"
                  :class="settings.theme === 'light' ? 'border-primary bg-primary/20 text-white' : 'border-[#3d4f51] text-[#9fb4b7] hover:border-[#5d6f71]'"
                >
                  <span class="material-symbols-outlined text-[20px]">light_mode</span>
                  <span class="text-sm">Light</span>
                </button>
              </div>
              <p class="text-xs text-[#5d6f71] mt-2">Light theme coming soon</p>
            </div>

            <div>
              <label class="block text-sm font-medium text-white mb-2">Auto-save queries to history</label>
              <button
                @click="settings.auto_save_queries = !settings.auto_save_queries"
                class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors"
                :class="settings.auto_save_queries ? 'bg-primary' : 'bg-[#3d4f51]'"
              >
                <span
                  class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform"
                  :class="settings.auto_save_queries ? 'translate-x-6' : 'translate-x-1'"
                />
              </button>
            </div>
          </div>

          <!-- Display Tab -->
          <div v-else-if="activeTab === 'display'" class="space-y-6">
            <div>
              <label class="block text-sm font-medium text-white mb-2">Show row numbers</label>
              <button
                @click="settings.show_row_numbers = !settings.show_row_numbers"
                class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors"
                :class="settings.show_row_numbers ? 'bg-primary' : 'bg-[#3d4f51]'"
              >
                <span
                  class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform"
                  :class="settings.show_row_numbers ? 'translate-x-6' : 'translate-x-1'"
                />
              </button>
            </div>

            <div>
              <label class="block text-sm font-medium text-white mb-2">Date/Time format</label>
              <select
                v-model="settings.date_format"
                class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 text-white text-sm focus:outline-none focus:border-primary"
              >
                <option v-for="format in dateFormatOptions" :key="format" :value="format">
                  {{ format }}
                </option>
              </select>
            </div>

            <div>
              <label class="block text-sm font-medium text-white mb-2">NULL value display</label>
              <input
                v-model="settings.null_display"
                type="text"
                class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 text-white text-sm focus:outline-none focus:border-primary"
                placeholder="NULL"
              />
              <p class="text-xs text-[#5d6f71] mt-1">How NULL values are displayed in results</p>
            </div>
          </div>

          <!-- Query Tab -->
          <div v-else-if="activeTab === 'query'" class="space-y-6">
            <div>
              <label class="block text-sm font-medium text-white mb-2">Default page size</label>
              <select
                v-model.number="settings.default_page_size"
                class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 text-white text-sm focus:outline-none focus:border-primary"
              >
                <option v-for="size in pageSizeOptions" :key="size" :value="size">
                  {{ size }} rows
                </option>
              </select>
            </div>

            <div>
              <label class="block text-sm font-medium text-white mb-2">Query timeout (seconds)</label>
              <select
                v-model.number="settings.query_timeout"
                class="w-full bg-background-dark border border-[#3d4f51] rounded-lg px-4 py-2.5 text-white text-sm focus:outline-none focus:border-primary"
              >
                <option v-for="timeout in timeoutOptions" :key="timeout" :value="timeout">
                  {{ timeout }} seconds
                </option>
              </select>
              <p class="text-xs text-[#5d6f71] mt-1">Maximum time to wait for query results</p>
            </div>
          </div>

          <!-- Performance Tab -->
          <div v-else-if="activeTab === 'performance'" class="space-y-6">
            <div class="p-4 bg-background-dark border border-[#2a3637] rounded-xl">
              <div class="flex items-center justify-between mb-4">
                <div class="flex items-center gap-2">
                  <span class="material-symbols-outlined text-primary text-[20px]">cached</span>
                  <h3 class="text-white font-medium">Query Cache</h3>
                </div>
                <button
                  @click="clearCache"
                  :disabled="clearingCache"
                  class="px-3 py-1.5 text-xs font-medium bg-red-500/10 text-red-400 rounded-lg hover:bg-red-500/20 transition-colors disabled:opacity-50"
                >
                  {{ clearingCache ? 'Clearing...' : 'Clear Cache' }}
                </button>
              </div>

              <div v-if="cacheStats" class="grid grid-cols-2 gap-4">
                <div class="p-3 bg-surface-dark rounded-lg">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Cached Queries</p>
                  <p class="text-xl font-semibold text-white">{{ cacheStats.entries }}</p>
                  <p class="text-xs text-[#5d6f71]">of {{ cacheStats.max_entries }} max</p>
                </div>
                <div class="p-3 bg-surface-dark rounded-lg">
                  <p class="text-xs text-[#5d6f71] uppercase mb-1">Cache Hits</p>
                  <p class="text-xl font-semibold text-emerald-400">{{ cacheStats.total_hits }}</p>
                  <p class="text-xs text-[#5d6f71]">queries served from cache</p>
                </div>
              </div>

              <p class="text-xs text-[#5d6f71] mt-4">
                Query results are cached for {{ cacheStats?.max_age_secs || 60 }} seconds to improve performance.
              </p>
            </div>

            <div class="p-4 bg-background-dark border border-[#2a3637] rounded-xl">
              <div class="flex items-center gap-2 mb-3">
                <span class="material-symbols-outlined text-primary text-[20px]">info</span>
                <h3 class="text-white font-medium">Performance Tips</h3>
              </div>
              <ul class="space-y-2 text-sm text-[#9fb4b7]">
                <li class="flex items-start gap-2">
                  <span class="material-symbols-outlined text-[16px] text-emerald-400 mt-0.5">check</span>
                  <span>Use pagination for large result sets</span>
                </li>
                <li class="flex items-start gap-2">
                  <span class="material-symbols-outlined text-[16px] text-emerald-400 mt-0.5">check</span>
                  <span>Select only the columns you need</span>
                </li>
                <li class="flex items-start gap-2">
                  <span class="material-symbols-outlined text-[16px] text-emerald-400 mt-0.5">check</span>
                  <span>Add LIMIT clause to exploratory queries</span>
                </li>
                <li class="flex items-start gap-2">
                  <span class="material-symbols-outlined text-[16px] text-emerald-400 mt-0.5">check</span>
                  <span>Use indexes on frequently queried columns</span>
                </li>
              </ul>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-between px-6 py-4 border-t border-[#2a3637] bg-background-dark/50">
        <button
          @click="resetToDefaults"
          class="text-sm text-[#9fb4b7] hover:text-white transition-colors"
        >
          Reset to defaults
        </button>
        
        <div class="flex items-center gap-3">
          <span v-if="error" class="text-red-400 text-sm">{{ error }}</span>
          <span v-if="successMessage" class="text-emerald-400 text-sm">{{ successMessage }}</span>
          
          <button
            @click="emit('close')"
            class="px-4 py-2 text-sm font-medium text-[#9fb4b7] hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            @click="saveSettings"
            :disabled="saving"
            class="px-4 py-2 bg-primary hover:bg-primary/90 text-white text-sm font-medium rounded-lg transition-colors disabled:opacity-50 flex items-center gap-2"
          >
            <span v-if="saving" class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
            {{ saving ? 'Saving...' : 'Save Changes' }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

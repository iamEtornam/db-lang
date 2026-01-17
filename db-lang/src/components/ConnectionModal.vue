<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";

defineProps<{
  isOpen: boolean;
}>();

const emit = defineEmits<{
  close: [];
  connect: [connection: ConnectionConfig];
}>();

interface ConnectionConfig {
  name: string;
  type: string;
  host: string;
  port: string;
  username: string;
  password: string;
  sslEnabled: boolean;
}

const connectionName = ref("");
const dbType = ref("postgres");
const host = ref("");
const port = ref("5432");
const username = ref("");
const password = ref("");
const sslEnabled = ref(true);

const isSQLite = computed(() => dbType.value === "sqlite");

// Watch for database type changes and update default port
watch(dbType, (newType) => {
  switch (newType) {
    case "postgres":
      port.value = "5432";
      break;
    case "mysql":
      port.value = "3306";
      break;
    case "mssql":
      port.value = "1433";
      break;
    case "sqlite":
      host.value = "";
      port.value = "";
      break;
  }
});

const isTesting = ref(false);
const testStatus = ref<"idle" | "success" | "error">("idle");
const testMessage = ref("");

const handleConnect = () => {
  const config: ConnectionConfig = {
    name: connectionName.value,
    type: dbType.value,
    host: host.value,
    port: port.value,
    username: username.value,
    password: password.value,
    sslEnabled: sslEnabled.value,
  };
  emit("connect", config);
};

const handleCancel = () => {
  emit("close");
};

function buildConnectionString(type: string, host: string, port: string, user: string, pass: string): string {
  switch (type) {
    case "postgres":
      return `postgresql://${user}:${pass}@${host}:${port}/postgres`;
    case "mysql":
      return `mysql://${user}:${pass}@${host}:${port}/mysql`;
    case "sqlite":
      return host; // For SQLite, host is the file path
    case "mssql":
      return `mssql://${user}:${pass}@${host}:${port}/master`;
    default:
      return "";
  }
}

const handleTestConnection = async () => {
  isTesting.value = true;
  testStatus.value = "idle";
  testMessage.value = "";

  const connStr = buildConnectionString(dbType.value, host.value, port.value, username.value, password.value);
  
  if (!connStr) {
    testStatus.value = "error";
    testMessage.value = "Invalid configuration";
    isTesting.value = false;
    return;
  }

  try {
    const success = await invoke("test_connection", {
      engine: dbType.value,
      connStr: connStr,
    });
    
    if (success) {
      testStatus.value = "success";
      testMessage.value = "Connection successful!";
    } else {
      testStatus.value = "error";
      testMessage.value = "Connection failed";
    }
  } catch (err) {
    testStatus.value = "error";
    testMessage.value = err as string;
  } finally {
    isTesting.value = false;
  }
};
</script>

<template>
  <!-- Modal Backdrop -->
  <Transition name="fade">
    <div
      v-if="isOpen"
      class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      @click.self="handleCancel"
    >
      <!-- Modal Container -->
      <div
        class="w-full max-w-[520px] bg-white dark:bg-surface-dark rounded-xl shadow-[0_20px_70px_rgba(0,0,0,0.55)] overflow-hidden border border-gray-200 dark:border-[#2a3637] flex flex-col"
        @click.stop
      >
        <!-- Header -->
        <header class="flex flex-col gap-1 px-8 pt-8 pb-6 border-b border-gray-100 dark:border-[#2a3637]">
          <div class="flex items-center gap-3">
            <div class="size-8 bg-primary/20 rounded-lg flex items-center justify-center text-primary">
              <span class="material-symbols-outlined text-[20px]">database</span>
            </div>
            <h1 class="text-xl font-bold text-gray-900 dark:text-white tracking-tight">New Database Connection</h1>
          </div>
          <p class="text-gray-500 dark:text-[#9fb4b7] text-sm font-normal">
            Configure your instance to start querying with Gemini AI.
          </p>
        </header>

        <!-- Form Body -->
        <div class="flex-1 px-8 py-6 space-y-5 overflow-y-auto max-h-[70vh]">
          <!-- Connection Name -->
          <div class="space-y-1.5">
            <label class="block text-sm font-medium text-gray-700 dark:text-white">Connection Name</label>
            <input
              v-model="connectionName"
              class="w-full h-11 px-3.5 bg-gray-50 dark:bg-[#1d2526] border border-gray-200 dark:border-[#3d4f51] rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-primary/50 focus:border-primary outline-none transition-all placeholder:text-gray-400 dark:placeholder:text-[#5d6f71]"
              placeholder="e.g. Production Analytics"
              type="text"
            />
          </div>

          <!-- Database Type -->
          <div class="space-y-1.5">
            <label class="block text-sm font-medium text-gray-700 dark:text-white">Database Type</label>
            <div class="relative">
              <select
                v-model="dbType"
                class="w-full h-11 pl-10 pr-10 bg-gray-50 dark:bg-[#1d2526] border border-gray-200 dark:border-[#3d4f51] rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-primary/50 focus:border-primary outline-none transition-all appearance-none cursor-pointer"
              >
                <option value="postgres">PostgreSQL</option>
                <option value="mysql">MySQL</option>
                <option value="sqlite">SQLite</option>
                <option value="mssql">SQL Server</option>
              </select>
              <div
                class="absolute left-3.5 top-1/2 -translate-y-1/2 flex items-center pointer-events-none text-gray-400 dark:text-[#9fb4b7]"
              >
                <span class="material-symbols-outlined text-[18px]">storage</span>
              </div>
              <div
                class="absolute right-3.5 top-1/2 -translate-y-1/2 flex items-center pointer-events-none text-gray-400 dark:text-[#9fb4b7]"
              >
                <span class="material-symbols-outlined text-[18px]">expand_more</span>
              </div>
            </div>
          </div>

          <!-- SQLite File Path -->
          <div v-if="isSQLite" class="space-y-1.5">
            <label class="block text-sm font-medium text-gray-700 dark:text-white">Database File Path</label>
            <input
              v-model="host"
              class="w-full h-11 px-3.5 bg-gray-50 dark:bg-[#1d2526] border border-gray-200 dark:border-[#3d4f51] rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-primary/50 focus:border-primary outline-none transition-all placeholder:text-gray-400 dark:placeholder:text-[#5d6f71]"
              placeholder="/path/to/database.db"
              type="text"
            />
            <p class="text-xs text-gray-500 dark:text-[#5d6f71] mt-1">
              Enter the full path to your SQLite database file
            </p>
          </div>

          <!-- Host & Port Row (for non-SQLite) -->
          <div v-else class="flex gap-4">
            <div class="flex-[3] space-y-1.5">
              <label class="block text-sm font-medium text-gray-700 dark:text-white">Host</label>
              <input
                v-model="host"
                class="w-full h-11 px-3.5 bg-gray-50 dark:bg-[#1d2526] border border-gray-200 dark:border-[#3d4f51] rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-primary/50 focus:border-primary outline-none transition-all placeholder:text-gray-400 dark:placeholder:text-[#5d6f71]"
                placeholder="localhost"
                type="text"
              />
            </div>
            <div class="flex-[1] space-y-1.5">
              <label class="block text-sm font-medium text-gray-700 dark:text-white">Port</label>
              <input
                v-model="port"
                class="w-full h-11 px-3.5 bg-gray-50 dark:bg-[#1d2526] border border-gray-200 dark:border-[#3d4f51] rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-primary/50 focus:border-primary outline-none transition-all placeholder:text-gray-400 dark:placeholder:text-[#5d6f71]"
                :placeholder="port"
                type="text"
              />
            </div>
          </div>

          <!-- Auth Row (for non-SQLite) -->
          <div v-if="!isSQLite" class="flex gap-4">
            <div class="flex-1 space-y-1.5">
              <label class="block text-sm font-medium text-gray-700 dark:text-white">Username</label>
              <input
                v-model="username"
                class="w-full h-11 px-3.5 bg-gray-50 dark:bg-[#1d2526] border border-gray-200 dark:border-[#3d4f51] rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-primary/50 focus:border-primary outline-none transition-all placeholder:text-gray-400 dark:placeholder:text-[#5d6f71]"
                placeholder="postgres"
                type="text"
              />
            </div>
            <div class="flex-1 space-y-1.5">
              <label class="block text-sm font-medium text-gray-700 dark:text-white">Password</label>
              <input
                v-model="password"
                class="w-full h-11 px-3.5 bg-gray-50 dark:bg-[#1d2526] border border-gray-200 dark:border-[#3d4f51] rounded-lg text-gray-900 dark:text-white focus:ring-2 focus:ring-primary/50 focus:border-primary outline-none transition-all placeholder:text-gray-400 dark:placeholder:text-[#5d6f71]"
                placeholder="••••••••"
                type="password"
              />
            </div>
          </div>

          <!-- SSL/TLS Toggle (for non-SQLite) -->
          <div v-if="!isSQLite" class="flex items-center justify-between pt-2">
            <div class="flex flex-col">
              <span class="text-sm font-medium text-gray-700 dark:text-white flex items-center gap-1.5">
                SSL/TLS Required
                <span class="material-symbols-outlined text-[16px] text-gray-400 cursor-help">info</span>
              </span>
              <span class="text-xs text-gray-500 dark:text-[#9fb4b7]">Encrypt connection to the database server</span>
            </div>
            <div class="relative inline-block w-11 h-6 align-middle select-none transition duration-200 ease-in">
              <input
                v-model="sslEnabled"
                class="toggle-checkbox absolute block w-6 h-6 rounded-full bg-white border-4 appearance-none cursor-pointer outline-none focus:ring-0 transition-all"
                :class="
                  sslEnabled
                    ? 'right-0 border-primary'
                    : 'left-0 border-gray-200 dark:border-[#3d4f51]'
                "
                id="toggle"
                type="checkbox"
              />
              <label
                class="toggle-label block overflow-hidden h-6 rounded-full cursor-pointer transition-colors"
                :class="sslEnabled ? 'bg-primary' : 'bg-gray-200 dark:bg-[#1d2526]'"
                for="toggle"
              ></label>
            </div>
          </div>
        </div>

        <!-- Footer -->
        <footer
          class="px-8 py-6 bg-gray-50/50 dark:bg-[#1d2526]/50 border-t border-gray-100 dark:border-[#2a3637] flex items-center justify-between"
        >
          <div class="flex items-center gap-3">
             <button
              @click="handleTestConnection"
              :disabled="isTesting"
              class="flex items-center gap-2 px-4 h-10 rounded-lg text-sm font-bold text-gray-600 dark:text-[#9fb4b7] hover:text-primary dark:hover:text-primary transition-colors border border-gray-200 dark:border-[#3d4f51] bg-white dark:bg-[#1d2526] disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <span class="material-symbols-outlined text-[18px]" :class="isTesting ? 'animate-spin' : ''">
                {{ isTesting ? 'progress_activity' : 'network_check' }}
              </span>
              Test Connection
            </button>
            <span
              v-if="testStatus !== 'idle'"
              class="text-xs font-medium"
              :class="testStatus === 'success' ? 'text-emerald-500' : 'text-red-500'"
            >
              {{ testMessage }}
            </span>
          </div>
         
          <div class="flex items-center gap-3">
            <button
              @click="handleCancel"
              class="px-4 h-10 text-sm font-medium text-gray-500 dark:text-[#9fb4b7] hover:text-gray-700 dark:hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              @click="handleConnect"
              class="px-6 h-10 bg-primary hover:bg-primary/90 text-white rounded-lg text-sm font-bold tracking-wide transition-all active:scale-95 flex items-center gap-2"
            >
              Connect
              <span class="material-symbols-outlined text-[18px]">arrow_forward</span>
            </button>
          </div>
        </footer>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.toggle-checkbox {
  transition: all 0.2s ease;
}
</style>

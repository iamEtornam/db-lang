<script setup lang="ts">
import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

const engine = ref("postgres");
const host = ref("localhost");
const port = ref(5432);
const user = ref("user");
const password = ref("password");
const dbname = ref("db");
const connectionStatus = ref("");
const isConnecting = ref(false);

const connectionString = computed(() => {
  switch (engine.value) {
    case "postgres":
      return `postgresql://${user.value}:${password.value}@${host.value}:${port.value}/${dbname.value}`;
    case "mysql":
      return `mysql://${user.value}:${password.value}@${host.value}:${port.value}/${dbname.value}`;
    case "sqlite":
      return dbname.value;
    default:
      return "";
  }
});

async function connect() {
  connectionStatus.value = "Connecting...";
  isConnecting.value = true;
  try {
    // A simple SELECT 1 is a good way to test the connection without fetching data
    await invoke("query_db", {
      engine: engine.value,
      connStr: connectionString.value,
      query: "SELECT 1",
    });
    connectionStatus.value = "Connected";
  } catch (err) {
    connectionStatus.value = `Error: ${(err as string).substring(0, 100)}...`;
  } finally {
    isConnecting.value = false;
  }
}
</script>

<template>
  <div class="db-connector">
    <div class="form-group">
      <label for="engine">Engine</label>
      <select id="engine" v-model="engine">
        <option value="postgres">PostgreSQL</option>
        <option value="mysql">MySQL</option>
        <option value="sqlite">SQLite</option>
      </select>
    </div>
    <div class="form-group">
      <label for="host">Host</label>
      <input id="host" v-model="host" type="text" />
    </div>
    <div class="form-group">
      <label for="port">Port</label>
      <input id="port" v-model="port" type="number" />
    </div>
    <div class="form-group">
      <label for="user">User</label>
      <input id="user" v-model="user" type="text" />
    </div>
    <div class="form-group">
      <label for="password">Password</label>
      <input id="password" v-model="password" type="password" />
    </div>
    <div class="form-group">
      <label for="dbname">Database / File Path</label>
      <input id="dbname" v-model="dbname" type="text" />
    </div>
    <button @click="connect" :disabled="isConnecting">
      {{ isConnecting ? 'Connecting...' : 'Test Connection' }}
    </button>
    <div v-if="connectionStatus" class="connection-status">
      {{ connectionStatus }}
    </div>
  </div>
</template>

<style scoped>
.db-connector {
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
}

label {
  margin-bottom: 8px;
  font-size: 0.875rem;
  font-weight: 500;
  color: var(--text-secondary-light);
}

input,
select {
  width: 100%;
  padding: 10px;
  border: 1px solid var(--border-light);
  border-radius: 8px;
  background-color: var(--bg-light);
  color: var(--text-primary-light);
  font-family: inherit;
  font-size: 1rem;
  box-sizing: border-box;
  transition: border-color 0.2s, box-shadow 0.2s;
}

input:focus,
select:focus {
  outline: none;
  border-color: var(--accent-light);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.3);
}

button {
  width: 100%;
  padding: 12px;
  border: none;
  border-radius: 8px;
  background-color: var(--accent-light);
  color: white;
  cursor: pointer;
  transition: background-color 0.2s;
  font-size: 1rem;
  font-weight: 500;
}

button:hover {
  background-color: var(--accent-hover-light);
}

button:disabled {
  background-color: #a0a0a0;
  cursor: not-allowed;
}

.connection-status {
  margin-top: 10px;
  font-weight: 500;
  font-size: 0.875rem;
  text-align: center;
  color: var(--text-secondary-light);
}

@media (prefers-color-scheme: dark) {
  label {
    color: var(--text-secondary-dark);
  }

  input,
  select {
    border-color: var(--border-dark);
    background-color: var(--bg-dark);
    color: var(--text-primary-dark);
  }

  input:focus,
  select:focus {
    border-color: var(--accent-dark);
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.3);
  }
  
  button {
    background-color: var(--accent-dark);
  }

  button:hover {
    background-color: var(--accent-hover-dark);
  }

  button:disabled {
    background-color: #555;
  }

  .connection-status {
    color: var(--text-secondary-dark);
  }
}
</style>

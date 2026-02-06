<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useSnippetsStore, type Snippet } from '../stores/snippets';

const emit = defineEmits<{
  close: [];
  selectSnippet: [snippet: Snippet];
}>();

const snippetsStore = useSnippetsStore();

const searchTerm = ref('');
const selectedTag = ref<string | null>(null);
const showCreateForm = ref(false);
const editingSnippet = ref<Snippet | null>(null);

// Form state
const formName = ref('');
const formDescription = ref('');
const formNaturalQuery = ref('');
const formSqlQuery = ref('');
const formTags = ref('');

onMounted(() => {
  snippetsStore.loadSnippets();
});

const displayedSnippets = computed(() => {
  let results = snippetsStore.snippets;
  
  if (searchTerm.value.trim()) {
    results = snippetsStore.searchSnippets(searchTerm.value);
  }
  
  if (selectedTag.value) {
    results = results.filter(s => 
      s.tags.toLowerCase().split(',').map(t => t.trim()).includes(selectedTag.value!.toLowerCase())
    );
  }
  
  return results;
});

function resetForm() {
  formName.value = '';
  formDescription.value = '';
  formNaturalQuery.value = '';
  formSqlQuery.value = '';
  formTags.value = '';
  editingSnippet.value = null;
  showCreateForm.value = false;
}

function startEdit(snippet: Snippet) {
  editingSnippet.value = snippet;
  formName.value = snippet.name;
  formDescription.value = snippet.description || '';
  formNaturalQuery.value = snippet.natural_query;
  formSqlQuery.value = snippet.sql_query;
  formTags.value = snippet.tags;
  showCreateForm.value = true;
}

async function handleSave() {
  if (!formName.value.trim() || !formSqlQuery.value.trim()) {
    return;
  }

  if (editingSnippet.value) {
    // Update existing
    await snippetsStore.updateSnippet({
      id: editingSnippet.value.id,
      name: formName.value,
      description: formDescription.value || null,
      natural_query: formNaturalQuery.value,
      sql_query: formSqlQuery.value,
      tags: formTags.value || null,
    });
  } else {
    // Create new
    await snippetsStore.createSnippet({
      name: formName.value,
      description: formDescription.value || null,
      natural_query: formNaturalQuery.value,
      sql_query: formSqlQuery.value,
      tags: formTags.value || null,
    });
  }

  resetForm();
}

async function handleDelete(snippetId: string) {
  if (confirm('Delete this snippet?')) {
    await snippetsStore.deleteSnippet(snippetId);
  }
}

function selectSnippet(snippet: Snippet) {
  emit('selectSnippet', snippet);
  emit('close');
}

function formatDate(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleDateString();
}
</script>

<template>
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4" @click.self="emit('close')">
    <div class="w-full max-w-3xl bg-surface-dark rounded-xl shadow-[0_20px_70px_rgba(0,0,0,0.55)] overflow-hidden border border-[#2a3637] flex flex-col max-h-[80vh]">
      <!-- Header -->
      <header class="flex items-center justify-between px-6 py-4 border-b border-[#2a3637]">
        <div class="flex items-center gap-3">
          <span class="material-symbols-outlined text-primary text-[20px]">bookmark</span>
          <h2 class="text-lg font-bold text-white">Saved Snippets</h2>
        </div>
        <div class="flex items-center gap-2">
          <button
            @click="showCreateForm = true; editingSnippet = null"
            class="flex items-center gap-2 px-3 py-1.5 bg-primary hover:bg-primary/90 rounded-lg text-white text-sm font-medium transition-colors"
          >
            <span class="material-symbols-outlined text-[16px]">add</span>
            New Snippet
          </button>
          <button @click="emit('close')" class="p-1 hover:bg-white/5 rounded-lg transition-colors">
            <span class="material-symbols-outlined text-[#9fb4b7] text-[20px]">close</span>
          </button>
        </div>
      </header>

      <!-- Create/Edit Form -->
      <div v-if="showCreateForm" class="px-6 py-4 border-b border-[#2a3637] bg-[#1d2526]">
        <div class="flex flex-col gap-4">
          <div class="flex gap-4">
            <div class="flex-1">
              <label class="block text-xs font-medium text-[#9fb4b7] mb-1">Name *</label>
              <input
                v-model="formName"
                class="w-full h-9 px-3 bg-surface-dark border border-[#3d4f51] rounded-lg text-white placeholder:text-[#5d6f71] focus:border-primary focus:ring-0 outline-none text-sm"
                placeholder="My Query"
              />
            </div>
            <div class="flex-1">
              <label class="block text-xs font-medium text-[#9fb4b7] mb-1">Tags (comma-separated)</label>
              <input
                v-model="formTags"
                class="w-full h-9 px-3 bg-surface-dark border border-[#3d4f51] rounded-lg text-white placeholder:text-[#5d6f71] focus:border-primary focus:ring-0 outline-none text-sm"
                placeholder="users, analytics"
              />
            </div>
          </div>
          <div>
            <label class="block text-xs font-medium text-[#9fb4b7] mb-1">Description</label>
            <input
              v-model="formDescription"
              class="w-full h-9 px-3 bg-surface-dark border border-[#3d4f51] rounded-lg text-white placeholder:text-[#5d6f71] focus:border-primary focus:ring-0 outline-none text-sm"
              placeholder="What does this query do?"
            />
          </div>
          <div>
            <label class="block text-xs font-medium text-[#9fb4b7] mb-1">Natural Language Query</label>
            <input
              v-model="formNaturalQuery"
              class="w-full h-9 px-3 bg-surface-dark border border-[#3d4f51] rounded-lg text-white placeholder:text-[#5d6f71] focus:border-primary focus:ring-0 outline-none text-sm"
              placeholder="Find all users who..."
            />
          </div>
          <div>
            <label class="block text-xs font-medium text-[#9fb4b7] mb-1">SQL Query *</label>
            <textarea
              v-model="formSqlQuery"
              class="w-full h-24 px-3 py-2 bg-surface-dark border border-[#3d4f51] rounded-lg text-white placeholder:text-[#5d6f71] focus:border-primary focus:ring-0 outline-none text-sm font-mono resize-none"
              placeholder="SELECT * FROM users WHERE..."
            ></textarea>
          </div>
          <div class="flex justify-end gap-2">
            <button
              @click="resetForm"
              class="px-4 py-2 text-[#9fb4b7] hover:text-white text-sm font-medium transition-colors"
            >
              Cancel
            </button>
            <button
              @click="handleSave"
              :disabled="!formName.trim() || !formSqlQuery.trim() || snippetsStore.isLoading"
              class="px-4 py-2 bg-primary hover:bg-primary/90 rounded-lg text-white text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {{ editingSnippet ? 'Update' : 'Save' }}
            </button>
          </div>
        </div>
      </div>

      <!-- Search and Tags -->
      <div class="px-6 py-4 border-b border-[#2a3637]">
        <div class="flex gap-3 mb-3">
          <div class="relative flex-1">
            <input
              v-model="searchTerm"
              class="w-full h-10 pl-10 pr-4 bg-[#1d2526] border border-[#3d4f51] rounded-lg text-white placeholder:text-[#5d6f71] focus:border-primary focus:ring-0 outline-none text-sm"
              placeholder="Search snippets..."
            />
            <span class="material-symbols-outlined absolute left-3 top-1/2 -translate-y-1/2 text-[#5d6f71] text-[18px]">search</span>
          </div>
        </div>
        <div v-if="snippetsStore.allTags.length > 0" class="flex flex-wrap gap-2">
          <button
            @click="selectedTag = null"
            class="px-2 py-1 text-xs rounded-full transition-colors"
            :class="selectedTag === null ? 'bg-primary text-white' : 'bg-[#1d2526] text-[#9fb4b7] hover:text-white'"
          >
            All
          </button>
          <button
            v-for="tag in snippetsStore.allTags"
            :key="tag"
            @click="selectedTag = selectedTag === tag ? null : tag"
            class="px-2 py-1 text-xs rounded-full transition-colors"
            :class="selectedTag === tag ? 'bg-primary text-white' : 'bg-[#1d2526] text-[#9fb4b7] hover:text-white'"
          >
            {{ tag }}
          </button>
        </div>
      </div>

      <!-- Snippets List -->
      <div class="flex-1 overflow-y-auto">
        <div v-if="snippetsStore.isLoading && snippetsStore.snippets.length === 0" class="p-8 text-center text-[#9fb4b7]">
          <span class="material-symbols-outlined animate-spin text-3xl text-primary mb-2">progress_activity</span>
          <p>Loading snippets...</p>
        </div>

        <div v-else-if="displayedSnippets.length === 0" class="p-8 text-center text-[#5d6f71]">
          <span class="material-symbols-outlined text-4xl mb-3">bookmark_border</span>
          <p v-if="searchTerm || selectedTag">No matching snippets found</p>
          <p v-else>No saved snippets yet</p>
        </div>

        <div v-else class="divide-y divide-[#2a3637]">
          <div
            v-for="snippet in displayedSnippets"
            :key="snippet.id"
            class="px-6 py-4 hover:bg-white/5 transition-colors group"
          >
            <div class="flex items-start justify-between gap-4 mb-2">
              <div class="flex-1">
                <h3 class="text-white text-sm font-medium">{{ snippet.name }}</h3>
                <p v-if="snippet.description" class="text-xs text-[#5d6f71] mt-0.5">{{ snippet.description }}</p>
              </div>
              <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  @click="selectSnippet(snippet)"
                  class="p-1.5 hover:bg-primary/20 rounded text-primary transition-colors"
                  title="Use this snippet"
                >
                  <span class="material-symbols-outlined text-[16px]">play_arrow</span>
                </button>
                <button
                  @click="startEdit(snippet)"
                  class="p-1.5 hover:bg-white/10 rounded text-[#9fb4b7] hover:text-white transition-colors"
                  title="Edit"
                >
                  <span class="material-symbols-outlined text-[16px]">edit</span>
                </button>
                <button
                  @click="handleDelete(snippet.id)"
                  class="p-1.5 hover:bg-red-500/20 rounded text-[#9fb4b7] hover:text-red-400 transition-colors"
                  title="Delete"
                >
                  <span class="material-symbols-outlined text-[16px]">delete</span>
                </button>
              </div>
            </div>
            <pre class="text-xs text-[#9fb4b7] font-mono bg-[#1d2526] rounded px-3 py-2 overflow-x-auto line-clamp-2 mb-2 cursor-pointer" @click="selectSnippet(snippet)">{{ snippet.sql_query }}</pre>
            <div class="flex items-center gap-3">
              <div v-if="snippet.tags" class="flex flex-wrap gap-1">
                <span
                  v-for="tag in snippet.tags.split(',').map(t => t.trim()).filter(Boolean)"
                  :key="tag"
                  class="px-1.5 py-0.5 text-[10px] bg-primary/20 text-primary rounded"
                >
                  {{ tag }}
                </span>
              </div>
              <span class="text-[10px] text-[#5d6f71] uppercase tracking-wider">{{ formatDate(snippet.updated_at) }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

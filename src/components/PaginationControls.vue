<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  currentPage: number;
  pageSize: number;
  totalCount: number | null;
  hasMore: boolean;
  loading?: boolean;
}>();

const emit = defineEmits<{
  pageChange: [page: number];
  pageSizeChange: [size: number];
}>();

const pageSizeOptions = [10, 25, 50, 100, 200];

const totalPages = computed(() => {
  if (props.totalCount === null) return null;
  return Math.ceil(props.totalCount / props.pageSize);
});

const startRow = computed(() => {
  return (props.currentPage - 1) * props.pageSize + 1;
});

const endRow = computed(() => {
  if (props.totalCount === null) {
    return props.currentPage * props.pageSize;
  }
  return Math.min(props.currentPage * props.pageSize, props.totalCount);
});

const canGoPrevious = computed(() => props.currentPage > 1);
const canGoNext = computed(() => {
  if (props.totalCount !== null) {
    return props.currentPage < (totalPages.value || 0);
  }
  return props.hasMore;
});

const visiblePages = computed(() => {
  if (totalPages.value === null) return [];
  
  const pages: (number | string)[] = [];
  const total = totalPages.value;
  const current = props.currentPage;
  
  if (total <= 7) {
    for (let i = 1; i <= total; i++) pages.push(i);
  } else {
    pages.push(1);
    
    if (current > 3) pages.push('...');
    
    const start = Math.max(2, current - 1);
    const end = Math.min(total - 1, current + 1);
    
    for (let i = start; i <= end; i++) {
      if (!pages.includes(i)) pages.push(i);
    }
    
    if (current < total - 2) pages.push('...');
    
    if (!pages.includes(total)) pages.push(total);
  }
  
  return pages;
});

function goToPage(page: number) {
  if (page >= 1 && (totalPages.value === null || page <= totalPages.value)) {
    emit('pageChange', page);
  }
}

function changePageSize(event: Event) {
  const target = event.target as HTMLSelectElement;
  emit('pageSizeChange', Number(target.value));
}
</script>

<template>
  <div class="flex items-center justify-between py-3 px-4 bg-background-dark/50 border-t border-[#2a3637]">
    <!-- Info -->
    <div class="flex items-center gap-4">
      <span class="text-sm text-[#9fb4b7]">
        <template v-if="totalCount !== null">
          Showing {{ startRow.toLocaleString() }}-{{ endRow.toLocaleString() }} of {{ totalCount.toLocaleString() }} rows
        </template>
        <template v-else>
          Page {{ currentPage }}
        </template>
      </span>
      
      <div class="flex items-center gap-2">
        <span class="text-xs text-[#5d6f71]">Rows per page:</span>
        <select
          :value="pageSize"
          @change="changePageSize"
          class="bg-surface-dark border border-[#3d4f51] rounded px-2 py-1 text-xs text-white focus:outline-none focus:border-primary"
        >
          <option v-for="size in pageSizeOptions" :key="size" :value="size">
            {{ size }}
          </option>
        </select>
      </div>
    </div>

    <!-- Navigation -->
    <div class="flex items-center gap-1">
      <!-- First page -->
      <button
        @click="goToPage(1)"
        :disabled="!canGoPrevious || loading"
        class="p-1.5 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed hover:bg-white/10"
        title="First page"
      >
        <span class="material-symbols-outlined text-[18px] text-[#9fb4b7]">first_page</span>
      </button>

      <!-- Previous -->
      <button
        @click="goToPage(currentPage - 1)"
        :disabled="!canGoPrevious || loading"
        class="p-1.5 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed hover:bg-white/10"
        title="Previous page"
      >
        <span class="material-symbols-outlined text-[18px] text-[#9fb4b7]">chevron_left</span>
      </button>

      <!-- Page numbers -->
      <template v-if="totalPages !== null">
        <template v-for="(page, idx) in visiblePages" :key="idx">
          <span v-if="page === '...'" class="px-2 text-[#5d6f71]">...</span>
          <button
            v-else
            @click="goToPage(page as number)"
            :disabled="loading"
            class="min-w-[32px] h-8 px-2 rounded-lg text-sm font-medium transition-colors disabled:cursor-not-allowed"
            :class="page === currentPage ? 'bg-primary text-white' : 'text-[#9fb4b7] hover:bg-white/10'"
          >
            {{ page }}
          </button>
        </template>
      </template>
      <template v-else>
        <span class="px-4 py-1.5 bg-primary/20 text-primary rounded-lg text-sm font-medium">
          {{ currentPage }}
        </span>
      </template>

      <!-- Next -->
      <button
        @click="goToPage(currentPage + 1)"
        :disabled="!canGoNext || loading"
        class="p-1.5 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed hover:bg-white/10"
        title="Next page"
      >
        <span class="material-symbols-outlined text-[18px] text-[#9fb4b7]">chevron_right</span>
      </button>

      <!-- Last page -->
      <button
        v-if="totalPages !== null"
        @click="goToPage(totalPages)"
        :disabled="!canGoNext || loading"
        class="p-1.5 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed hover:bg-white/10"
        title="Last page"
      >
        <span class="material-symbols-outlined text-[18px] text-[#9fb4b7]">last_page</span>
      </button>
    </div>

    <!-- Loading indicator -->
    <div v-if="loading" class="absolute inset-0 bg-black/30 flex items-center justify-center">
      <div class="w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin"></div>
    </div>
  </div>
</template>

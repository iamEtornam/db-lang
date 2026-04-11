<script setup lang="ts">
import { SidebarProvider, SidebarInset } from '~/components/ui/sidebar'
</script>

<template>
  <SidebarProvider>
    <LayoutAppSidebar />
    <SidebarInset>
      <LayoutHeader />
      <main class="flex flex-1 flex-col overflow-hidden p-4 pt-0">
        <slot />
      </main>
    </SidebarInset>
  </SidebarProvider>
</template>

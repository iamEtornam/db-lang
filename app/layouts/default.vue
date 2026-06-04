<script setup lang="ts">
import { SidebarProvider, SidebarInset } from '~/components/ui/sidebar'
</script>

<template>
  <!-- h-svh pins the shell to the viewport so app-shell pages (index, schema)
       can bound their internal scroll regions. Without it the wrapper's
       min-h-svh lets the whole document grow, so a tall page scrolls as one
       block instead of its panes scrolling independently. The <main> is the
       scroll container for grow-style pages (history, settings); app-shell
       pages stay h-full + overflow-hidden and never push it past the fold. -->
  <SidebarProvider class="h-svh">
    <LayoutAppSidebar />
    <SidebarInset>
      <LayoutHeader />
      <main class="flex flex-1 flex-col min-h-0 overflow-y-auto overflow-x-hidden p-4 pt-0">
        <slot />
      </main>
    </SidebarInset>
  </SidebarProvider>
</template>

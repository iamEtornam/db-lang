<script setup lang="ts">
import { SidebarMenuButton } from '~/components/ui/sidebar'

const colorMode = useColorMode()

function toggle() {
  colorMode.preference = colorMode.value === 'dark' ? 'light' : 'dark'
}
</script>

<template>
  <SidebarMenuButton :tooltip="colorMode.value === 'dark' ? 'Light mode' : 'Dark mode'" @click="toggle">
    <Icon :name="colorMode.value === 'dark' ? 'lucide:sun' : 'lucide:moon'" />
    <span>{{ colorMode.value === 'dark' ? 'Light mode' : 'Dark mode' }}</span>
  </SidebarMenuButton>
</template>

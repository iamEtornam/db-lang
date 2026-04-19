<script setup lang="ts">
import { Toaster } from 'vue-sonner'

const colorMode = useColorMode()

useHead({
  titleTemplate: '%s | QueryStudio',
  htmlAttrs: {
    lang: 'en',
  },
  meta: [
    { name: 'description', content: 'AI-powered database management tool' },
  ],
})

const { maybeCheckOnBoot, loadCurrentVersion } = useAppUpdater()

onMounted(() => {
  loadCurrentVersion()
  maybeCheckOnBoot().catch((err) => {
    console.warn('Background update check failed', err)
  })
})
</script>

<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
  <Toaster
    position="bottom-right"
    :duration="4000"
    :theme="colorMode.value === 'dark' ? 'dark' : 'light'"
    close-button
    rich-colors
  />
</template>

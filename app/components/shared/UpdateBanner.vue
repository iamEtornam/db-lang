<script setup lang="ts">
import { Button } from '~/components/ui/button'

const {
  state,
  progress,
  latestVersion,
  errorMessage,
  installAndRelaunch,
  dismiss,
} = useAppUpdater()

const visible = computed(() =>
  state.value === 'available'
  || state.value === 'downloading'
  || state.value === 'ready'
  || state.value === 'error',
)

const label = computed(() => {
  switch (state.value) {
    case 'available':
      return `Update available — Query Studio ${latestVersion.value}`
    case 'downloading':
      return `Downloading update… ${progress.value}%`
    case 'ready':
      return 'Update ready — restart to apply'
    case 'error':
      return errorMessage.value || 'Update failed'
    default:
      return ''
  }
})

const tone = computed(() =>
  state.value === 'error'
    ? 'border-destructive/40 bg-destructive/10 text-destructive'
    : 'border-primary/30 bg-primary/10 text-primary',
)

const cta = computed(() => {
  switch (state.value) {
    case 'available':
      return 'Install & restart'
    case 'downloading':
      return null
    case 'ready':
      return 'Restart now'
    case 'error':
      return 'Retry'
    default:
      return null
  }
})

async function onCta() {
  if (state.value === 'error') {
    await useAppUpdater().checkForUpdate()
    return
  }
  await installAndRelaunch()
}
</script>

<template>
  <div
    v-if="visible"
    class="flex items-center gap-2 rounded-md border px-2.5 py-1 text-xs"
    :class="tone"
    role="status"
  >
    <Icon
      v-if="state === 'downloading'"
      name="lucide:loader-2"
      class="size-3.5 animate-spin"
    />
    <Icon
      v-else-if="state === 'ready'"
      name="lucide:check-circle-2"
      class="size-3.5"
    />
    <Icon
      v-else-if="state === 'error'"
      name="lucide:alert-triangle"
      class="size-3.5"
    />
    <Icon v-else name="lucide:download" class="size-3.5" />

    <span class="font-medium">{{ label }}</span>

    <Button
      v-if="cta"
      size="sm"
      variant="ghost"
      class="h-6 px-2 text-xs"
      @click="onCta"
    >
      {{ cta }}
    </Button>

    <button
      v-if="state !== 'downloading'"
      class="text-current/70 hover:text-current"
      aria-label="Dismiss"
      @click="dismiss"
    >
      <Icon name="lucide:x" class="size-3.5" />
    </button>
  </div>
</template>

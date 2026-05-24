import { defineConfig } from 'vitest/config'
import { fileURLToPath } from 'node:url'

// Vitest config for pure-function tests in app/lib. Mirrors Nuxt's `~/`
// alias so test imports look the same as the runtime ones.
//
// No DOM environment needed yet — all the lib/* modules are framework-
// agnostic. If component tests get added later, switch `environment` to
// `'jsdom'` or `'happy-dom'`.
export default defineConfig({
  resolve: {
    alias: {
      '~': fileURLToPath(new URL('./app', import.meta.url)),
      '@': fileURLToPath(new URL('./app', import.meta.url)),
    },
  },
  test: {
    include: ['app/**/*.{test,spec}.ts'],
    environment: 'node',
  },
})

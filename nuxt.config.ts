import tailwindcss from '@tailwindcss/vite'

export default defineNuxtConfig({
  ssr: false,

  compatibilityDate: '2025-01-01',

  devtools: { enabled: true },

  future: {
    compatibilityVersion: 4,
  },

  modules: [
    'shadcn-nuxt',
    '@vueuse/nuxt',
    '@pinia/nuxt',
    '@nuxtjs/color-mode',
    '@nuxt/fonts',
    '@nuxt/icon',
  ],

  shadcn: {
    prefix: '',
    componentDir: './app/components/ui',
  },

  colorMode: {
    classSuffix: '',
    preference: 'dark',
    fallback: 'dark',
  },

  css: ['~/assets/css/tailwind.css'],

  vite: {
    plugins: [tailwindcss()],
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
    server: {
      strictPort: true,
    },
    optimizeDeps: {
      include: [
        '@vue/devtools-core',
        '@vue/devtools-kit',
        'vue-sonner',
        'reka-ui',
        '@tauri-apps/api/core',
        '@tauri-apps/api/event',
        'class-variance-authority',
        'clsx',
        'tailwind-merge',
        '@radix-icons/vue',
      ],
    },
  },

  fonts: {
    families: [
      { name: 'Geist', provider: 'google' },
      { name: 'Geist Mono', provider: 'google' },
    ],
  },

  icon: {
    // Tauri serves a static bundle — no server, no CDN access.
    // All icons must be bundled into the client JS at build time.
    serverBundle: false,
    // Disable CDN fallback: CSP in tauri.conf.json blocks api.iconify.design,
    // and failed fetches trigger uncaught promise rejections that blank the page.
    fallbackToApi: false,
    clientBundle: {
      scan: true,
      // Explicitly include icons resolved dynamically (e.g. from engines.ts constants)
      // so the static scanner doesn't miss them.
      icons: [
        'simple-icons:postgresql',
        'simple-icons:mysql',
        'simple-icons:mariadb',
        'simple-icons:sqlite',
        'simple-icons:mongodb',
        'simple-icons:redis',
        'simple-icons:firebase',
        'lucide:database',
      ],
    },
  },
})

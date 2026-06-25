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
    // Render icons as inline <svg>, not via runtime-injected CSS classes.
    // CSS mode looks up icons asynchronously (loadIcon -> initClientBundle ->
    // mountCSS), and the dynamic <style> injection is unreliable in Tauri
    // release webviews — icons end up as empty <span>s. SVG mode runs
    // initClientBundle synchronously in setup() and renders inline SVG, so
    // bundled icons render the first frame.
    mode: 'svg',
    clientBundle: {
      scan: true,
      // The scanner only globs **/*.{vue,jsx,tsx,md,mdc,mdx,yml,yaml} — it
      // does NOT scan plain .ts files. Icons defined only in app/constants/*.ts
      // (e.g. engines.ts) must be listed explicitly here.
      icons: [
        'simple-icons:postgresql',
        'simple-icons:mysql',
        'simple-icons:mariadb',
        'simple-icons:sqlite',
        'simple-icons:mongodb',
        'simple-icons:redis',
        'simple-icons:firebase',
        'lucide:database',
        // Charts page nav entry — referenced from menus.ts (a .ts file the
        // icon scanner skips), so it must be listed explicitly.
        'lucide:bar-chart-3',
        'lucide:scroll-text',
        'lucide:play',
        'lucide:lock',
      ],
    },
  },
})

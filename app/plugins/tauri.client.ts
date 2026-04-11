// Client-only plugin to initialize Tauri-specific functionality
export default defineNuxtPlugin(() => {
  // Prevent context menu in production (desktop app behavior)
  if (process.env.NODE_ENV === 'production') {
    document.addEventListener('contextmenu', (e) => {
      e.preventDefault()
    })
  }
})

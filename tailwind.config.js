/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        "primary": "#1b8e9d",
        "background-light": "#f7f7f7",
        "background-dark": "#1a1a1a",
        "mac-border": "rgba(0,0,0,0.08)",
        "mac-glass": "rgba(255,255,255,0.7)",
      },
      fontFamily: {
        "display": ["Space Grotesk"]
      },
      borderRadius: {
        "DEFAULT": "0.25rem",
        "lg": "0.5rem",
        "xl": "0.75rem",
        "full": "9999px"
      },
    },
  },
  plugins: [
    require('@tailwindcss/forms'),
    require('@tailwindcss/container-queries'),
  ],
}

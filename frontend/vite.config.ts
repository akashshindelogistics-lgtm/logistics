import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// `base` is the public path the app is served under. Locally that's `/`; the
// GitHub Pages build sets VITE_BASE to the repo subpath (e.g. `/logistics/`).
// It flows through to `import.meta.env.BASE_URL`, which the router uses as its
// basename and Vite uses to rewrite asset URLs.
const base = process.env.VITE_BASE || '/'

export default defineConfig({
  base,
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      '/api': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
})

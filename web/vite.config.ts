import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// In dev, proxy the live-play WebSocket and the REST history API to the Axum
// backend so the SPA talks to the same origin it will in production.
const BACKEND = 'http://localhost:8080'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/ws': { target: BACKEND, ws: true, changeOrigin: true },
      '/api': { target: BACKEND, changeOrigin: true },
    },
  },
  // Backend serves this from web/dist with SPA fallback to index.html.
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
})

import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// During dev, the frontend talks to the backend via the `/api` proxy below so there
// are no CORS concerns. Override the backend target with VITE_BACKEND if needed.
const backend = process.env.VITE_BACKEND ?? 'http://127.0.0.1:8787'

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    port: 5174,
    proxy: {
      '/api': {
        target: backend,
        changeOrigin: true,
      },
    },
  },
})

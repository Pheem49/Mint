import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  base: './',
  build: {
    outDir: resolve(__dirname, 'out/renderer'),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        index: resolve(__dirname, 'src/renderer/index-web.html')
      }
    }
  },
  server: {
    port: 9000,
    strictPort: true,
    fs: {
      allow: [
        resolve(__dirname)
      ]
    }
  },
  plugins: [react()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src/renderer/src-web'),
      '@shared': resolve(__dirname, 'src')
    }
  }
})

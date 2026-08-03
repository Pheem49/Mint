// @ts-nocheck
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  // Must be root-relative (not './'): the SPA fallback serves index-web.html at
  // deep paths like /chat/<id>, and a relative base would resolve ./assets/...
  // against that path (e.g. /chat/assets/...) instead of the site root, 404ing
  // every script/style and leaving a blank page on direct navigation/refresh.
  base: '/',
  cacheDir: resolve(__dirname, '.vite-web'),
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
    host: true,
    strictPort: true,
    fs: {
      allow: [
        resolve(__dirname)
      ]
    }
  },
  preview: {
    port: 9000,
    host: true,
    strictPort: true,
  },
  optimizeDeps: {
    force: true,
  },
  plugins: [
    react(),
    {
      name: 'rewrite-html',
      configureServer(server) {
        server.middlewares.use((req, res, next) => {
          // Skip Vite's own dev endpoints (/@vite/client, /@react-refresh, /@id/, /@fs/, ...):
          // they have no dot-extension either, so without this check the SPA fallback
          // below would rewrite them to index-web.html, breaking the HMR client and
          // leaving a blank page.
          if (
            req.url &&
            !req.url.startsWith('/api') &&
            !req.url.startsWith('/@') &&
            !/\.[a-zA-Z0-9]+(\?.*)?$/.test(req.url)
          ) {
            req.url = '/index-web.html'
          }
          next()
        })
      },
      configurePreviewServer(server) {
        server.middlewares.use((req, res, next) => {
          if (req.url && !req.url.startsWith('/api') && !/\.[a-zA-Z0-9]+(\?.*)?$/.test(req.url)) {
            req.url = '/index-web.html'
          }
          next()
        })
      }
    }
  ],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src/renderer/src-web'),
      '@shared': resolve(__dirname, 'src')
    }
  }
})

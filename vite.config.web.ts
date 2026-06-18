import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  base: './',
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
  optimizeDeps: {
    force: true,
  },
  plugins: [
    react(),
    {
      name: 'rewrite-html',
      configureServer(server) {
        server.middlewares.use((req, res, next) => {
          if (req.url === '/' || req.url === '/index.html') {
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

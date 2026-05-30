import { defineConfig, externalizeDepsPlugin } from 'electron-vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  main: {
    entry: 'src/main/index.ts',
    resolve: {
      alias: {
        '@shared': resolve(__dirname, 'src')
      }
    },
    plugins: [externalizeDepsPlugin()]
  },
  preload: {
    input: {
      index: resolve(__dirname, 'src/preload/index.ts'),
      settings: resolve(__dirname, 'src/preload/settings.ts'),
      picker: resolve(__dirname, 'src/preload/picker.ts'),
      spotlight: resolve(__dirname, 'src/preload/spotlight.ts'),
      widget: resolve(__dirname, 'src/preload/widget.ts')
    },
    plugins: [externalizeDepsPlugin()]
  },
  renderer: {
    root: resolve(__dirname, 'src/renderer'),
    build: {
      rollupOptions: {
        input: {
          index: resolve(__dirname, 'src/renderer/index.html')
        }
      }
    },
    plugins: [react()]
  }
})

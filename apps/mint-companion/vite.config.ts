import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
export default defineConfig({ base: './', plugins: [react()], build: { outDir: '../../out/companion', emptyOutDir: true }, server: { port: 9001, strictPort: true } })

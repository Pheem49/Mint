import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
export default defineConfig({ base: './', plugins: [react(), tailwindcss()], build: { outDir: '../../out/companion', emptyOutDir: true }, server: { port: 9001, strictPort: true } })

import { createRequire } from 'node:module'
import { dirname, resolve, sep } from 'node:path'
import { cpSync, createReadStream } from 'node:fs'
import { stat } from 'node:fs/promises'

const packageRoot = dirname(createRequire(import.meta.url).resolve('pdfjs-dist/package.json'))
const directories = ['cmaps', 'standard_fonts', 'wasm']

/** Bundle PDF fonts/decoders locally, and expose the same read-only assets in Vite dev. */
export function pdfjsAssets() {
  let output
  return {
    name: 'mint-pdfjs-assets',
    configResolved(config) { output = resolve(config.root, config.build.outDir, 'pdfjs') },
    configureServer(server) {
      server.middlewares.use('/pdfjs/', async (request, response, next) => {
        try {
          const relative = decodeURIComponent(new URL(request.url || '/', 'http://localhost').pathname).replace(/^\/+/, '')
          const target = resolve(packageRoot, relative)
          if (!directories.some(dir => target.startsWith(resolve(packageRoot, dir) + sep))) return next()
          const info = await stat(target)
          if (!info.isFile()) return next()
          response.setHeader('Content-Length', info.size)
          response.setHeader('Content-Type', target.endsWith('.wasm') ? 'application/wasm' : 'application/octet-stream')
          createReadStream(target).on('error', () => response.destroy()).pipe(response)
        } catch { next() }
      })
    },
    closeBundle() { for (const directory of directories) cpSync(resolve(packageRoot, directory), resolve(output, directory), { recursive: true }) },
  }
}

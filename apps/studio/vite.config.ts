import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { requestCore, MAX_REQUEST_BYTES, coreTimeout } from '../../tools/core-client.mjs'

export default defineConfig({
  plugins: [react(), {
    name: 'aislide-local-core',
    configureServer(server) {
      server.middlewares.use(async (request, response, next) => {
        if (request.url !== '/api/core') return next()
        const address = server.httpServer?.address()
        const port = typeof address === 'object' && address ? address.port : server.config.server.port
        const host = `127.0.0.1:${port}`
        if (request.method !== 'POST' || request.headers.host !== host || request.headers.origin !== `http://${host}` || !request.headers['content-type']?.startsWith('application/json')) {
          response.writeHead(403).end('Forbidden')
          return
        }
        const controller = new AbortController()
        request.once('aborted', () => controller.abort())
        response.once('close', () => { if (!response.writableFinished) controller.abort() })
        request.setTimeout(15_000, () => request.destroy())
        try {
          let length = 0
          const chunks: Buffer[] = []
          for await (const chunk of request) {
            const buffer = Buffer.from(chunk)
            length += buffer.length
            if (length > MAX_REQUEST_BYTES) throw new Error('Request exceeds 4 MiB')
            chunks.push(buffer)
          }
          const value = JSON.parse(Buffer.concat(chunks).toString('utf8'))
          request.setTimeout(coreTimeout(value) + 5000, () => { controller.abort(); request.destroy() })
          const result = await requestCore(value, { signal: controller.signal })
          response.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }).end(JSON.stringify(result))
        } catch (error) {
          response.writeHead(400, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }).end(JSON.stringify({ error: error instanceof Error ? error.message : 'Core request failed' }))
        }
      })
    },
  }],
  server: { host: '127.0.0.1', port: 4173, strictPort: true, watch: { ignored: ['**/src-tauri/target/**'] } },
  build: { target: 'es2022' },
})

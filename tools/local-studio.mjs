import { createServer } from 'vite';
import { resolve } from 'node:path';
import { startLocalModel } from './local-model-runtime.mjs';

const model = await startLocalModel();
Object.assign(process.env, model.environment);
let studio;
let closing = false;
async function close() {
  if (closing) return;
  closing = true;
  await studio?.close(); await model.stop();
}
try {
  studio = await createServer({ root: resolve('apps/studio'), configFile: resolve('apps/studio/vite.config.ts'), server: { host: '127.0.0.1', port: 0, strictPort: true } });
  await studio.listen();
  const port = studio.httpServer.address().port;
  console.log(`AISlide with real local Qwen2.5-1.5B: http://127.0.0.1:${port}/`);
  console.log('Loopback-only model. No cloud credentials or global environment settings changed. Ctrl+C closes both owned servers.');
  process.once('SIGINT', () => { void close(); }); process.once('SIGTERM', () => { void close(); });
  model.server.once('exit', () => { if (!closing) { console.error('Local model stopped; closing its Studio instance.'); void close(); } });
} catch (error) { await close(); throw error; }
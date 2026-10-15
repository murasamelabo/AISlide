import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { once } from 'node:events';
import { resolve } from 'node:path';

export async function startLocalModel() {
  if (process.platform !== 'win32') throw new Error('The prepared local model runtime targets Windows');
  const reservation = createServer(); reservation.listen(0, '127.0.0.1'); await once(reservation, 'listening');
  const port = reservation.address().port;
  await new Promise((done, reject) => reservation.close((error) => error ? reject(error) : done()));
  const server = spawn(resolve('.tools/local-model/runtime/llama-server.exe'), [
    '--model', resolve('.tools/local-model/qwen2.5-1.5b-instruct-q4_k_m.gguf'), '--host', '127.0.0.1', '--port', String(port),
    '--alias', 'aislide-qwen-local', '--ctx-size', '12288', '--parallel', '1', '--threads', '6', '--temp', '0', '--no-webui',
  ], { stdio: ['ignore', 'pipe', 'pipe'], shell: false, windowsHide: true });
  const closed = new Promise((done) => server.once('close', done));
  const stop = async () => { if (server.exitCode === null && server.signalCode === null) server.kill(); await closed; };
  let log = ''; let timer;
  try {
    await new Promise((ready, reject) => {
      const consume = (bytes) => { log = (log + bytes.toString()).slice(-64000); if (/server is listening|listening on http/i.test(log)) { clearTimeout(timer); ready(); } };
      server.stdout.on('data', consume); server.stderr.on('data', consume);
      server.once('error', reject); server.once('exit', (code) => reject(new Error(`Local server exited during startup: ${code}`)));
      timer = setTimeout(() => reject(new Error('Local model startup exceeded 60 seconds')), 60000);
    });
    const response = await fetch(`http://127.0.0.1:${port}/health`, { signal: AbortSignal.timeout(5000) });
    if (!response.ok) throw new Error(`Local model health returned HTTP ${response.status}`);
    return { server, stop, get log() { return log; }, environment: {
      AISLIDE_AI_BASE_URL: `http://127.0.0.1:${port}/v1`, AISLIDE_AI_MODEL: 'aislide-qwen-local', AISLIDE_AI_API_KEY: '',
      AISLIDE_AI_ALLOW_REMOTE: '0', AISLIDE_AI_JSON_MODE: 'schema', AISLIDE_AI_TIMEOUT_SECONDS: '300',
    } };
  } catch (error) { await stop(); throw error; } finally { clearTimeout(timer); }
}
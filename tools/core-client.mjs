import { execFile } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const MAX_REQUEST_BYTES = 4 * 1024 * 1024;
export function coreTimeout(request) {
  return request?.op === 'generate' ? 310_000 : 20_000;
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const binary = join(root, 'target', 'debug', process.platform === 'win32' ? 'aislide.exe' : 'aislide');
let active = false;

export async function requestCore(request, { signal } = {}) {
  if (signal?.aborted) throw new Error('Operation cancelled');
  const payload = JSON.stringify(request);
  if (!payload || Buffer.byteLength(payload) > MAX_REQUEST_BYTES) throw new Error('Request exceeds the 4 MiB limit');
  if (active) throw new Error('Core is busy; retry after the current operation');
  active = true;
  try {
    return await new Promise((resolveResponse, reject) => {
      const child = execFile(binary, ['request'], {
        cwd: root,
        encoding: 'utf8',
        maxBuffer: MAX_REQUEST_BYTES,
        timeout: coreTimeout(request),
        killSignal: 'SIGKILL',
        windowsHide: true,
        signal,
      }, (error, stdout, stderr) => {
        if (error) {
          let message = error.name === 'AbortError' ? 'Operation cancelled' : 'Core execution failed. Run npm run core:build.';
          if (stderr) {
            try { message = JSON.parse(stderr).error ?? message; } catch { message = 'Core failed without a valid error response'; }
          }
          reject(new Error(message));
          return;
        }
        try { resolveResponse(JSON.parse(stdout)); }
        catch { reject(new Error('Core returned invalid JSON')); }
      });
      child.stdin?.on('error', () => {});
      child.stdin?.end(payload);
    });
  } finally {
    active = false;
  }
}
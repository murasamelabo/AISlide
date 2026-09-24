import { execFile } from 'node:child_process';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CAPACITY_PROFILES, encodeCoreRequest } from '../packages/client/index.mjs';

export const MAX_REQUEST_BYTES = CAPACITY_PROFILES.large.request_bytes;
export function coreTimeout(request, encodedBytes = 0) {
  if (['generate', 'text_assist', 'segment_image'].includes(request?.op)) return 310_000;
  const base = encodedBytes > 4 * 1048576 || ['verify_session_recovery', 'prepare_recovery', 'verify_recovery_record'].includes(request?.op) ? 120_000 : 20_000;
  const managed = request?.op === 'apply_operations' && Array.isArray(request.operations)
    ? request.operations.slice(0, 128).filter(operation => ['add_part', 'update_part', 'add_graph', 'update_graph'].includes(operation?.op)).length
    : ['insert_part', 'update_part', 'insert_graph', 'update_graph', 'apply_graph'].includes(request?.op) ? 1 : 0;
  return managed ? Math.min(300_000, Math.max(base, 60_000 + managed * 5_000)) : base;
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
export function coreBinary(selected = process.env.AISLIDE_CORE_BINARY) {
  if (!selected) return join(root, 'target', 'debug', process.platform === 'win32' ? 'aislide.exe' : 'aislide');
  if (!isAbsolute(selected) || /^[\\/]{2}/.test(selected) || process.platform === 'win32' && !/^[A-Za-z]:[\\/]/.test(selected)) throw new Error('AISLIDE_CORE_BINARY requires an absolute local path');
  return selected;
}
const binary = coreBinary();
const active = new Set();

export async function requestCore(request, { signal } = {}) {
  if (signal?.aborted) throw new Error('Operation cancelled');
  const payload = encodeCoreRequest(request);
  const timeout = coreTimeout(request, Buffer.byteLength(payload));
  const worker = ['render_element_preview', 'compute_chart_presentation'].includes(request?.op) ? 'chart-preview' : ['prepare_recovery', 'verify_recovery_record', 'verify_session_recovery'].includes(request?.op) ? 'recovery' : 'editing';
  if (active.has(worker)) throw new Error('Core is busy; retry after the current operation');
  active.add(worker);
  try {
    return await new Promise((resolveResponse, reject) => {
      const child = execFile(binary, ['request'], {
        cwd: root,
        encoding: 'utf8',
        maxBuffer: MAX_REQUEST_BYTES + 1,
        timeout,
        killSignal: 'SIGKILL',
        windowsHide: true,
        signal,
      }, (error, stdout, stderr) => {
        if (error) {
          let message = error.name === 'AbortError' ? 'Operation cancelled' : 'Core execution failed. Run npm run core:build.';
          if (error.killed && error.name !== 'AbortError') message = `Core operation timed out after ${timeout / 1000} seconds; the session was not committed`;
          if (error.code === 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER') message = 'Core response exceeded the finite JSON wire budget';
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
    active.delete(worker);
  }
}
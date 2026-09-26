import { execFile } from 'node:child_process';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CAPACITY_PROFILES, encodeCoreRequest } from '../packages/client/index.mjs';

export const MAX_REQUEST_BYTES = CAPACITY_PROFILES.large.request_bytes;
function documentWork(request) {
  const deck = request?.document?.deck ?? request?.deck;
  const slides = Array.isArray(deck?.slides) ? deck.slides : [];
  const pending = slides.slice(0, 256).map(slide => ({ elements: Array.isArray(slide?.elements) ? slide.elements : [], index: 0 }));
  let elements = 0;
  while (pending.length && elements < 8192) {
    const current = pending.at(-1);
    if (current.index >= current.elements.length) { pending.pop(); continue; }
    const element = current.elements[current.index++];
    elements++;
    if (Array.isArray(element?.children) && element.children.length) pending.push({ elements: element.children, index: 0 });
  }
  return { slides: Math.min(slides.length, 256), elements };
}

export function coreTimeout(request, encodedBytes = 0) {
  if (['generate', 'text_assist', 'segment_image'].includes(request?.op)) return 310_000;
  const work = documentWork(request);
  const bytes = Number.isFinite(encodedBytes) && encodedBytes > 0 ? encodedBytes : 0;
  const units = Math.max(Math.ceil(work.slides / 8), Math.ceil(work.elements / 256), Math.ceil(bytes / 1048576));
  const documentBudget = units > 1 ? Math.min(180_000, 20_000 + units * 10_000) : 20_000;
  const recoveryBudget = bytes > 4 * 1048576 || ['verify_session_recovery', 'prepare_recovery', 'verify_recovery_record'].includes(request?.op) ? 120_000 : 20_000;
  const pages = Math.max(1, Math.min(8, Array.isArray(request?.options?.page_indices) ? request.options.page_indices.length : work.slides || 1));
  const operationBudget = request?.op === 'preview_presentation' ? 60_000 + (pages - 1) * 5_000 : ['open_presentation', 'import_document'].includes(request?.op) ? 60_000 : 20_000;
  const compositeBudget = ['preview_slide_revision', 'prepare_delivery'].includes(request?.op) ? 120_000 : 20_000;
  const base = Math.max(documentBudget, recoveryBudget, operationBudget, compositeBudget);
  const managed = request?.op === 'apply_operations' && Array.isArray(request.operations)
    ? request.operations.slice(0, 128).filter(operation => ['add_part', 'update_part', 'add_graph', 'update_graph'].includes(operation?.op)).length
    : ['insert_part', 'update_part', 'insert_graph', 'update_graph', 'apply_graph', 'set_accessibility'].includes(request?.op) ? 1 : 0;
  return managed ? Math.min(300_000, Math.max(base, 60_000 + managed * 5_000)) : base;
}

export function coreTimeoutMessage(request, limit, elapsedMs) {
  const elapsed = (Math.round(elapsedMs / 100) / 10).toFixed(1);
  const timing = `timed out after ${elapsed} seconds elapsed (configured limit ${limit / 1000} seconds)`;
  if (request?.op === 'preview_presentation') return `Core preview ${timing}; preview interrupted; no document changes`;
  if (['open_presentation', 'import_document'].includes(request?.op)) return `Core PPTX open ${timing}; no presentation was opened; existing documents were not changed`;
  if (['measure_layout', 'preflight_presentation', 'preview_slide_revision', 'prepare_delivery', 'validate', 'validate_document', 'check_accessibility', 'render_element_preview', 'compute_chart_presentation', 'export_static'].includes(request?.op)) return `Core read-only operation ${timing}; no document changes`;
  return `Core operation ${timing}; the session was not committed`;
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
  const started = performance.now();
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
          let message = 'Core execution failed. Run npm run core:build.';
          if (error.name === 'AbortError') { message = 'Operation cancelled'; }
          else if (error.code === 'ERR_CHILD_PROCESS_STDIO_MAXBUFFER') { message = 'Core response exceeded the finite JSON wire budget'; }
          else if (error.killed) { message = coreTimeoutMessage(request, timeout, performance.now() - started); }
          else if (stderr) {
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
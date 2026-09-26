import test from 'node:test';
import assert from 'node:assert/strict';
import { requestCore, coreTimeout, MAX_REQUEST_BYTES } from './core-client.mjs';
import { CAPACITY_PROFILES } from '../packages/client/index.mjs';

test('managed batch timeout scales with bounded work without changing ordinary requests', () => {
  const batch = count => ({ op: 'apply_operations', operations: Array.from({ length: count }, () => ({ op: 'add_part' })) });
  assert.equal(coreTimeout({ op: 'sample' }), 20000);
  assert.equal(coreTimeout({ op: 'apply_operations', operations: [{ op: 'set_frame' }] }), 20000);
  assert.equal(coreTimeout({ op: 'transaction' }, 4 * 1048576 + 1), 120000);
  assert.equal(coreTimeout({ op: 'verify_session_recovery' }), 120000);
  assert.equal(coreTimeout(batch(1)), 65000);
  assert.equal(coreTimeout(batch(21)), 165000);
  assert.equal(coreTimeout(batch(128)), 300000);
  assert.equal(coreTimeout(batch(129)), 300000);
  assert.equal(coreTimeout(batch(1), 5 * 1048576), 120000);
  for (const op of ['insert_part', 'update_part', 'insert_graph', 'update_graph', 'apply_graph']) assert.equal(coreTimeout({ op }), 65000);
  for (const op of ['generate', 'text_assist', 'segment_image']) assert.equal(coreTimeout({ op }), 310000);
  assert.equal(coreTimeout({ op: 'apply_operations', operations: null }), 20000);
  assert.equal(coreTimeout({ op: 'sample', timeout: Infinity }), 20000);
});

test('accessibility updates receive a finite authoring budget without changing ordinary requests', () => {
  assert.equal(coreTimeout({ op: 'set_accessibility' }), 65000);
  assert.equal(coreTimeout({ op: 'set_accessibility' }, 5 * 1048576), 120000);
  assert.equal(coreTimeout({ op: 'set_accessibility', timeout: Infinity }), 65000);
  assert.equal(coreTimeout({ op: 'sample' }), 20000);
});

test('document work and read-only rendering receive finite size-aware time budgets', () => {
  const document = { deck: { slides: Array.from({ length: 42 }, () => ({ elements: Array.from({ length: 32 }, () => ({ type: 'text' })) })) } };
  for (const op of ['transaction', 'apply_operations', 'measure_layout', 'preflight_presentation']) assert.equal(coreTimeout({ op, document }, 3653348), 80000, op);
  assert.equal(coreTimeout({ op: 'apply_operations', document, operations: [{ op: 'add_graph' }] }), 80000);
  assert.equal(coreTimeout({ op: 'open_presentation' }, 1461087), 60000);
  assert.equal(coreTimeout({ op: 'import_document' }, 1048576), 60000);
  assert.equal(coreTimeout({ op: 'preview_presentation', document, options: { page_indices: [0] } }), 80000);
  assert.equal(coreTimeout({ op: 'preview_presentation', document, options: { page_indices: [0,1,2,3,4,5,6,7] } }), 95000);
  assert.equal(coreTimeout({ op: 'preview_presentation', options: { page_indices: [0] } }), 60000);
  for (const op of ['preview_slide_revision', 'prepare_delivery']) assert.equal(coreTimeout({ op }), 120000, op);
  const nested = { deck: { slides: [{ elements: [{ type: 'group', children: Array.from({ length: 1344 }, () => ({ type: 'text' })) }] }] } };
  assert.equal(coreTimeout({ op: 'transaction', document: nested }), 80000);
  assert.equal(coreTimeout({ op: 'transaction', document: { deck: { slides: Array.from({ length: 10000 }, () => ({ elements: [] })) } } }), 180000);
  assert.equal(coreTimeout({ op: 'transaction' }, 96 * 1048576), 180000);
  assert.equal(coreTimeout({ op: 'sample' }, Number.NaN), 20000);
  assert.equal(coreTimeout({ op: 'transaction', document, timeout: Infinity }), 80000);
  assert.equal(coreTimeout({ op: 'apply_operations', document, operations: Array.from({ length: 128 }, () => ({ op: 'add_graph' })) }), 300000);
});

test('timeout diagnostics separate elapsed time limits and read-only outcomes', async () => {
  const bridge = await import('./core-client.mjs');
  assert.equal(typeof bridge.coreTimeoutMessage, 'function');
  const preview = bridge.coreTimeoutMessage({ op: 'preview_presentation' }, 20000, 29526);
  assert.match(preview, /preview.*timed out after 29\.5 seconds elapsed.*limit 20 seconds/i);
  assert.match(preview, /preview interrupted.*no document changes/i);
  assert.doesNotMatch(preview, /session was not committed/);
  const opened = bridge.coreTimeoutMessage({ op: 'open_presentation' }, 60000, 62500);
  assert.match(opened, /62\.5 seconds elapsed.*limit 60 seconds/);
  assert.match(opened, /no presentation was opened/);
  assert.match(bridge.coreTimeoutMessage({ op: 'import_document' }, 60000, 62500), /no presentation was opened/);
  const changed = bridge.coreTimeoutMessage({ op: 'transaction' }, 80000, 81350);
  assert.match(changed, /81\.4 seconds elapsed.*limit 80 seconds.*session was not committed/);
  for (const op of ['measure_layout', 'preflight_presentation', 'preview_slide_revision', 'prepare_delivery', 'validate']) assert.match(bridge.coreTimeoutMessage({ op }, 80000, 80000), /no document changes/);
});

test('an operator can select an isolated local core binary without request-controlled paths', async () => {
  const bridge = await import('./core-client.mjs');
  assert.equal(typeof bridge.coreBinary, 'function');
  const selected = process.platform === 'win32' ? 'C:\\isolated\\aislide.exe' : '/isolated/aislide';
  assert.equal(bridge.coreBinary(selected), selected);
  assert.match(bridge.coreBinary(''), /target[\\/]debug[\\/]aislide/);
  for (const value of ['relative/aislide', '\\\\server\\share\\aislide.exe', '//server/share/aislide', 'https://example.test/aislide']) {
    assert.throws(() => bridge.coreBinary(value), /absolute local path/i);
  }
});

test('the bridge returns the real Rust sample', async () => {
  const report = await requestCore({ op: 'sample' });
  assert.equal(report.sections.length, 12);
  assert.deepEqual(await requestCore({ op: 'capacity_profiles' }), { default: 'large', ...CAPACITY_PROFILES });
});

test('the bridge compiles and exports with the shared core', async () => {
  const report = await requestCore({ op: 'sample' });
  const { deck } = await requestCore({ op: 'compile', report });
  const result = await requestCore({ op: 'export', deck });
  assert.equal(Buffer.from(result.base64, 'base64').subarray(0, 2).toString(), 'PK');
});

test('the bridge rejects oversized inputs and already-cancelled work', async () => {
  await assert.rejects(requestCore({ payload: 'a'.repeat(MAX_REQUEST_BYTES) }), /limit|large/i);
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(requestCore({ op: 'sample' }, { signal: controller.signal }), /cancel|abort/i);
});

test('the bridge surfaces a Rust protocol error', async () => {
  await assert.rejects(requestCore({ op: 'execute_shell' }), /unknown variant|unknown operation/i);
});

test('phase5 background recovery has one isolated worker and does not block normal editing requests', async () => {
  const request = { op: 'prepare_recovery', state: { version: 2, enabled: false, generation: 0, consent_epoch: 0, entries: [] }, expected_generation: 0, action: { op: 'open' }, now_ms: 1000 };
  const pending = requestCore(request);
  const editing = requestCore({ op: 'sample' });
  const competing = assert.rejects(requestCore(request), /busy/i);
  const [result, report] = await Promise.all([pending, editing, competing]);
  assert.equal(result.state.enabled, false);
  assert.equal(report.sections.length, 12);
});

test('phase3 read-only preview has a bounded separate worker without blocking editing', async () => {
  const input = { op: 'render_element_preview', element: { type: 'text', id: 'wordart', x: 0, y: 0, width: 300, height: 180, text: 'office affinity', font_size: 30, bold: true, color: '087F73', visual: { text_warp: 'wave2' } } };
  const pending = requestCore(input);
  const normal = requestCore({ op: 'sample' });
  const competing = assert.rejects(requestCore(input), /busy/i);
  const [preview, report] = await Promise.all([pending, normal, competing]);
  assert.ok(preview.svg.includes('<path'));
  assert.equal(preview.office_parity_verified, false);
  assert.equal(report.sections.length, 12);
});
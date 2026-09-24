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
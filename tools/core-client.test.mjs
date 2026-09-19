import test from 'node:test';
import assert from 'node:assert/strict';
import { requestCore, MAX_REQUEST_BYTES } from './core-client.mjs';
import { CAPACITY_PROFILES } from '../packages/client/index.mjs';

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
import test from 'node:test';
import assert from 'node:assert/strict';
import { requestCore, MAX_REQUEST_BYTES } from './core-client.mjs';

test('the bridge returns the real Rust sample', async () => {
  const report = await requestCore({ op: 'sample' });
  assert.equal(report.sections.length, 12);
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
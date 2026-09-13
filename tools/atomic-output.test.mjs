import test from 'node:test';
import assert from 'node:assert/strict';
import { publishNewFile } from './atomic-output.mjs';

test('failed write still closes and removes temporary output', async () => {
  const actions = [];
  const io = {
    open: async () => ({ writeFile: async () => { throw new Error('disk full'); }, sync: async () => {}, close: async () => { actions.push('close'); } }),
    link: async () => { actions.push('link'); },
    unlink: async () => { actions.push('unlink'); },
  };
  await assert.rejects(publishNewFile('temp', 'output', Buffer.from('data'), undefined, io), /disk full/);
  assert.deepEqual(actions, ['close', 'unlink']);
});

test('collision never removes an existing destination', async () => {
  const removed = [];
  const io = {
    open: async () => ({ writeFile: async () => {}, sync: async () => {}, close: async () => {} }),
    link: async () => { throw new Error('already exists'); },
    unlink: async (path) => { removed.push(path); },
  };
  await assert.rejects(publishNewFile('temp', 'output', Buffer.from('data'), undefined, io), /already exists/);
  assert.deepEqual(removed, ['temp']);
});
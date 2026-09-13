import test from 'node:test';
import assert from 'node:assert/strict';
import * as filesystem from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { publishProject } from './atomic-project.mjs';

const bundle = { base64: Buffer.from('new pptx bytes').toString('base64'), checkpoint: { fixture: true } };
test('an existing checkpoint prevents any project publication and preserves existing data', async () => {
  const directory = await filesystem.mkdtemp(join(tmpdir(), 'aislide-pair-'));
  try {
    await filesystem.writeFile(join(directory, 'report.aislide.json'), 'keep existing');
    await assert.rejects(() => publishProject(directory, 'report.pptx', bundle));
    assert.deepEqual(await filesystem.readdir(directory), ['report.aislide.json']);
    assert.equal(await filesystem.readFile(join(directory, 'report.aislide.json'), 'utf8'), 'keep existing');
  } finally { await filesystem.rm(directory, { recursive: true, force: true }); }
});
test('failed staging and pre-cancellation publish no project files', async () => {
  const directory = await filesystem.mkdtemp(join(tmpdir(), 'aislide-pair-'));
  try {
    let opened = 0;
    const io = { ...filesystem, open: async (...args) => {
      const file = await filesystem.open(...args); opened += 1;
      if (opened === 2) return { writeFile: async () => { throw new Error('injected write failure'); }, close: () => file.close() };
      return file;
    } };
    await assert.rejects(() => publishProject(directory, 'report.pptx', bundle, undefined, io), /injected write failure/);
    assert.deepEqual(await filesystem.readdir(directory), []);
    const controller = new AbortController(); controller.abort();
    await assert.rejects(() => publishProject(directory, 'report.pptx', bundle, controller.signal), /cancelled/);
    assert.deepEqual(await filesystem.readdir(directory), []);
  } finally { await filesystem.rm(directory, { recursive: true, force: true }); }
});
test('a complete pair is published without overwriting either file', async () => {
  const directory = await filesystem.mkdtemp(join(tmpdir(), 'aislide-pair-'));
  try {
    const saved = await publishProject(directory, 'report.pptx', bundle);
    assert.equal(await filesystem.readFile(saved.path, 'utf8'), 'new pptx bytes');
    assert.deepEqual(JSON.parse(await filesystem.readFile(saved.checkpoint_path, 'utf8')), bundle.checkpoint);
    await assert.rejects(() => publishProject(directory, 'report.pptx', bundle));
    assert.deepEqual((await filesystem.readdir(directory)).sort(), ['report.aislide.json', 'report.pptx']);
  } finally { await filesystem.rm(directory, { recursive: true, force: true }); }
});

test('a racing destination is never deleted during partial publication recovery', async () => {
  const directory = await filesystem.mkdtemp(join(tmpdir(), 'aislide-pair-race-'));
  try {
    let linked = false;
    const io = { ...filesystem, link: async (temporary, destination) => {
      await filesystem.link(temporary, destination);
      if (!linked) {
        linked = true;
        await filesystem.writeFile(join(directory, 'report.aislide.json'), 'concurrent checkpoint', { flag: 'wx' });
      }
    } };
    await assert.rejects(() => publishProject(directory, 'report.pptx', bundle, undefined, io), /partial/i);
    assert.equal(await filesystem.readFile(join(directory, 'report.pptx'), 'utf8'), 'new pptx bytes');
    assert.equal(await filesystem.readFile(join(directory, 'report.aislide.json'), 'utf8'), 'concurrent checkpoint');
    assert.deepEqual((await filesystem.readdir(directory)).sort(), ['report.aislide.json', 'report.pptx']);
  } finally { await filesystem.rm(directory, { recursive: true, force: true }); }
});
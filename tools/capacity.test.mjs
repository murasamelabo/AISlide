import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir, mkdir } from 'node:fs/promises';
import { resolve, join, extname } from 'node:path';
import { performance } from 'node:perf_hooks';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { AislideClient, DocumentSession, CAPACITY_PROFILES, encodeCoreRequest } from '../packages/client/index.mjs';
import { requestCore, coreTimeout } from './core-client.mjs';

function report(slides) {
  return { title: 'Synthetic G38 capacity', subtitle: '', period: 'Synthetic fixture', source: 'Synthetic data, not a factual report',
    sections: Array.from({ length: slides }, (_, index) => ({ title: `Synthetic page ${index + 1}`, layout: 'statement', body: ['A bounded, editable text slide.'] })) };
}

test('phase5 SDK restores both history directions and validates profile changes atomically', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('session-recovery', 'Start', { capacityProfile: 'standard' });
  assert.equal(session.capacityProfile, 'standard');
  await session.transact([{ op: 'replace', path: '/deck/title', value: 'First' }]);
  await session.transact([{ op: 'replace', path: '/deck/title', value: 'Second' }]);
  await session.undo();
  const snapshot = session.recoveryEnvelope;
  const restored = await client.recoverSession(snapshot);
  assert.deepEqual(restored.document, session.document);
  assert.equal(restored.capacityProfile, 'standard');
  assert.equal(restored.canUndo, true); assert.equal(restored.canRedo, true);
  await restored.redo(); assert.equal(restored.document.deck.title, 'Second');
  await restored.undo(); await restored.undo(); assert.equal(restored.document.deck.title, 'Start');
  snapshot.past[0].after_hash = '0'.repeat(64);
  await assert.rejects(client.recoverSession(snapshot), /receipt/i);
  const { deck } = await client.request({ op: 'compile', report: report(64) });
  const medium = await client.createDocument({ id: 'medium-profile', deck }, { capacityProfile: 'standard' });
  const before = medium.document;
  await assert.rejects(medium.setCapacityProfile('legacy'), /32|slides/i);
  assert.equal(medium.capacityProfile, 'standard'); assert.deepEqual(medium.document, before);
  await medium.setCapacityProfile('large'); assert.equal(medium.capacityProfile, 'large');
});

test('phase5 SDK history uses UTF8 hard limits and never exposes a broken chain', async () => {
  const initial = { id: 'history-budget', revision: 0, hash: 'initial', deck: {} };
  let calls = 0;
  const session = new DocumentSession(async () => {
    calls += 1;
    return { document: { ...initial, revision: calls, hash: `hash-${calls}` }, receipt: {
      document_id: initial.id, after_hash: `hash-${calls}`, inverse: [{ op: 'replace', path: '/deck/title', value: calls === 2 ? '界'.repeat(1_400_000) : 'before' }],
    } };
  }, initial);
  await session.transact([]); assert.equal(session.canUndo, true);
  await session.transact([]); assert.equal(session.canUndo, false);
  assert.equal(session.historyBoundary, 'history_limit');
  assert.equal(session.recoveryEnvelope.past.length, 0);
  await session.transact([]); assert.equal(session.canUndo, true);
  assert.equal(session.recoveryEnvelope.past.length, 1);
});

test('phase5 SDK late cancelled profile selection preserves the selected profile', async () => {
  let finish;
  const session = new DocumentSession(() => new Promise(resolveReply => { finish = resolveReply; }), { id: 'profile-cancel', revision: 0, hash: 'initial', deck: {} });
  const controller = new AbortController();
  const pending = session.setCapacityProfile('standard', { signal: controller.signal });
  const rejected = assert.rejects(pending, /cancelled/i);
  controller.abort(); finish(session.recoveryEnvelope); await rejected;
  assert.equal(session.capacityProfile, 'large');
});

test('G38 fixed profiles match core and reject unbounded transport input', async () => {
  const client = new AislideClient(requestCore);
  assert.deepEqual(await client.capacityProfiles(), { default: 'large', ...CAPACITY_PROFILES });
  for (const capacity_profile of ['unlimited', null, { slides: 100000 }]) {
    assert.throws(() => encodeCoreRequest({ op: 'sample', capacity_profile }), /capacity profile/);
    await assert.rejects(client.request({ op: 'sample', capacity_profile }), /capacity profile/);
  }
  assert.throws(() => encodeCoreRequest({ op: 'sample', capacity_profile: 'legacy', extra: 'a'.repeat(4194304) }), /limit/);
  assert.throws(() => encodeCoreRequest({ extra: Array(250001).fill(null) }), /node/);
  const cyclic = {}; cyclic.self = cyclic;
  assert.throws(() => encodeCoreRequest(cyclic), /depth/);
  assert.equal(coreTimeout({ op: 'validate' }), 20000);
  assert.equal(coreTimeout({ op: 'transaction' }, 4 * 1048576 + 1), 120000);
  assert.equal(coreTimeout({ op: 'verify_session_recovery' }), 120000);
  const controller = new AbortController(); controller.abort();
  await assert.rejects(requestCore({ op: 'sample' }, { signal: controller.signal }), /cancelled/);
});

test('G38 real CLI SDK lifecycle measures 64 and 128 synthetic slides', { timeout: 180000 }, async () => {
  const client = new AislideClient(requestCore);
  for (const count of [64, 128]) {
    const timings = {};
    const measure = async (label, action) => { const started = performance.now(); const result = await action(); timings[label] = Math.round(performance.now() - started); return result; };
    const { deck } = await measure('compile_ms', () => requestCore({ op: 'compile', report: report(count) }));
    const session = await measure('create_ms', () => client.createDocument({ id: `g38-${count}`, deck }));
    const original = session.document;
    await measure('edit_ms', () => session.transact([{ op: 'replace', path: '/deck/title', value: 'Synthetic changed' }]));
    await measure('undo_ms', () => session.undo());
    assert.equal(session.document.hash, original.hash);
    await session.redo(); await session.undo();
    const exported = await measure('export_ms', () => session.exportPresentation());
    assert.equal(exported.checkpoint, undefined);
    const reopened = await measure('open_ms', () => client.openPresentation(`g38-open-${count}`, exported.base64));
    assert.equal(reopened.session.document.deck.slides.length, count);
    await measure('native_edit_ms', () => reopened.session.transact([{ op: 'replace', path: '/deck/slides/0/elements/2/text', value: 'Synthetic edit' }]));
    await reopened.session.undo();
    assert.equal((await reopened.session.exportPresentation()).base64, exported.base64);
    await assert.rejects(requestCore({ op: 'validate', deck, capacity_profile: 'legacy' }), /slides/);
    console.log('G38_MEASURE', JSON.stringify({ slides: count, document_bytes: Buffer.byteLength(JSON.stringify(original)), reopened_document_bytes: Buffer.byteLength(JSON.stringify(reopened.session.document)), pptx_bytes: Buffer.from(exported.base64, 'base64').length, timings }));
  }
});

test('G38 MCP accepts a medium scene and returns large results once', { timeout: 120000 }, async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'g38-offline-proof', version: '1.0.0' });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, JSON.stringify(result.content)); return result; };
  const data = response => JSON.parse(response.content[0].text);
  try {
    await client.connect(transport);
    assert.deepEqual(data(await call('capacity_profiles')), { default: 'large', ...CAPACITY_PROFILES });
    const created = data(await call('compile_report', { report: report(64), capacity_profile: 'standard' }));
    const response = await call('get_deck', { deck_id: created.deck_id });
    assert.equal(data(response).slides.length, 64);
    assert.ok(Buffer.byteLength(response.content[0].text) > 65536);
    assert.equal(response.structuredContent, undefined);
    const deck = data(response);
    deck.slides = Array.from({ length: 128 }, (_, index) => ({ ...structuredClone(deck.slides[index % 64]), id: `medium-${index}` }));
    const changed = data(await call('replace_deck', { deck_id: created.deck_id, expected_revision: 0, deck }));
    assert.equal(data(await call('get_deck', { deck_id: created.deck_id })).slides.length, 128);
    deck.slides.push({ ...structuredClone(deck.slides[0]), id: 'overflow' });
    assert.equal((await client.callTool({ name: 'replace_deck', arguments: { deck_id: created.deck_id, expected_revision: changed.revision, deck } })).isError, true);
    assert.equal(data(await call('get_deck', { deck_id: created.deck_id })).slides.length, 128);
    await call('edit_slides', { deck_id: created.deck_id, expected_revision: changed.revision, operations: [{ op: 'move', slide_id: 'medium-0', index: 127 }] });
    assert.equal(data(await call('get_deck', { deck_id: created.deck_id })).slides[127].id, 'medium-0');
    await call('undo', { deck_id: created.deck_id });
    await call('undo', { deck_id: created.deck_id });
    assert.equal(data(await call('get_deck', { deck_id: created.deck_id })).slides.length, 64);
  } finally { await client.close(); }
});

test('phase5 built Studio opens edits undoes and saves 256 slides without a network server', { timeout: 180000 }, async () => {
  const { chromium, expect } = await import('@playwright/test');
  const client = new AislideClient(requestCore);
  const { deck } = await requestCore({ op: 'compile', report: report(256) });
  const session = await client.createDocument({ id: 'g38-studio', deck });
  const exported = await session.exportPresentation();
  const root = resolve('apps/studio/dist');
  const files = new Map();
  async function index(directory, prefix = '') {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      if (entry.isDirectory()) await index(join(directory, entry.name), `${prefix}${entry.name}/`);
      else files.set(`/${prefix}${entry.name}`, join(directory, entry.name));
    }
  }
  await index(root);
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  const context = await browser.newContext({ viewport: { width: 1440, height: 960 }, serviceWorkers: 'block' });
  const failures = [];
  const page = await context.newPage(); page.on('pageerror', error => failures.push(error.message));
  await context.route('**/*', async route => {
    const url = new URL(route.request().url());
    if (url.origin !== 'https://g38.invalid') { failures.push(`Unexpected origin ${url.origin}`); await route.abort(); return; }
    if (url.pathname === '/api/core') {
      try { await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(await requestCore(route.request().postDataJSON())) }); }
      catch (error) { await route.fulfill({ status: 400, contentType: 'application/json', body: JSON.stringify({ error: error.message }) }); }
      return;
    }
    const file = files.get(url.pathname === '/' ? '/index.html' : url.pathname);
    if (!file) { failures.push(`Unknown built asset ${url.pathname}`); await route.abort(); return; }
    const types = { '.html': 'text/html', '.js': 'application/javascript', '.css': 'text/css', '.woff2': 'font/woff2', '.woff': 'font/woff', '.ttf': 'font/ttf', '.svg': 'image/svg+xml', '.png': 'image/png' };
    await route.fulfill({ status: 200, contentType: types[extname(file)] ?? 'application/octet-stream', body: await readFile(file) });
  });
  try {
    await page.goto('https://g38.invalid/');
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled({ timeout: 30000 });
    await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'g38-synthetic.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(exported.base64, 'base64') });
    await expect(page.locator('.thumbnail')).toHaveCount(256, { timeout: 30000 });
    await page.locator('.thumbnail').last().click();
    await expect(page.locator('.slide-stage')).toContainText('Synthetic page 256');
    await page.getByRole('button', { name: 'Edit title', exact: true }).dblclick();
    const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
    await editor.fill('Synthetic page edited'); await editor.press('Control+Enter');
    await expect(editor).toHaveCount(0, { timeout: 30000 });
    await expect(page.locator('.slide-stage')).toContainText('Synthetic page edited');
    const undoCompleted = page.waitForResponse(response => response.url().endsWith('/api/core') && response.request().postDataJSON()?.op === 'undo_transaction');
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    const undoResponse = await undoCompleted;
    assert.ok(undoResponse.ok(), await undoResponse.text());
    await undoResponse.finished();
    await expect(page.locator('.slide-stage')).toContainText('Synthetic page 256');
    const pending = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
    const download = await pending;
    const chunks = []; for await (const chunk of await download.createReadStream()) chunks.push(chunk);
    assert.equal(Buffer.concat(chunks).toString('base64'), exported.base64);
    await page.evaluate(async () => { await document.fonts.ready; });
    await mkdir('.artifacts/g38', { recursive: true });
    await page.screenshot({ path: '.artifacts/g38/studio-256-desktop.png' });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: '.artifacts/g38/studio-256-mobile.png' });
    await expect(page.getByRole('alert')).toHaveCount(0);
    assert.deepEqual(failures, []);
  } finally { await browser.close(); }
});

test('phase5 MCP supports 256 slides, checked profile selection and both restored histories', { timeout: 120000 }, async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'phase5-mcp', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const created = await call('compile_report', { report: report(256), capacity_profile: 'large' });
    const deck = await call('get_deck', { deck_id: created.deck_id });
    assert.equal(deck.slides.length, 256);
    await call('edit_slides', { deck_id: created.deck_id, expected_revision: 0, operations: [{ op: 'move', slide_id: deck.slides[0].id, index: 255 }] });
    await call('apply_transaction', { deck_id: created.deck_id, expected_revision: 1, operations: [{ op: 'replace', path: '/deck/title', value: 'Changed once' }] });
    await call('undo', { deck_id: created.deck_id });
    const envelope = await call('get_session_recovery', { deck_id: created.deck_id });
    assert.equal(envelope.past.length, 1); assert.equal(envelope.future.length, 1);
    const recovered = await call('recover_session', { recovery_json: JSON.stringify(envelope) });
    assert.equal(recovered.capacity_profile, 'large'); assert.equal(recovered.can_undo, true); assert.equal(recovered.can_redo, true);
    await call('redo', { deck_id: recovered.deck_id });
    assert.equal((await call('get_deck', { deck_id: recovered.deck_id })).title, 'Changed once');
    const before = await call('get_session_recovery', { deck_id: recovered.deck_id });
    assert.equal((await client.callTool({ name: 'set_capacity_profile', arguments: { deck_id: recovered.deck_id, expected_revision: before.document.revision, profile: 'standard' } })).isError, true);
    assert.deepEqual(await call('get_session_recovery', { deck_id: recovered.deck_id }), before);
    const legacy = await call('create_presentation', { title: 'Legacy', capacity_profile: 'legacy' });
    assert.equal((await call('get_session_recovery', { deck_id: legacy.deck_id })).capacity_profile, 'legacy');
  } finally { await client.close(); }
});
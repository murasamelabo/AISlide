import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createServer } from 'node:http';
import { once } from 'node:events';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { readFile } from 'node:fs/promises';
import { AislideClient, DocumentSession } from '../packages/client/index.mjs';
import { requestCore, coreTimeout } from './core-client.mjs';
import { model, verifyModel, setup } from './local-image-model-setup.mjs';
import { startLocalModel } from './local-model-runtime.mjs';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import sharp from 'sharp';

function transport(environment) {
  return (request, { signal } = {}) => new Promise((resolveResult, reject) => {
    const child = execFile(resolve('target/debug/aislide.exe'), ['request'], { env: { ...process.env, ...environment }, windowsHide: true, timeout: coreTimeout(request), maxBuffer: 16777217, signal }, (error, stdout, stderr) => {
      if (error) { reject(new Error(error.name === 'AbortError' ? 'Operation cancelled' : stderr || error.message)); return; }
      try { resolveResult(JSON.parse(stdout)); } catch (error) { reject(error); }
    });
    child.stdin.on('error', () => {}); child.stdin.end(JSON.stringify(request));
  });
}

test('local model setup requires acknowledgement and a strong byte pin', async () => {
  await assert.rejects(setup([]), /acknowledgement/);
  assert.throws(() => verifyModel(Buffer.alloc(model.bytes)), /digest mismatch/);
  assert.throws(() => verifyModel(Buffer.alloc(1)), /digest mismatch/);
  for (const op of ['text_assist', 'segment_image']) assert.equal(coreTimeout({ op }), 310000);
  assert.equal(coreTimeout({ op: 'edit_image' }), 20000);
});

test('bounded fixture provider proves review-only text, native apply, stale guards, Undo and late cancellation', async () => {
  const server = createServer(async (request, response) => {
    let content = ''; for await (const chunk of request) content += chunk;
    const body = JSON.parse(content);
    assert.equal(body.response_format.json_schema.strict, true);
    assert.equal(body.response_format.json_schema.schema.additionalProperties, false);
    assert.equal(request.headers.authorization, 'Bearer synthetic-local-key');
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end(JSON.stringify({ choices: [{ finish_reason: 'stop', message: { content: JSON.stringify({ text: 'This is a test.\nSecond corrected paragraph.' }) } }] }));
  });
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const local = transport({ AISLIDE_AI_BASE_URL: `http://127.0.0.1:${server.address().port}/v1`, AISLIDE_AI_MODEL: 'bounded-test-fixture-not-ai', AISLIDE_AI_API_KEY: 'synthetic-local-key' });
  try {
    const client = new AislideClient(local);
    const created = await client.createPresentation('ai-sdk');
    const deck = created.document.deck;
    deck.slides[0].elements = [{ type: 'text', id: 'text', x: 60, y: 100, width: 900, height: 300, text: 'This are a test.\nSecond paragraph.', font_size: 24, color: '222222', bold: false,
      format: { paragraphs: [{ alignment: 'center', runs: [{ text: 'This are a test.', style: { bold: true } }] }, { space_after: { kind: 'points', value: 800 }, runs: [{ text: 'Second paragraph.', style: { italic: true } }] }] } }];
    await created.replaceDeck(deck);
    const bytes = (await created.exportPresentation()).base64;
    const session = (await client.openPresentation('ai-native', bytes)).session;
    const original = session.document;
    const expected_text = original.deck.slides[0].elements[0].text;
    const result = await session.textAssist({ task: 'proofread', text: expected_text, language: 'en' });
    assert.equal(session.document.hash, original.hash); assert.equal(session.canUndo, false);
    assert.equal(result.provenance.verified, false);
    await assert.rejects(session.applyTextAssist('slide-1', { id: 'text', expected_text: 'stale', candidate: result.candidate }, { expectedRevision: 0 }), /hash|text/i);
    await session.applyTextAssist('slide-1', { id: 'text', expected_text, candidate: result.candidate }, { expectedRevision: 0 });
    assert.equal(session.revision, 1);
    const reopened = (await client.openPresentation('changed', (await session.exportPresentation()).base64)).session;
    assert.equal(reopened.document.deck.slides[0].elements[0].text, result.candidate.text);
    assert.equal(reopened.document.deck.slides[0].elements[0].format.paragraphs[1].space_after.value, 800);
    await session.undo(); assert.equal((await session.exportPresentation()).base64, bytes);
    const controller = new AbortController();
    const cancelled = new DocumentSession(async request => { const result = await local(request); controller.abort(); return result; }, original);
    await assert.rejects(cancelled.applyTextAssist('slide-1', { id: 'text', expected_text, candidate: result.candidate }, { signal: controller.signal }), /cancel/i);
    assert.equal(cancelled.document.hash, original.hash); assert.equal(cancelled.canUndo, false);
    const latePreview = new AbortController();
    const readonly = new DocumentSession(async request => { const result = await local(request); latePreview.abort(); return result; }, original);
    await assert.rejects(readonly.textAssist({ task: 'proofread', text: expected_text, language: 'en' }, { signal: latePreview.signal }), /cancel/i);
    assert.equal(readonly.document.hash, original.hash); assert.equal(readonly.canUndo, false);
    const early = new AbortController(); early.abort();
    await assert.rejects(session.textAssist({ task: 'proofread', text: expected_text, language: 'en' }, { signal: early.signal }), /cancel/i);
  } finally { server.closeAllConnections(); await new Promise(done => server.close(done)); }
});

test('local AI MCP schemas reject unknown paths and share the actual core apply history', async () => {
  const client = new Client({ name: 'local-ai-tests', version: '1.0.0' });
  const channel = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text); };
  try {
    await client.connect(channel);
    const tools = (await client.listTools()).tools;
    for (const name of ['text_assist', 'apply_text_assist', 'segmentation_status', 'segment_image']) assert.equal(tools.find(tool => tool.name === name).inputSchema.additionalProperties, false);
    const status = await call('segmentation_status'); assert.equal(status.remote, false); assert.equal('path' in status, false);
    assert.equal((await client.callTool({ name: 'text_assist', arguments: { input: { task: 'proofread', text: 'test', language: 'en', allow_remote: true } } })).isError, true);
    assert.equal((await client.callTool({ name: 'segment_image', arguments: { base64: '', mime_type: 'image/png', path: 'untrusted' } })).isError, true);
    const { deck_id } = await call('create_presentation', { title: 'Synthetic local AI test' });
    await call('add_object', { deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'text', kind: 'text' });
    const before = await call('get_document', { deck_id });
    const expected_text = before.deck.slides[0].elements[0].text;
    const candidate = { text: 'Reviewed synthetic text', source_sha256: createHash('sha256').update(expected_text).digest('hex') };
    await call('apply_text_assist', { deck_id, expected_revision: before.revision, slide_id: 'slide-1', id: 'text', expected_text, candidate });
    await call('undo', { deck_id }); assert.equal((await call('get_document', { deck_id })).hash, before.hash);
  } finally { await client.close(); }
});

test('real existing Qwen proofreading and translation smoke, with actual cancellation', { skip: process.env.AISLIDE_REAL_AI_TEST !== '1', timeout: 360000 }, async () => {
  const path = resolve('.tools/local-model/qwen2.5-1.5b-instruct-q4_k_m.gguf');
  const bytes = await readFile(path);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), '6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e');
  const modelServer = await startLocalModel();
  try {
    const client = new AislideClient(transport(modelServer.environment));
    for (const input of [
      { task: 'proofread', text: 'This are a test. The report contain three sections.', language: 'en' },
      { task: 'proofread', text: '私は昨日、学校に行きますた。', language: 'ja' },
      { task: 'translate', text: 'The meeting starts at nine. Please bring the report.', language: 'en', target_language: 'ja' },
      { task: 'translate', text: '明日の会議は午前九時に始まります。資料を持ってきてください。', language: 'ja', target_language: 'en' },
    ]) {
      const result = await client.textAssist(input);
      assert.equal(result.provenance.mode, 'model'); assert.equal(result.provenance.remote, false);
      assert.notEqual(result.candidate.text, input.text);
      assert.equal(result.candidate.source_sha256, createHash('sha256').update(input.text).digest('hex'));
      if ((input.target_language ?? input.language) === 'ja') assert.match(result.candidate.text, /[\u3040-\u30ff\u4e00-\u9fff]/);
      else assert.match(result.candidate.text, /[a-zA-Z]{3}/);
      console.log('REAL_QWEN', JSON.stringify({ input, result }));
    }
    const controller = new AbortController();
    const request = client.textAssist({ task: 'translate', text: 'Please translate this synthetic sentence. '.repeat(60), language: 'en', target_language: 'ja' }, { signal: controller.signal });
    setImmediate(() => controller.abort());
    await assert.rejects(request, /cancel/i);
  } finally { await modelServer.stop(); }
});

test('actual segmentation status contains the fixed model pin but never host paths', async () => {
  const status = await new AislideClient(requestCore).segmentationStatus();
  assert.equal(status.sha256, model.sha256);
  assert.equal(status.remote, false);
  assert.equal(status.message.includes(process.env.LOCALAPPDATA), false);
});

test('image candidate and apply late cancellation cannot create document history', async () => {
  const client = new AislideClient(requestCore);
  const bytes = await sharp({ create: { width: 2, height: 2, channels: 4, background: '#cc4422' } }).png().toBuffer();
  const source = await client.createPresentation('ai-image-cancel');
  const deck = source.document.deck;
  deck.slides[0].elements = [{ type: 'picture', id: 'image', x: 60, y: 100, width: 200, height: 200, base64: bytes.toString('base64'), mime_type: 'image/png', alt: 'Synthetic', crop: { left: 0, right: 0, top: 0, bottom: 0 } }];
  const session = await client.createDocument({ id: 'image-original', deck });
  const original = session.document;
  const image = await client.editImage({ base64: bytes.toString('base64'), mime_type: 'image/png', params: { grayscale: true } });
  const controller = new AbortController();
  const cancelled = new DocumentSession(async request => { const result = await requestCore(request); controller.abort(); return result; }, original);
  await assert.rejects(cancelled.applyImageEdit('slide-1', { id: 'image', image }, { signal: controller.signal }), /cancel/i);
  assert.equal(cancelled.document.hash, original.hash); assert.equal(cancelled.canUndo, false);
  const previewToken = new AbortController();
  const preview = new AislideClient(async request => { assert.equal(request.op, 'segment_image'); previewToken.abort(); return { image }; });
  await assert.rejects(preview.segmentImage({ base64: bytes.toString('base64'), mime_type: 'image/png' }, { signal: previewToken.signal }), /cancel/i);
  assert.equal(session.document.hash, original.hash);
});
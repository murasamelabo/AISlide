import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, globSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const fontPath = globSync('.tools/cargo/registry/src/*/cosmic-text-0.19.0/fonts/NotoSans-Regular.ttf')[0];
const base64 = fontPath ? readFileSync(fontPath).toString('base64') : '';
if (fontPath) assert.match(readFileSync(fontPath.replace('NotoSans-Regular.ttf','NotoSans-LICENSE'),'utf8'), /SIL OPEN FONT LICENSE/);

const cjkPath = resolve('.tools/fonts/phase5/NotoSansJP-Regular.ttf');
test('phase5 MCP accepts full CJK font bytes without raising the image schema and reports a history boundary', { skip: !existsSync(cjkPath), timeout: 90000 }, async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'phase5-cjk-mcp', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    assert.equal(tools.find(tool => tool.name === 'inspect_font').inputSchema.properties.base64.maxLength, 16777216);
    assert.equal(tools.find(tool => tool.name === 'add_picture').inputSchema.properties.base64.maxLength, 1398104);
    const base64 = readFileSync(cjkPath).toString('base64');
    assert.equal((await call('inspect_font', { base64 })).family, 'Noto Sans JP');
    const { deck_id } = await call('create_presentation', { title: 'Japanese MCP', capacity_profile: 'large' });
    await call('embed_font', { deck_id, expected_revision: 0, base64, license_acknowledged: true });
    assert.equal((await call('list_fonts', { deck_id })).fonts[0].byte_length, 5766884);
    await call('undo', { deck_id });
    const envelope = await call('get_session_recovery', { deck_id });
    assert.equal(envelope.future.length, 0); assert.equal(envelope.history_boundary, 'history_limit');
  } finally { await client.close(); }
});

test('phase5 real full CJK font exceeds 1 MiB and roundtrips native Japanese text with complete admission', { skip: !existsSync(cjkPath), timeout: 300000 }, async context => {
  const bytes = readFileSync(cjkPath);
  assert.ok(bytes.length > 1048576);
  const client = new AislideClient(async (request, options) => {
    const started = performance.now();
    try { return await requestCore(request, { ...options, signal: options?.signal ? AbortSignal.any([options.signal, context.signal]) : context.signal }); }
    finally { console.log('PHASE5_CJK_STEP', request.op, Math.round(performance.now() - started)); }
  });
  const base64 = bytes.toString('base64');
  const info = await client.inspectFont(base64);
  assert.equal(info.family, 'Noto Sans JP'); assert.equal(info.usable, true); assert.equal(info.fs_type, 0);
  const session = await client.createPresentation('cjk-full-font', 'Japanese full font');
  await session.addObject('slide-1', { id: 'japanese', kind: 'text' });
  await session.replaceTextContent('slide-1', { id: 'japanese', text: '日本語の編集と復元。容量を確認します。' });
  const before = session.document;
  await session.embedFont({ base64, license_acknowledged: true });
  await session.formatText('slide-1', { id: 'japanese', start: 0, end: [...session.document.deck.slides[0].elements[0].text].length, style: { font_family: info.family } });
  const measured = await client.request({ op: 'measure_layout', deck: session.document.deck });
  assert.ok(measured.measurements.some(item => item.fonts.includes(info.family)));
  assert.ok(!measured.issues.some(issue => issue.code === 'missing_glyph' || issue.severity === 'error'), JSON.stringify(measured.issues));
  const exported = await session.exportPresentation();
  assert.ok(Buffer.from(exported.base64, 'base64').length < 16 * 1048576);
  const reopened = (await client.openPresentation('cjk-reopened', exported.base64)).session;
  assert.equal(reopened.document.deck.embedded_fonts[0].base64, base64);
  assert.ok(Buffer.byteLength(JSON.stringify(reopened.document)) > 8 * 1048576);
  assert.ok(Buffer.byteLength(JSON.stringify(reopened.document)) < 32 * 1048576);
  assert.equal((await reopened.exportPresentation()).base64, exported.base64);
  await assert.rejects(reopened.setCapacityProfile('standard'), /document|8388608/i);
  assert.equal(reopened.capacityProfile, 'large');
  await reopened.setFontUsage(info.sha256, true);
  await reopened.replaceTextContent('slide-1', { id: 'japanese', text: '日本語の文字を変更しました。' });
  const edited = await reopened.exportPresentation();
  const final = (await client.openPresentation('cjk-final', edited.base64)).session;
  assert.equal(final.document.deck.slides[0].elements[0].text, '日本語の文字を変更しました。');
  await reopened.undo(); await reopened.undo();
  assert.equal((await reopened.exportPresentation()).base64, exported.base64);
  await session.undo(); await session.undo(); assert.equal(session.document.hash, before.hash);
  console.log('PHASE5_CJK', JSON.stringify({ font_bytes: bytes.length, font_sha256: info.sha256, pptx_bytes: Buffer.from(exported.base64, 'base64').length, reopened_document_bytes: Buffer.byteLength(JSON.stringify(reopened.document)), family: info.family, office_verified: false }));
});

test('font SDK uses consent, explicit assignment, bounded inspection and Undo', { skip: !fontPath }, async () => {
  const client = new AislideClient(requestCore);
  const info = await client.inspectFont(base64);
  assert.equal(info.family, 'Noto Sans');
  assert.equal(info.license_verified, false);
  assert.equal(info.usable, true);
  const session = await client.createPresentation('font-sdk');
  const original = session.document;
  await assert.rejects(session.embedFont({ base64, license_acknowledged: false }), /license/i);
  await session.embedFont({ base64, license_acknowledged: true }, { expectedRevision: 0 });
  assert.equal(session.document.deck.embedded_fonts.length, 1);
  assert.equal((await session.listFonts()).fonts[0].sha256, info.sha256);
  await assert.rejects(session.embedFont({ base64, license_acknowledged: true }, { expectedRevision: 0 }), /revision/i);
  await session.undo(); assert.equal(session.document.hash, original.hash);
  await session.redo(); assert.equal(session.document.deck.embedded_fonts.length, 1);
  await session.addObject('slide-1', { id: 'text', kind: 'text' });
  const text = session.document.deck.slides[0].elements[0].text;
  await session.formatText('slide-1', { id: 'text', start: 0, end: [...text].length, style: { font_family: info.family } });
  const exported = await session.exportPresentation();
  const inventory = await client.inspectPptxFonts(exported.base64);
  assert.equal(inventory.fonts.length, 1);
  assert.equal(inventory.fonts[0].info.sha256, info.sha256);
  const reopened = (await client.openPresentation('font-sdk-reopen', exported.base64)).session;
  assert.equal(reopened.document.deck.embedded_fonts[0].license_acknowledged, false);
  assert.equal((await reopened.exportPresentation()).base64, exported.base64);
  await reopened.setFontUsage(info.sha256, true);
  assert.equal(reopened.document.deck.embedded_fonts[0].license_acknowledged, true);
  const measured = await client.request({ op: 'measure_layout', deck: reopened.document.deck });
  assert.ok(measured.measurements[0].fonts.includes(info.family));
  const pdf = await reopened.exportStatic({ format: 'pdf' });
  assert.ok(Buffer.from(pdf.files[0].base64,'base64').subarray(0,5).equals(Buffer.from('%PDF-')));
  assert.equal(pdf.office_parity_verified, false);
});

test('font MCP tools are strict and use the same revision history', { skip: !fontPath }, async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'font-tests', version: '1.0.0' });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text); };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const name of ['inspect_font','inspect_pptx_fonts','list_fonts','embed_font','set_font_usage']) {
      const tool = tools.find((tool) => tool.name === name); assert.ok(tool, name); assert.equal(tool.inputSchema.additionalProperties, false);
    }
    const { deck_id } = await call('create_presentation', { title: 'Font MCP' });
    const before = await call('get_document', { deck_id });
    assert.equal((await client.callTool({ name:'embed_font', arguments:{deck_id, expected_revision:0, base64, license_acknowledged:false} })).isError, true);
    assert.equal((await client.callTool({ name:'inspect_font', arguments:{base64, path:'not-allowed'} })).isError, true);
    await call('embed_font', { deck_id, expected_revision:0, base64, license_acknowledged:true });
    assert.equal((await call('list_fonts',{deck_id})).fonts[0].family,'Noto Sans');
    await call('undo',{deck_id}); assert.equal((await call('get_document',{deck_id})).hash,before.hash);
  } finally { await client.close(); }
});
import test from 'node:test';
import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { AislideClient, DocumentSession } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const dictionaryInput = { language: 'en-US', format: 'json', content: JSON.stringify({ language: 'en-US', words: ['hello', 'world'], synonyms: { hello: ['greeting'] }, translations: { 'ja-JP': { hello: 'こんにちは' } } }) };

test('proofing SDK uses local dictionaries and cancellable requests without mutation', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('proof-sdk');
  const original = session.document;
  const dictionary = await session.importProofingDictionary(dictionaryInput);
  const result = await session.proofText({ text: 'hello wurld', language: 'en-US', dictionary, term: 'hello', target_language: 'ja-JP' });
  assert.deepEqual(result.issues, [{ word: 'wurld', start: 6, end: 11, suggestions: ['world'] }]);
  assert.deepEqual(result.synonyms, ['greeting']);
  assert.equal(result.translation, 'こんにちは');
  assert.equal(result.complete_dictionary, false);
  assert.equal(session.document.hash, original.hash);
  assert.equal(session.canUndo, false);
  await assert.rejects(session.proofText({ text: 'hello', language: 'de-DE', dictionary }), /dictionary/i);
  const controller = new AbortController(); controller.abort();
  await assert.rejects(session.proofText({ text: 'hello', language: 'en-US' }, { signal: controller.signal }), /cancel/i);
  assert.equal(session.document.hash, original.hash);
});

test('format SDK native rich text and language persist and Undo restores exact package', async () => {
  const client = new AislideClient(requestCore);
  const authored = await client.createPresentation('format-sdk');
  const deck = authored.document.deck;
  deck.slides[0].elements = [
    { type: 'text', id: 'source', x: 80, y: 100, width: 500, height: 120, text: 'hello report', font_size: 32, color: 'BB2211', bold: true, format: { italic: true, alignment: 'center' } },
    { type: 'text', id: 'target', x: 80, y: 300, width: 500, height: 180, text: 'hello world\nsecond', font_size: 22, color: '202525', bold: false, format: { hyperlink: 'https://example.invalid/target', paragraphs: [{ runs: [{ text: 'hello ', style: { highlight: 'FFFF00' } }, { text: 'world', style: { language: 'fr-FR' } }] }, { runs: [{ text: 'second', style: {} }] }] } },
  ];
  await authored.replaceDeck(deck);
  const bytes = (await authored.exportPresentation()).base64;
  const session = (await client.openPresentation('format-native', bytes)).session;
  const before = session.document;
  const style = await session.copyFormat('slide-1', { id: 'source' });
  assert.equal(session.revision, 0); assert.equal(session.canUndo, false);
  assert.equal(JSON.stringify(style).includes('https:'), false);
  await session.applyFormat('slide-1', { ids: ['target'], style }, { expectedRevision: 0 });
  const changed = session.document.deck.slides[0].elements[1];
  assert.equal(changed.text, before.deck.slides[0].elements[1].text);
  assert.equal(changed.font_size, before.deck.slides[0].elements[1].font_size);
  assert.equal(changed.format.paragraphs[0].runs[0].style.font_size, 32);
  assert.equal(changed.format.paragraphs[0].runs[0].style.highlight, 'FFFF00');
  assert.equal(changed.format.hyperlink, 'https://example.invalid/target');
  await assert.rejects(session.applyFormat('slide-1', { ids: ['target'], style }, { expectedRevision: 0 }), /revision/i);
  const reopened = (await client.openPresentation('format-reopened', (await session.exportPresentation()).base64)).session;
  assert.equal(reopened.document.deck.slides[0].elements[1].font_size, 32);
  await session.undo(); assert.equal((await session.exportPresentation()).base64, bytes);
  await session.setProofingLanguage('slide-1', { id: 'target', language: 'en-US' });
  const language = (await client.openPresentation('language-reopened', (await session.exportPresentation()).base64)).session.document.deck.slides[0].elements[1];
  assert.ok(language.format.paragraphs.every(paragraph => paragraph.runs.every(run => run.style.language === 'en-US')));
  await session.undo(); assert.equal((await session.exportPresentation()).base64, bytes);
  const controller = new AbortController();
  const cancelled = new DocumentSession(async request => { const result = await requestCore(request); controller.abort(); return result; }, before);
  await assert.rejects(cancelled.applyFormat('slide-1', { ids: ['target'], style }, { signal: controller.signal }), /cancel/i);
  assert.equal(cancelled.document.hash, before.hash); assert.equal(cancelled.canUndo, false);
});

test('proofing and painter MCP tools are strict and share revision history', async () => {
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs')], stderr: 'pipe' });
  const client = new Client({ name: 'proofing-tests', version: '1.0.0' });
  const call = async (name, args = {}) => { const result = await client.callTool({ name, arguments: args }); assert.ok(!result.isError, JSON.stringify(result.content)); return JSON.parse(result.content[0].text); };
  try {
    await client.connect(transport);
    const tools = (await client.listTools()).tools;
    for (const name of ['import_proofing_dictionary', 'proof_text', 'copy_format', 'apply_format', 'set_proofing_language', 'sample_slide_pixel']) {
      const tool = tools.find(tool => tool.name === name); assert.ok(tool, name); assert.equal(tool.inputSchema.additionalProperties, false);
    }
    const dictionary = await call('import_proofing_dictionary', dictionaryInput);
    assert.equal((await call('proof_text', { text: 'wurld', language: 'en-US', dictionary })).issues[0].suggestions[0], 'world');
    assert.equal((await client.callTool({ name: 'import_proofing_dictionary', arguments: { ...dictionaryInput, path: 'not-allowed' } })).isError, true);
    assert.equal((await client.callTool({ name: 'proof_text', arguments: { text: 'hello', language: 'en-US', remote: true } })).isError, true);
    const { deck_id } = await call('create_presentation', { title: 'Proofing MCP' });
    await call('add_object', { deck_id, expected_revision: 0, slide_id: 'slide-1', id: 'text', kind: 'text' });
    const before = await call('get_document', { deck_id });
    const style = await call('copy_format', { deck_id, slide_id: 'slide-1', id: 'text' });
    assert.equal((await call('get_document', { deck_id })).revision, before.revision);
    await call('apply_format', { deck_id, expected_revision: before.revision, slide_id: 'slide-1', ids: ['text'], style });
    await call('undo', { deck_id });
    assert.equal((await call('get_document', { deck_id })).hash, before.hash);
    assert.equal((await client.callTool({ name: 'apply_format', arguments: { deck_id, expected_revision: 999, slide_id: 'slide-1', ids: ['text'], style } })).isError, true);
    const rect = { type: 'rect', id: 'source', x: 100, y: 100, width: 100, height: 100, fill: '00AA44' };
    await call('apply_transaction', { deck_id, expected_revision: (await call('get_document', { deck_id })).revision, operations: [{ op: 'replace', path: '/deck/slides/0/elements', value: [rect, { ...rect,id:'target',x:300,fill:'FFFFFF' }] }] });
    const source = await call('get_document', { deck_id });
    const objectStyle = await call('copy_format', { deck_id,slide_id:'slide-1',id:'source' });
    assert.equal(objectStyle.kind,'filled');
    const sampled = await call('sample_slide_pixel', { deck_id,slide_id:'slide-1',x:120,y:120 });
    assert.equal(sampled.color,'00AA44');
    assert.equal((await call('get_document', { deck_id })).hash,source.hash);
    assert.equal((await client.callTool({ name:'sample_slide_pixel',arguments:{deck_id,slide_id:'slide-1',x:120,y:120,url:'https://example.invalid'} })).isError,true);
    await call('apply_format', { deck_id,expected_revision:source.revision,slide_id:'slide-1',ids:['target'],style:objectStyle });
    assert.equal((await call('get_document', { deck_id })).deck.slides[0].elements[1].fill,'00AA44');
    await call('undo', { deck_id }); assert.equal((await call('get_document', { deck_id })).hash,source.hash);
    const table = { type:'table',id:'table-source',x:10,y:10,width:400,height:200,rows:[['Source']],font_size:24,format:{cells:[{row:0,column:0,style:{text_format:{paragraphs:[{alignment:'center',runs:[{text:'Source',style:{bold:true,color:'0055AA'}}]}]}}}]}};
    await call('apply_transaction', { deck_id,expected_revision:(await call('get_document',{deck_id})).revision,operations:[{op:'replace',path:'/deck/slides/0/elements',value:[table,{...table,id:'table-target',x:500,rows:[['Keep']],format:{}}]}] });
    const tableBefore=await call('get_document',{deck_id});
    const tableStyle=await call('copy_format',{deck_id,slide_id:'slide-1',id:'table-source'});
    assert.equal(JSON.stringify(tableStyle).includes('Source'),false);
    await call('apply_format',{deck_id,expected_revision:tableBefore.revision,slide_id:'slide-1',ids:['table-target'],style:tableStyle});
    const painted=(await call('get_document',{deck_id})).deck.slides[0].elements[1];
    assert.equal(painted.rows[0][0],'Keep'); assert.equal(painted.format.cells[0].style.text_format.paragraphs[0].runs[0].style.bold,true);
    await call('undo',{deck_id}); assert.equal((await call('get_document',{deck_id})).hash,tableBefore.hash);
  } finally { await client.close(); }
});

test('phase2 SDK object formats and slide sampling preserve native state', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('phase2-painter');
  const deck = session.document.deck;
  deck.slides[0].elements = [{ type:'rect',id:'source',x:100,y:100,width:100,height:100,fill:'FF0000',visual:{opacity:0.5} },{ type:'rect',id:'target',x:300,y:100,width:100,height:100,fill:'00AA44' }];
  await session.replaceDeck(deck);
  const original = (await session.exportPresentation()).base64;
  const native = (await client.openPresentation('phase2-style-native',original)).session;
  const before = native.document;
  assert.equal((await native.sampleSlidePixel('slide-1',150,150)).color,'FF7F7F');
  assert.equal(native.canUndo,false);
  const style = await native.copyFormat('slide-1',{id:'source'});
  assert.equal(style.kind,'filled');
  await assert.rejects(native.applyFormat('slide-1',{ids:['target','missing'],style}));
  assert.equal(native.document.hash,before.hash);
  await native.applyFormat('slide-1',{ids:['target'],style});
  assert.equal(native.document.deck.slides[0].elements[1].x,300);
  assert.equal((await native.sampleSlidePixel('slide-1',350,150)).color,'FF7F7F');
  await native.undo(); assert.equal((await native.exportPresentation()).base64,original);
});
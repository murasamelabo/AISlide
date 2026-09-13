import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

test('MCP completes the source-bound PoC with native graphics and shared transactions', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-mcp-poc-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'poc-proof', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const response = await client.callTool({ name, arguments: args });
    assert.ok(!response.isError, JSON.stringify(response.content));
    return JSON.parse(response.content[0].text);
  };
  try {
    await client.connect(transport);
    const source = await call('ingest_source', { format: 'csv', name: 'fixture.csv', base64: Buffer.from('Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n').toString('base64') });
    const result = await call('compile_data_report', { source_id: source.source_id, mapping: { title: 'MCP provided values', period: 'Synthetic fixture', table_index: 0, category_column: 0, value_columns: [1], row_start: 0, row_count: 3, chart_kind: 'column' } });
    assert.equal(result.slides, 12);
    const before = await call('get_document', { deck_id: result.deck_id });
    assert.equal(before.sources[0].name, 'fixture.csv');
    const titleIndex = before.deck.slides[0].elements.findIndex((element) => element.id === 'title');
    const change = await call('apply_transaction', { deck_id: result.deck_id, expected_revision: 0, operations: [{ op: 'replace', path: `/deck/slides/0/elements/${titleIndex}/text`, value: 'Verified MCP edit' }] });
    assert.equal(change.revision, 1);
    const stale = await client.callTool({ name: 'apply_transaction', arguments: { deck_id: result.deck_id, expected_revision: 0, operations: [{ op: 'replace', path: '/deck/title', value: 'Stale' }] } });
    assert.equal(stale.isError, true);
    await call('undo', { deck_id: result.deck_id });
    assert.equal((await call('get_document', { deck_id: result.deck_id })).deck.slides[0].elements[titleIndex].text, before.deck.slides[0].elements[titleIndex].text);
    await call('redo', { deck_id: result.deck_id });
    await call('add_diagram', { deck_id: result.deck_id, slide_id: 'slide-12', id: 'mcp-flow', steps: ['Ingest', 'Validate', 'Export'] });
    await call('export_project', { deck_id: result.deck_id, filename: 'poc-proof.pptx' });
    const pptx = await readFile(join(directory, 'poc-proof.pptx'));
    const checkpoint = JSON.parse(await readFile(join(directory, 'poc-proof.aislide.json'), 'utf8'));
    assert.equal(pptx.subarray(0, 2).toString(), 'PK');
    assert.ok(checkpoint.document.bindings.length > 0);
    const restored = await call('open_project', { base64: pptx.toString('base64'), checkpoint });
    assert.equal((await call('get_document', { deck_id: restored.deck_id })).hash, checkpoint.document.hash);
    const duplicate = await client.callTool({ name: 'export_project', arguments: { deck_id: result.deck_id, filename: 'poc-proof.pptx' } });
    assert.equal(duplicate.isError, true);
    assert.deepEqual(await readFile(join(directory, 'poc-proof.pptx')), pptx);
  } finally { await client.close(); await rm(directory, { recursive: true, force: true }); }
});
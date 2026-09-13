import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

test('MCP generates, edits and saves with the GUI closed', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-mcp-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
  const client = new Client({ name: 'aislide-test', version: '1.0.0' });
  const call = async (name, args = {}) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content[0].text);
  };
  try {
    await client.connect(transport);
    const tools = await client.listTools();
    assert.ok(tools.tools.some((tool) => tool.name === 'compile_report'));
    const report = await call('sample_report');
    const result = await call('compile_report', { report });
    assert.equal(result.slides, 12);
    await call('update_text', { deck_id: result.deck_id, slide_id: 'slide-1', element_id: 'title', text: 'MCP edited title' });
    const deck = await call('get_deck', { deck_id: result.deck_id });
    assert.equal(deck.slides[0].elements.find((element) => element.id === 'title').text, 'MCP edited title');
    await call('export_pptx', { deck_id: result.deck_id, filename: 'test-report.pptx' });
    const bytes = await readFile(join(directory, 'test-report.pptx'));
    assert.equal(bytes.subarray(0, 2).toString(), 'PK');
    const duplicate = await client.callTool({ name: 'export_pptx', arguments: { deck_id: result.deck_id, filename: 'test-report.pptx' } });
    assert.equal(duplicate.isError, true);
    assert.deepEqual(await readFile(join(directory, 'test-report.pptx')), bytes);
    const traversal = await client.callTool({ name: 'export_pptx', arguments: { deck_id: result.deck_id, filename: '../escape.pptx' } });
    assert.equal(traversal.isError, true);
  } finally {
    await client.close();
    await rm(directory, { recursive: true, force: true });
  }
});
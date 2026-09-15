import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Database, ShieldCheck, Cloud, Users } from 'lucide-react';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const directory = resolve(process.argv[2] ?? `.artifacts/workspace-ux-${Date.now()}`);
await mkdir(directory, { recursive: true });
assert.equal((await readdir(directory)).length, 0, 'Use a new or empty output directory');
const client = new Client({ name: 'workspace-qualification', version: '1.0.0' });
const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe' });
const call = async (name, args = {}) => {
  const response = await client.callTool({ name, arguments: args });
  assert.ok(!response.isError, `${name}: ${JSON.stringify(response.content)}`);
  return JSON.parse(response.content[0].text);
};
const text = (id, value, x, y, width, height, size) => ({ type: 'text', id, text: value, x, y, width, height, font_size: size, color: '@dk1', bold: size >= 28, format: { font_family: '@minor' } });
const note = 'Synthetic workspace qualification: generated icons, editable graph and sample chart values. No real company, production system or performance evidence.';
try {
  await client.connect(transport);
  const created = await call('create_presentation', { title: 'Workspace operations' });
  const deck_id = created.deck_id;
  await call('edit_slides', { deck_id, expected_revision: 0, operations: [
    { op: 'insert', id: 'icons', title: 'Local icon insertion' },
    { op: 'insert', id: 'architecture', title: 'Editable architecture' },
    { op: 'insert', id: 'chart', title: 'Independent chart data' },
  ] });
  let document = await call('get_document', { deck_id });
  await call('apply_transaction', { deck_id, expected_revision: document.revision, operations: document.deck.slides.flatMap((slide, index) => [
    { op: 'replace', path: `/deck/slides/${index}/notes`, value: note },
    { op: 'replace', path: `/deck/slides/${index}/elements`, value: [text('heading', slide.title, 64, 48, 1152, 56, 32), text('disclaimer', 'Synthetic qualification / Native editable objects', 64, 668, 1152, 28, 16)] },
  ]) });
  const icons = [{ name: 'Database', icon: Database }, { name: 'Shield', icon: ShieldCheck }, { name: 'Cloud', icon: Cloud }, { name: 'Users', icon: Users }];
  for (const [index, icon] of icons.entries()) {
    document = await call('get_document', { deck_id });
    const svg = renderToStaticMarkup(createElement(icon.icon, { size: 24, color: index % 2 ? '#007f73' : '#0017c1', strokeWidth: 2 }));
    await call('add_asset', { deck_id, expected_revision: document.revision, slide_id: 'icons', id: `icon-${index}`, base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: `${icon.name} (Lucide)`, size: 144 });
  }
  document = await call('get_document', { deck_id });
  const iconSlide = document.deck.slides.findIndex((slide) => slide.id === 'icons');
  const iconOperations = icons.flatMap((icon, index) => {
    const element = document.deck.slides[iconSlide].elements.findIndex((element) => element.id === `icon-${index}`);
    return [{ op: 'replace', path: `/deck/slides/${iconSlide}/elements/${element}/x`, value: 104 + index * 284 }, { op: 'replace', path: `/deck/slides/${iconSlide}/elements/${element}/y`, value: 248 }, { op: 'add', path: `/deck/slides/${iconSlide}/elements/-`, value: text(`label-${index}`, icon.name, 104 + index * 284, 428, 220, 40, 24) }];
  });
  await call('apply_transaction', { deck_id, expected_revision: document.revision, operations: iconOperations });
  const catalog = await call('graph_catalog');
  document = await call('get_document', { deck_id });
  await call('add_graph', { deck_id, expected_revision: document.revision, slide_id: 'architecture', id: 'graph', spec: catalog.examples[0].spec });
  document = await call('get_document', { deck_id });
  await call('add_object', { deck_id, expected_revision: document.revision, slide_id: 'chart', id: 'native-chart', kind: 'chart', preset: 'column' });
  await call('export_pptx', { deck_id, filename: 'workspace-source.pptx' });
  const original = await readFile(join(directory, 'workspace-source.pptx'));
  const opened = await call('open_pptx', { base64: original.toString('base64') });
  const resultId = opened.deck_id;
  await call('edit_slides', { deck_id: resultId, expected_revision: 0, operations: [
    { op: 'duplicate', slide_id: 'chart', id: 'chart-copy' },
    { op: 'rename', slide_id: 'chart-copy', title: 'Edited native chart copy' },
    { op: 'move', slide_id: 'chart-copy', index: 0 },
    { op: 'remove', slide_id: 'slide-1' },
    { op: 'insert', id: 'added', after: 'architecture', title: 'Added after reopening' },
  ] });
  document = await call('get_document', { deck_id: resultId });
  const addedIndex = document.deck.slides.findIndex((slide) => slide.id === 'added');
  const copiedChart = document.deck.slides[0].elements.findIndex((element) => element.id === 'native-chart');
  const oldValue = document.deck.slides[0].elements[copiedChart].series[0].values[0];
  await call('apply_transaction', { deck_id: resultId, expected_revision: document.revision, operations: [
    { op: 'replace', path: '/deck/title', value: 'Workspace operations / Native reopen' },
    { op: 'replace', path: `/deck/slides/0/elements/${copiedChart}/series/0/values/0`, value: oldValue + 3 },
    { op: 'replace', path: '/deck/slides/0/elements/0/text', value: 'Edited native chart copy' },
    { op: 'replace', path: `/deck/slides/${addedIndex}/notes`, value: note },
    { op: 'replace', path: `/deck/slides/${addedIndex}/elements`, value: [text('added-heading', 'Added after reopening', 64, 48, 1152, 56, 32), text('added-detail', 'New slide and SVG asset inserted into the preserved native package.', 64, 136, 1100, 44, 22)] },
  ] });
  document = await call('get_document', { deck_id: resultId });
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><defs><linearGradient id="paint"><stop stop-color="#0017c1"/><stop offset="1" stop-color="#007f73"/></linearGradient></defs><rect x="4" y="4" width="40" height="40" rx="8" fill="url(#paint)"/><path d="m14 24 7 7 14-15" fill="none" stroke="#fff" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/></svg>';
  await call('add_asset', { deck_id: resultId, expected_revision: document.revision, slide_id: 'added', id: 'imported-path', base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: 'Synthetic outlined SVG check icon', size: 240 });
  document = await call('get_document', { deck_id: resultId });
  assert.equal(document.deck.slides.find((slide) => slide.id === 'chart').elements.find((element) => element.id === 'native-chart').series[0].values[0], oldValue);
  assert.equal(document.parts[0].stale, false);
  await call('export_pptx', { deck_id: resultId, filename: 'workspace-edited.pptx' });
  const edited = await readFile(join(directory, 'workspace-edited.pptx'));
  const reopened = await call('open_pptx', { base64: edited.toString('base64') });
  const verified = await call('get_document', { deck_id: reopened.deck_id });
  assert.deepEqual(verified.deck.slides.map((slide) => slide.id), ['chart-copy', 'icons', 'architecture', 'added', 'chart']);
  assert.equal(verified.parts[0].stale, false);
  assert.equal(verified.deck.slides[0].elements.find((element) => element.type === 'chart').series[0].values[0], oldValue + 3);
  assert.deepEqual(await readFile(join(directory, 'workspace-source.pptx')), original);
  const counts = {};
  const count = (elements) => { for (const element of elements) { counts[element.type] = (counts[element.type] ?? 0) + 1; if (element.type === 'group') count(element.children); } };
  for (const slide of verified.deck.slides) count(slide.elements);
  const evidence = { generated_at: new Date().toISOString(), transport: 'Official MCP SDK / local Rust core', source_sha256: createHash('sha256').update(original).digest('hex'), edited_sha256: createHash('sha256').update(edited).digest('hex'), bytes: edited.length, slides: verified.deck.slides.map((slide) => ({ id: slide.id, title: slide.title })), objects: counts, independent_chart_values: true, reopened_metadata_current: true, original_unchanged: true, svg_stored_as_png: true, office_visual_parity: false };
  await writeFile(join(directory, 'evidence.json'), JSON.stringify(evidence, null, 2), { flag: 'wx' });
  console.log(JSON.stringify({ directory, ...evidence }, null, 2));
} finally { await client.close(); }
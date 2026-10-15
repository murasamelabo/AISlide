import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { Network, Search, Clock, Bot, LayoutGrid, Target, RefreshCw } from 'lucide-react';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const { values, positionals } = parseArgs({ options: { input: { type: 'string' }, 'font-family': { type: 'string' } }, allowPositionals: true });
assert.ok(positionals.length <= 1, 'Usage: node tools/comparison-panels-demo.mjs [new-output-directory] [--input content.json] [--font-family installed-font]');
const directory = resolve(positionals[0] ?? `.artifacts/comparison-panels-${Date.now()}`);
await mkdir(directory, { recursive: true });
assert.equal((await readdir(directory)).length, 0, 'Use a new or empty output directory');
const input = values.input ? JSON.parse(await readFile(resolve(values.input), 'utf8')) : {
  title: 'Separate workflows and shared context', section: '01  Workflow comparison', footer: 'Synthetic example / not a measured system', notes: 'Visual example only. Claims and source authenticity are not verified.', transition: true,
  panels: [
    { label: 'Separate workflows', items: [{ text: 'Signals are joined manually.', icon: 'network' }, { text: 'Context is rebuilt between tools.', icon: 'search' }, { text: 'Handoffs require coordination.', icon: 'clock' }, { text: 'Assistants inherit fragmented context.', icon: 'bot' }] },
    { label: 'Shared context', items: [{ text: 'Signals are available together.', icon: 'grid' }, { text: 'Investigation uses a common context.', icon: 'target' }, { text: 'Actions follow the investigation.', icon: 'refresh' }, { text: 'People and assistants use the same context.', icon: 'bot' }] },
  ],
};
assert.ok(input && typeof input === 'object' && !Array.isArray(input));
assert.ok(Object.keys(input).every(key => ['title', 'section', 'footer', 'notes', 'panels', 'transition'].includes(key)), 'Unknown content field');
assert.equal(input.panels.length, 2);
const iconComponents = { network: Network, search: Search, clock: Clock, bot: Bot, grid: LayoutGrid, target: Target, refresh: RefreshCw };
const environment = process.env.AISLIDE_CORE_BINARY ? { AISLIDE_CORE_BINARY: process.env.AISLIDE_CORE_BINARY } : {};
const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory], stderr: 'pipe', env: environment });
const client = new Client({ name: 'comparison-panels-example', version: '1.0.0' });
const calls = [];
const invoke = async (name, arguments_ = {}) => {
  const result = await client.callTool({ name, arguments: arguments_ }, undefined, { timeout: 320000, maxTotalTimeout: 320000 });
  assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
  calls.push(name);
  return result;
};
const call = async (name, arguments_) => JSON.parse((await invoke(name, arguments_)).content[0].text);
const text = (id, value, x, y, width, height, font_size, color, bold = false) => ({ type: 'text', id, text: value, x, y, width, height, font_size, color, bold });
const icons = new Map();
async function icon(name, color) {
  assert.ok(Object.hasOwn(iconComponents, name), `Unknown icon: ${name}`);
  const key = `${name}:${color}`;
  if (!icons.has(key)) {
    const svg = renderToStaticMarkup(createElement(iconComponents[name], { size: 96, color: `#${color}`, strokeWidth: 1.8 }));
    const asset = await call('create_graph_icon', { base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: `Lucide ${name}` });
    icons.set(key, { asset_id: asset.asset_id, alt: `Lucide ${name}` });
  }
  return icons.get(key);
}

try {
  await client.connect(transport);
  await call('get_tool_schema', { name: 'create_graph_icon' });
  const headerIcon = await icon('grid', '0F6CBD');
  const panels = [];
  for (const [panelIndex, panel] of input.panels.entries()) {
    assert.ok(Object.keys(panel).every(key => ['label', 'items'].includes(key)), 'Unknown example panel field');
    const items = [];
    for (const item of panel.items) {
      assert.ok(Object.keys(item).every(key => ['text', 'icon'].includes(key)), 'Unknown example row field');
      items.push({ text: item.text, ...(item.icon ? { icon: await icon(item.icon, panelIndex === 0 ? 'CC3355' : item.icon === 'grid' ? '0F6CBD' : '087F73') } : {}) });
    }
    panels.push({ label: panel.label, items });
  }
  const { deck_id } = await call('create_presentation', { title: input.title, ...(values['font-family'] ? { setup: { font_family: values['font-family'] } } : {}) });
  const base = await call('get_deck_summary', { deck_id });
  const spec = { version: 1, preset: 'contrast/panels', title: '', subtitle: '', data: { kind: 'comparison_panels', panels, transition: input.transition ?? false }, layout: { x: 32, y: 120, width: 1216, height: 528, show_title: false } };
  await call('get_tool_schema', { name: 'apply_operations' });
  await call('apply_operations', { deck_id, expected_revision: base.revision, expected_hash: base.hash, operations: [
    { op: 'add_elements', slide_id: 'slide-1', elements: [
      { type: 'shape', id: 'header-icon-background', x: 48, y: 34, width: 48, height: 48, preset: 'ellipse', fill: 'E8F2FE', stroke: 'E8F2FE', stroke_width: 0, text: '', font_size: 18, color: '0F6CBD', bold: false },
      text('section-label', input.section ?? '', 108, 20, 1124, 20, 12, '0F6CBD', true),
      text('title', input.title, 108, 44, 1124, 48, 32, '202B36', true),
      { type: 'rect', id: 'header-rule', x: 48, y: 98, width: 1184, height: 1.2, fill: 'CBD7E3' },
      text('footer', input.footer ?? '', 48, 680, 1120, 20, 11, '5B6F83'),
      { ...text('page', '1', 1200, 680, 32, 20, 11, '5B6F83'), format: { alignment: 'right' } },
    ] },
    { op: 'add_picture', slide_id: 'slide-1', id: 'header-icon', ...headerIcon, frame: { x: 56, y: 42, width: 32, height: 32 }, fit: 'contain' },
    { op: 'add_part', slide_id: 'slide-1', id: 'paired-comparison', spec },
    { op: 'update_notes', slide_id: 'slide-1', notes: input.notes ?? 'Visual example only. Claims are not independently verified.' },
  ] });
  const document = await call('get_document', { deck_id });
  assert.equal(document.parts.length, 1);
  assert.equal(document.parts[0].spec.preset, 'contrast/panels');
  assert.equal(document.parts[0].stale, false);
  await call('undo', { deck_id });
  assert.equal((await call('get_deck_summary', { deck_id })).hash, base.hash);
  await call('redo', { deck_id });
  assert.equal((await call('get_deck_summary', { deck_id })).hash, document.hash);
  const measured = await call('measure_layout', { deck_id });
  assert.ok(!measured.measurements.some(measurement => measurement.overflow || measurement.missing_glyphs), 'Text layout must fit with available glyphs');
  const preflight = await call('preflight_presentation', { deck_id, options: { page_indices: [0], min_font_size: 10 } });
  assert.ok(!preflight.findings.some(finding => ['CONTAINER_PADDING', 'IMAGE_ASPECT_DISTORTED', 'TEXT_OVERFLOW', 'TEXT_CLIPPED', 'MISSING_GLYPHS'].includes(finding.code)), 'Comparison geometry and text must pass preflight');
  if (values['font-family']) assert.ok(!preflight.findings.some(finding => finding.code === 'FONT_FALLBACK'), 'Use an installed font that covers all text');
  const preview = await invoke('preview_presentation', { deck_id, options: { page_indices: [0], format: 'png', max_dimension: 1280, overflow: 'error' } });
  const image = preview.content.find(content => content.type === 'image');
  assert.ok(image, 'Expected a rendered slide image');
  await writeFile(join(directory, 'comparison-panels.png'), Buffer.from(image.data, 'base64'), { flag: 'wx' });
  await call('export_pptx', { deck_id, filename: 'comparison-panels.pptx' });
  const bytes = await readFile(join(directory, 'comparison-panels.pptx'));
  assert.equal(bytes.subarray(0, 2).toString(), 'PK');
  const reopened = await call('open_pptx', { base64: bytes.toString('base64') });
  const restored = await call('get_document', { deck_id: reopened.deck_id });
  assert.equal(restored.parts[0].spec.preset, 'contrast/panels');
  assert.equal(restored.parts[0].stale, false);
  assert.deepEqual(restored.parts[0].spec.data.panels.map(panel => panel.items.map(item => item.text)), input.panels.map(panel => panel.items.map(item => item.text)));
  const changedSpec = structuredClone(restored.parts[0].spec);
  changedSpec.data.panels[0].items[0].text = 'Validation';
  await call('update_part', { deck_id: reopened.deck_id, expected_revision: restored.revision, slide_id: restored.deck.slides[0].id, id: restored.parts[0].element_id, spec: changedSpec });
  await call('undo', { deck_id: reopened.deck_id });
  assert.equal((await call('get_deck_summary', { deck_id: reopened.deck_id })).hash, restored.hash);
  await call('export_pptx', { deck_id: reopened.deck_id, filename: 'comparison-panels-undo.pptx' });
  assert.deepEqual(await readFile(join(directory, 'comparison-panels-undo.pptx')), bytes);
  const proof = { format: 'aislide.comparison-panels-example', version: 1, page_count: 1, sha256: createHash('sha256').update(bytes).digest('hex'), preset: spec.preset, input_source: values.input ? 'caller-supplied' : 'synthetic', icon_source: 'Lucide, local installed library', icon_assets: icons.size, body_font_size: 18, undo_redo_verified: true, native_reopen_verified: true, native_update_undo_verified: true, office_visual_parity: false, preflight, calls };
  await writeFile(join(directory, 'proof.json'), JSON.stringify(proof, null, 2), { flag: 'wx' });
  console.log(JSON.stringify({ directory, pptx: join(directory, 'comparison-panels.pptx'), preview: join(directory, 'comparison-panels.png'), sha256: proof.sha256, preflight_findings: preflight.findings?.length ?? null, calls: calls.length }));
} finally { await client.close(); await transport.close(); }
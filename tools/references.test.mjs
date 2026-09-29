import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { unzipSync, strFromU8 } from 'fflate';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

test('reference workflow uses real MCP core, Undo and PPTX/PDF delivery', { timeout: 120_000 }, async () => {
  const retained = process.env.AISLIDE_REFERENCE_ARTIFACTS;
  const output = retained ?? await mkdtemp(join(tmpdir(), 'aislide-references-'));
  await mkdir(output, { recursive: true });
  const transport = new StdioClientTransport({ command: process.execPath, args: [fileURLToPath(new URL('./mcp.mjs', import.meta.url)), '--output-dir', output], env: process.env.AISLIDE_CORE_BINARY ? { AISLIDE_CORE_BINARY: process.env.AISLIDE_CORE_BINARY } : undefined });
  const client = new Client({ name: 'reference-verification', version: '1.0.0' });
  const call = async (name, args) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content.find(item => item.type === 'text').text);
  };
  try {
    await client.connect(transport);
    let created = await call('create_presentation', { title: 'Reference distribution test (synthetic)' });
    created = await call('apply_operations', { deck_id: created.deck_id, expected_revision: created.revision, expected_hash: created.hash, operations: [
      { op: 'add_elements', slide_id: 'slide-1', elements: [
        { type: 'text', id: 'heading', x: 64, y: 64, width: 1152, height: 80, text: 'Documentation entry points', font_size: 36, color: '202525', bold: true },
        { type: 'text', id: 'body', x: 64, y: 200, width: 1152, height: 220, text: 'Microsoft Learn: Azure [1]\nMicrosoft Learn: Architecture Center [2]\n\nSynthetic reference-distribution verification fixture.', font_size: 24, color: '202525', bold: false },
        { type: 'text', id: 'existing-source', x: 64, y: 680, width: 1050, height: 24, text: 'Microsoft Learn', font_size: 16, color: '202525', bold: false },
        { type: 'text', id: 'page-number', x: 1160, y: 680, width: 56, height: 24, text: '1', font_size: 16, color: '202525', bold: false },
      ] },
      { op: 'update_notes', slide_id: 'slide-1', notes: 'Verification notes. Sources: https://learn.microsoft.com/azure/ https://learn.microsoft.com/azure/architecture/' },
    ] });
    const options = { placement: 'appendix', entries: [
      { id: 'learn', name: 'Microsoft Learn: Azure', url: 'https://learn.microsoft.com/azure/', slide_ids: ['slide-1'], publish: true },
      { id: 'architecture', name: 'Microsoft Learn: Architecture Center', url: 'https://learn.microsoft.com/azure/architecture/', slide_ids: ['slide-1'], publish: true },
      { id: 'private', name: 'Unapproved local source', url: 'file:///private/DO_NOT_PUBLISH.pdf', slide_ids: ['slide-1'] },
    ] };
    const updated = await call('set_references', { deck_id: created.deck_id, expected_revision: created.revision, expected_hash: created.hash, options });
    assert.equal(updated.revision, created.revision + 1);
    assert.deepEqual(updated.publication, { supplied: 3, published: 2, excluded: 1 });
    const repeated = await call('set_references', { deck_id: created.deck_id, expected_revision: updated.revision, expected_hash: updated.hash, options });
    assert.equal(repeated.hash, updated.hash);
    const exported = await call('finalize_presentation', { deck_id: created.deck_id, expected_revision: updated.revision, expected_hash: updated.hash, name: 'references', options: { pdf: true, preview: 'contact_sheet', preflight: true, max_dimension: 1280 }, include_images: false });
    const presentation = exported.files.find(file => file.kind === 'pptx');
    const pdf = exported.files.find(file => file.kind === 'pdf');
    assert.ok(presentation && pdf);
    const parts = unzipSync(await readFile(presentation.path));
    const xml = Object.entries(parts).filter(([name]) => name.endsWith('.xml') || name.endsWith('.rels')).map(([, bytes]) => strFromU8(bytes)).join('\n');
    assert.match(xml, /https:\/\/learn\.microsoft\.com\/azure\//);
    assert.doesNotMatch(xml, /DO_NOT_PUBLISH/);
    assert.equal((await readFile(pdf.path)).subarray(0, 5).toString(), '%PDF-');
    const checked = await call('preflight_presentation', { deck_id: created.deck_id, options: { page_indices: [0, 1] } });
    assert.ok(!checked.findings.some(finding => ['TEXT_OVERFLOW', 'TEXT_OVERLAP', 'OFF_SLIDE', 'SOURCE_URL_NOT_VISIBLE', 'REFERENCE_COLLISION'].includes(finding.code)), JSON.stringify(checked.findings));
    const undone = await call('undo', { deck_id: created.deck_id });
    assert.equal(undone.hash, created.hash);
    console.log(JSON.stringify({ output, pptx: presentation.path, pdf: pdf.path, checked: 2, retained: Boolean(retained) }));
  } finally {
    await client.close();
    if (!retained) await rm(output, { recursive: true, force: true });
  }
});

test('reference guidance and CI include the publication workflow within the skill budget', async () => {
  const skill = await readFile(new URL('../.github/skills/aislide-authoring/SKILL.md', import.meta.url), 'utf8');
  assert.ok(Buffer.byteLength(skill) <= 6000);
  for (const required of ['set_references', 'publication.excluded', '616..672', 'REFERENCE_COLLISION']) assert.ok(skill.includes(required));
  const server = await readFile(new URL('./mcp.mjs', import.meta.url), 'utf8');
  assert.match(server, /Before customer\/PDF distribution, call set_references/);
  const manifest = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  assert.ok(manifest.scripts['test:mcp'].includes('tools/references.test.mjs'));
  const workflow = await readFile(new URL('../.github/workflows/verify.yml', import.meta.url), 'utf8');
  assert.match(workflow, /cargo\.mjs test --workspace --locked/);
  assert.match(workflow, /run: npm run test:mcp/);
});
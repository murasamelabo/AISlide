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

test('appendix-only publication explicitly handles a dense slide through real MCP', { timeout: 120_000 }, async () => {
  const output = await mkdtemp(join(tmpdir(), 'aislide-appendix-only-'));
  const transport = new StdioClientTransport({ command: process.execPath, args: [fileURLToPath(new URL('./mcp.mjs', import.meta.url)), '--output-dir', output], env: process.env.AISLIDE_CORE_BINARY ? { AISLIDE_CORE_BINARY: process.env.AISLIDE_CORE_BINARY } : undefined });
  const client = new Client({ name: 'appendix-only-verification', version: '1.0.0' });
  const call = async (name, args) => {
    const result = await client.callTool({ name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result.content));
    return JSON.parse(result.content.find(item => item.type === 'text').text);
  };
  try {
    await client.connect(transport);
    let original = await call('create_presentation', { title: 'Private source title' });
    original = await call('apply_operations', { deck_id: original.deck_id, expected_revision: original.revision, expected_hash: original.hash, operations: [
      { op: 'add_elements', slide_id: 'slide-1', elements: [{ type: 'text', id: 'dense-body', x: 0, y: 0, width: 1280, height: 720, text: 'Synthetic dense body without a marker band', font_size: 24, color: '202525', bold: false }] },
      { op: 'update_notes', slide_id: 'slide-1', notes: 'Private speaker note. https://learn.microsoft.com/azure/' },
    ] });
    const entries = [
      { id: 'learn', name: 'Microsoft Learn: Azure', url: 'https://learn.microsoft.com/azure/', slide_ids: ['slide-1'], publish: true },
      { id: 'private', name: 'Unapproved source', url: 'file:///DO_NOT_PUBLISH.pdf', slide_ids: ['slide-1'] },
    ];
    const guard = { deck_id: original.deck_id, expected_revision: original.revision, expected_hash: original.hash };
    const blocked = await client.callTool({ name: 'set_references', arguments: { ...guard, options: { placement: 'appendix', entries } } });
    assert.equal(blocked.isError, true);
    assert.match(JSON.stringify(blocked.content), /candidate frame.*dense-body.*appendix_only/);
    assert.equal((await call('get_deck_summary', { deck_id: original.deck_id })).hash, original.hash);
    const updated = await call('set_references', { ...guard, options: { placement: 'appendix_only', entries } });
    assert.deepEqual(updated.publication, { supplied: 2, published: 1, excluded: 1 });
    const invalid = await client.callTool({ name: 'set_references', arguments: { deck_id: original.deck_id, expected_revision: updated.revision, expected_hash: updated.hash, options: { placement: 'appendix_only', entries: [{ ...entries[0], url: 'file:///DO_NOT_PUBLISH.pdf' }] } } });
    assert.equal(invalid.isError, true);
    assert.equal((await call('get_deck_summary', { deck_id: original.deck_id })).hash, updated.hash);
    const exported = await call('finalize_presentation', { deck_id: original.deck_id, expected_revision: updated.revision, expected_hash: updated.hash, name: 'appendix-only', options: { pdf: true, preview: 'contact_sheet', preflight: true, max_dimension: 640 }, include_images: false });
    const presentation = exported.files.find(file => file.kind === 'pptx');
    const pdf = exported.files.find(file => file.kind === 'pdf');
    assert.ok(presentation && pdf);
    const parts = unzipSync(await readFile(presentation.path));
    const source = strFromU8(parts['ppt/slides/slide1.xml']);
    const appendix = strFromU8(parts['ppt/slides/slide2.xml']);
    assert.match(source, /Synthetic dense body without a marker band/);
    assert.doesNotMatch(source, /aislide-ref-|Slides:|learn\.microsoft/);
    assert.match(appendix, /Slides: 1/);
    assert.match(appendix, /Microsoft Learn: Azure/);
    assert.match(appendix, /https:\/\/learn\.microsoft\.com\/azure\//);
    assert.doesNotMatch(appendix, /Private source title|Private speaker note|DO_NOT_PUBLISH|Unapproved source/);
    assert.match(strFromU8(parts['ppt/slides/_rels/slide2.xml.rels']), /https:\/\/learn\.microsoft\.com\/azure\//);
    assert.equal((await readFile(pdf.path)).subarray(0, 5).toString(), '%PDF-');
    const checked = await call('preflight_presentation', { deck_id: original.deck_id, options: { page_indices: [0, 1] } });
    assert.ok(!checked.findings.some(finding => ['TEXT_OVERFLOW', 'SOURCE_URL_NOT_VISIBLE', 'REFERENCE_COLLISION'].includes(finding.code)), JSON.stringify(checked.findings));
    const undone = await call('undo', { deck_id: original.deck_id });
    assert.equal(undone.hash, original.hash);
  } finally {
    await client.close();
    await rm(output, { recursive: true, force: true });
  }
});

test('slide planning covers 33 purposes with bounded routing and portable references', async () => {
  const root = new URL('../.github/skills/slide-planning/SKILL.md', import.meta.url);
  const skill = await readFile(root, 'utf8');
  assert.match(skill.replace(/^\uFEFF/, ''), /^---\r?\nname: slide-planning\r?\n/);
  assert.ok(Buffer.byteLength(skill) <= 6000, 'Keep the planning entry point compact');
  for (const required of ['Set one `primary_purpose`', 'secondary_purpose', 'null by default', 'allow at most one', 'wording-only', 'approved structure', 'same ledger', 'return instead of invoking it again', 'tech-deck-ja', 'japanese-editing', 'english-editing', 'aislide-authoring']) {
    assert.ok(skill.includes(required), `Missing planning boundary: ${required}`);
  }
  const families = {
    organization: ['company-introduction', 'recruiting-pitch', 'culture-deck', 'employee-onboarding', 'portfolio'],
    commercial: ['service-introduction', 'sales-proposal', 'product-demo', 'customer-case', 'partnership-proposal'],
    management: ['fundraising-pitch', 'business-plan', 'growth-strategy', 'financial-results', 'decision-proposal', 'sustainability-report'],
    delivery: ['project-kickoff', 'progress-report', 'retrospective', 'change-announcement', 'all-hands'],
    learning: ['training', 'procedure', 'workshop', 'technical-explanation', 'research-presentation', 'research-report'],
    public: ['keynote', 'product-launch', 'public-briefing', 'policy-proposal', 'incident-briefing', 'creative-proposal'],
  };
  const catalogUrl = new URL('references/catalog.md', root);
  const catalog = await readFile(catalogUrl, 'utf8');
  const rows = [...catalog.matchAll(/^\| `([a-z-]+)` \| ([a-z]+) \| ([^|]+) \| \[[^\]]+\]\(([^)]+)\) \|$/gm)];
  assert.equal(rows.length, 33);
  assert.equal(new Set(rows.map(row => row[1])).size, 33);
  const pages = new Map([[root.href, skill], [catalogUrl.href, catalog]]);
  for (const [family, purposes] of Object.entries(families)) {
    assert.deepEqual(rows.filter(row => row[2] === family).map(row => row[1]), purposes);
    const familyUrl = new URL(`references/${family}.md`, root);
    const content = await readFile(familyUrl, 'utf8');
    pages.set(familyUrl.href, content);
    const sections = content.split(/^## /m).slice(1);
    assert.equal(sections.length, purposes.length);
    for (const purpose of purposes) {
      const section = sections.find(section => section.startsWith(`${purpose}\n`) || section.startsWith(`${purpose}\r\n`));
      assert.ok(section, `Missing purpose: ${purpose}`);
      for (const field of ['読者・到達点', '必須材料', '構成例', '表現', '確認']) assert.ok(section.includes(field), `Missing ${field} for ${purpose}`);
      assert.equal(rows.find(row => row[1] === purpose)[4], `${family}.md#${purpose}`);
    }
  }
  const checksUrl = new URL('references/checks-and-sources.md', root);
  const checks = await readFile(checksUrl, 'utf8');
  pages.set(checksUrl.href, checks);
  for (const required of ['webinar', 'fundraising-pitch', 'sales-proposal', 'wording-only', 'secondary_purpose', 'slideland.tech', 'sequoiacap.com', 'mitcommlab.mit.edu']) assert.ok(checks.includes(required), `Missing scenario or source: ${required}`);
  for (const [base, content] of pages) {
    for (const match of content.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
      if (/^https?:\/\//.test(match[1])) continue;
      const target = new URL(match[1], base);
      const anchor = target.hash.slice(1);
      target.hash = '';
      const destination = await readFile(target, 'utf8');
      if (anchor) assert.ok(destination.split(/\r?\n/).includes(`## ${anchor}`), `Missing anchor: ${match[1]}`);
    }
  }
  const routing = {
    'aislide-authoring': /Bounded edits skip planning/,
    'tech-deck-ja': /編集スキルや実行担当をここから再呼び出ししない/,
    'japanese-editing': /文言の修正だけなら構成スキルを起動しない/,
    'english-editing': /Wording-only edits skip\s+`slide-planning`/,
  };
  for (const [name, boundary] of Object.entries(routing)) {
    const entry = await readFile(new URL(`../${name}/SKILL.md`, root), 'utf8');
    assert.ok(entry.includes('slide-planning'), `Missing routing for ${name}`);
    assert.match(entry, boundary, `Missing no-recursion boundary for ${name}`);
    if (name === 'tech-deck-ja') assert.ok(entry.includes('以下はPPTX実行も依頼された場合だけ適用'));
  }
  const guide = await readFile(new URL('../README.md', root), 'utf8');
  assert.ok(guide.includes('slide-planning/SKILL.md'));
});

test('English editing skill covers English structure and presentation-specific meaning checks', async () => {
  const skillUrl = new URL('../.github/skills/english-editing/SKILL.md', import.meta.url);
  const skill = await readFile(skillUrl, 'utf8');
  assert.match(skill.replace(/^\uFEFF/, ''), /^---\r?\nname: english-editing\r?\n/);
  assert.ok(Buffer.byteLength(skill) <= 6000, 'Keep the English editing entry point compact');
  for (const required of ['subject', 'verb', 'articles', 'modal', 'parallel', 'US', 'UK', 'AI detector', './references/presentation.md', './references/checks-and-sources.md']) {
    assert.ok(skill.includes(required), `Missing English editing guidance: ${required}`);
  }
  const pages = new Map([[skillUrl, skill]]);
  for (const path of ['references/presentation.md', 'references/checks-and-sources.md']) {
    const url = new URL(path, skillUrl);
    pages.set(url, await readFile(url, 'utf8'));
  }
  const presentation = [...pages.values()][1];
  for (const required of ['sentence case', 'speaker notes', 'not statistically significant', 'parallel', 'percentage points', 'do not shrink']) {
    assert.ok(presentation.includes(required), `Missing English presentation rule: ${required}`);
  }
  const checks = [...pages.values()][2];
  for (const required of ['Synthetic', 'Keep unchanged', 'Ask, do not guess', 'may not', 'not all', 'Unknown actor', 'No causal upgrade', 'https://www.archives.gov/', 'https://github.com/blader/humanizer']) {
    assert.ok(checks.includes(required), `Missing English preservation case or source: ${required}`);
  }
  for (const [base, content] of pages) {
    for (const match of content.matchAll(/\[([^\]]+)\]\(([^)]+)\)/g)) {
      if (/^https:\/\//.test(match[2])) continue;
      await readFile(new URL(match[2].split('#')[0], base), 'utf8');
    }
  }
  const authoring = await readFile(new URL('../.github/skills/aislide-authoring/SKILL.md', import.meta.url), 'utf8');
  assert.match(authoring, /\.\.\/english-editing\/SKILL\.md/);
  const guide = await readFile(new URL('../.github/skills/README.md', import.meta.url), 'utf8');
  assert.match(guide, /english-editing\/SKILL\.md/);
});

test('Japanese editing skill is portable and separates meaning from presentation formatting', async () => {
  const skillUrl = new URL('../.github/skills/japanese-editing/SKILL.md', import.meta.url);
  const skill = await readFile(skillUrl, 'utf8');
  assert.match(skill.replace(/^\uFEFF/, ''), /^---\r?\nname: japanese-editing\r?\n/);
  assert.ok(Buffer.byteLength(skill) <= 6000, 'Keep the editing entry point compact');
  for (const required of ['意味の保持', '数値', '断定', '提案', 'メール', 'プレゼン', '不明', '引用', 'AI判定', 'references/presentation.md']) {
    assert.ok(skill.includes(required), `Missing Japanese editing guidance: ${required}`);
  }
  const presentationUrl = new URL('references/presentation.md', skillUrl);
  const presentation = await readFile(presentationUrl, 'utf8');
  for (const required of ['投影', '配布', '見出し', '箇条書き', '図表', 'ノート', '母数', '因果', '文字を縮小', '架空例', '変更しない']) {
    assert.ok(presentation.includes(required), `Missing presentation editing guidance: ${required}`);
  }
  for (const [base, content] of [[skillUrl, skill], [presentationUrl, presentation]]) {
    for (const match of content.matchAll(/\[([^\]]+)\]\(([^)]+)\)/g)) {
      const target = match[2];
      if (/^https:\/\//.test(target)) continue;
      await readFile(new URL(target.split('#')[0], base), 'utf8');
    }
  }
  for (const name of ['tech-deck-ja', 'aislide-authoring']) {
    const entry = await readFile(new URL(`../.github/skills/${name}/SKILL.md`, import.meta.url), 'utf8');
    assert.match(entry, /\.\.\/japanese-editing\/SKILL\.md/);
  }
  const guide = await readFile(new URL('../.github/skills/README.md', import.meta.url), 'utf8');
  assert.match(guide, /japanese-editing\/SKILL\.md/);
});

test('reference guidance and CI include the publication workflow within the skill budget', async () => {
  const skill = await readFile(new URL('../.github/skills/aislide-authoring/SKILL.md', import.meta.url), 'utf8');
  assert.ok(Buffer.byteLength(skill) <= 6000);
  for (const required of ['Retain attribution.', 'legacy stretch', '1-48 nodes', '64 edges', '1-8 columns', 'default 3', 'no groups or node resizing', '`null` resets defaults', 'empty input clears', 'set_references', 'publication.excluded', 'title:"参考資料"']) {
    assert.ok(skill.includes(required), `Missing authoring guidance: ${required}`);
  }
  assert.match(skill, /\[distribution references\]\(references\/distribution-references\.md\)/);
  const references = await readFile(new URL('../.github/skills/aislide-authoring/references/distribution-references.md', import.meta.url), 'utf8');
  for (const required of ['publish:true', 'slide_ids', 'publication.excluded', '616..672', 'REFERENCE_COLLISION', 'appendix_only', 'bottom quarter', 'page numbers', '256', 'subset', 'never auto-publish', '/azure/', '/azure']) {
    assert.ok(references.includes(required), `Missing distribution guidance: ${required}`);
  }
  const technical = await readFile(new URL('../.github/skills/aislide-authoring/references/technical-panels.md', import.meta.url), 'utf8');
  assert.match(technical, /appendix_only/);
  assert.match(technical, /bottom quarter/);
  const documentation = await readFile(new URL('../docs/authoring/references.md', import.meta.url), 'utf8');
  for (const required of ['appendix_only', 'bottom quarter', '1-based', '256', 'subset', 'Undo', 'PPTX']) assert.ok(documentation.includes(required), `Missing reference documentation: ${required}`);
  const server = await readFile(new URL('./mcp.mjs', import.meta.url), 'utf8');
  assert.match(server, /Before customer\/PDF distribution, call set_references/);
  const manifest = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8'));
  assert.ok(manifest.scripts['test:mcp'].includes('tools/references.test.mjs'));
  const workflow = await readFile(new URL('../.github/workflows/verify.yml', import.meta.url), 'utf8');
  assert.match(workflow, /cargo\.mjs test --workspace --locked/);
  assert.match(workflow, /run: npm run test:mcp/);
});
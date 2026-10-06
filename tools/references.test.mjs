import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { unzipSync, strFromU8 } from 'fflate';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';
import { requestCore } from './core-client.mjs';

const specialistFamilies = {
  organization: { name: 'organization-deck-ja', purposes: ['company-introduction', 'recruiting-pitch', 'culture-deck', 'employee-onboarding', 'portfolio'] },
  commercial: { name: 'commercial-deck-ja', purposes: ['service-introduction', 'sales-proposal', 'product-demo', 'customer-case', 'partnership-proposal'] },
  management: { name: 'management-deck-ja', purposes: ['fundraising-pitch', 'business-plan', 'growth-strategy', 'financial-results', 'decision-proposal', 'sustainability-report'] },
  delivery: { name: 'delivery-deck-ja', purposes: ['project-kickoff', 'progress-report', 'retrospective', 'change-announcement', 'all-hands'] },
  learning: { name: 'learning-deck-ja', purposes: ['training', 'procedure', 'workshop', 'technical-explanation', 'research-presentation', 'research-report'] },
  public: { name: 'public-deck-ja', purposes: ['keynote', 'product-launch', 'public-briefing', 'policy-proposal', 'incident-briefing', 'creative-proposal'] },
};

const pilotPurposes = {
  organization: 'recruiting-pitch',
  commercial: 'sales-proposal',
  management: 'decision-proposal',
  delivery: 'progress-report',
  learning: 'training',
  public: 'keynote',
};

function validateExampleLedger(section, purpose) {
  const example = section.split('### ページ設計とノート')[1].split('### 図表と部品の選定')[0];
  const lines = example.split(/\r?\n/).filter(line => /^\s*\|\s*S\d+/.test(line));
  const slides = lines.map(line => {
    const row = line.match(/^\| S([1-9]\d*) \| ([^|]*) \| ([^|]*) \| ([^|]*) \| ([^|]*) \|$/);
    assert.ok(row, `Malformed ledger row in ${purpose}: ${line}`);
    for (const cell of row.slice(2)) assert.ok(cell.trim().length > 0, `Empty ledger cell in ${purpose}`);
    return row;
  });
  assert.ok(slides.length >= 4 && slides.length <= 8, `${purpose} needs a bounded, coherent page example`);
  assert.deepEqual(slides.map(row => Number(row[1])), slides.map((_, index) => index + 1));
  const declaredInputs = new Set(example.split('| ID |')[0].match(/F\d+/g));
  for (const row of slides) {
    for (const source of row[3].match(/F\d+/g) ?? []) assert.ok(declaredInputs.has(source), `Undeclared example source ${source} in ${purpose}`);
  }
  return slides.length;
}

test('purpose guide ledger validation rejects empty and malformed trailing rows', () => {
  const header = '### ページ設計とノート\n架空例: F1=example input\n| ID | 役割・見出し | 本文と根拠 | 図表案 | ノート |\n';
  const rows = Array.from({ length: 4 }, (_, index) => `| S${index + 1} | Role | Body / F1 | Table | Note |`).join('\n');
  const section = extra => `${header}${rows}\n${extra}\n### 図表と部品の選定\n`;
  assert.equal(validateExampleLedger(section(''), 'synthetic'), 4);
  assert.throws(() => validateExampleLedger(section('| S5 |  | F999 | chart | notes |'), 'synthetic'), /Empty ledger cell/);
  assert.throws(() => validateExampleLedger(section('| S5 | Role | F1 | notes |'), 'synthetic'), /Malformed ledger row/);
  assert.throws(() => validateExampleLedger(section('| S5 | Role | F999 | chart | notes |'), 'synthetic'), /Undeclared example source/);
});

for (const [family, { purposes }] of Object.entries(specialistFamilies)) {
  for (const purpose of purposes.filter(purpose => purpose !== 'technical-explanation')) {
    test(`purpose guide ${purpose} provides actionable decisions and a synthetic ledger`, async () => {
      const guide = await readFile(new URL(`../.github/skills/slide-planning/references/${family}.md`, import.meta.url), 'utf8');
      const section = guide.split(/^## /m).find(section => section.startsWith(`${purpose}\n`) || section.startsWith(`${purpose}\r\n`));
      assert.ok(section, `Missing purpose: ${purpose}`);
      for (const heading of ['判断と境界', '入力と不足時の分岐', '構成の分岐', 'ページ設計とノート', '図表と部品の選定', '失敗例と修正', '完了基準']) {
        assert.ok(section.includes(`### ${heading}`), `Missing ${purpose} guidance: ${heading}`);
      }
      assert.ok(section.includes('架空例'), 'Ledger examples must be explicitly synthetic');
      validateExampleLedger(section, purpose);
      assert.ok(section.includes('不合格'), 'State when the purpose-specific result is not ready');
      if (purpose === 'progress-report') assert.ok(section.includes('未設定・不明・異なるを言い換えない'));
      if (purpose === 'product-launch') {
        assert.ok(section.includes('発表内容の確認状態と素材の有無を分ける'));
        assert.ok(section.includes('提供条件は未確認'));
      }
      if (purpose === 'incident-briefing') {
        assert.ok(section.includes('未確認と調査中を同一視しない'));
        assert.ok(section.includes('影響: 範囲は未確認'));
        assert.doesNotMatch(section, /不足なら調査中/);
      }
      if (purpose === 'change-announcement') assert.ok(section.includes('採否を決めることが主目的の場合だけ decision-proposal'));
    });
  }
}

test('purpose guide part references exist in the current core catalog', async () => {
  const catalog = await requestCore({ op: 'part_catalog' });
  const presets = new Set(catalog.presets.map(preset => preset.id));
  assert.ok(presets.size > 0);
  for (const family of Object.keys(specialistFamilies)) {
    const guide = await readFile(new URL(`../.github/skills/slide-planning/references/${family}.md`, import.meta.url), 'utf8');
    for (const match of guide.matchAll(/`([a-z][a-z0-9-]+\/[a-z0-9-]+)`/g)) {
      assert.ok(presets.has(match[1]), `Unknown ${family} guide preset: ${match[1]}`);
    }
  }
});

test('purpose guide comparison fixes 18 synthetic inputs before model evaluation', async () => {
  const checks = await readFile(new URL('../.github/skills/slide-planning/references/checks-and-sources.md', import.meta.url), 'utf8');
  const cases = [...checks.matchAll(/^\| `([a-z-]+\.(?:complete|gap|boundary))` \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$/gm)];
  assert.equal(cases.length, 18);
  assert.equal(new Set(cases.map(row => row[1])).size, 18);
  for (const purpose of Object.values(pilotPurposes)) {
    for (const condition of ['complete', 'gap', 'boundary']) {
      assert.ok(cases.some(row => row[1] === `${purpose}.${condition}`), `Missing ${purpose} ${condition}`);
    }
  }
  for (const required of ['すべて架空例', '用途別ガイドだけを差し替える', '取得できなければ不明', '普遍的な優越性を主張しない']) assert.ok(checks.includes(required));
});

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
  const catalogUrl = new URL('references/catalog.md', root);
  const catalog = await readFile(catalogUrl, 'utf8');
  const rows = [...catalog.matchAll(/^\| `([a-z-]+)` \| ([a-z]+) \| ([^|]+) \| \[[^\]]+\]\(([^)]+)\) \| \[[a-z-]+\]\([^)]+\) \|$/gm)];
  assert.equal(rows.length, 33);
  assert.equal(new Set(rows.map(row => row[1])).size, 33);
  const pages = new Map([[root.href, skill], [catalogUrl.href, catalog]]);
  for (const [family, { purposes }] of Object.entries(specialistFamilies)) {
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

test('Japanese specialist routing covers all 33 purposes without duplicate assignments', async () => {
  const catalogUrl = new URL('../.github/skills/slide-planning/references/catalog.md', import.meta.url);
  const catalog = await readFile(catalogUrl, 'utf8');
  const rows = [...catalog.matchAll(/^\| `([a-z-]+)` \| ([a-z]+) \| [^|]+ \| \[[^\]]+\]\(([^)]+)\) \| \[([a-z-]+)\]\(([^)]+)\) \|$/gm)];
  assert.ok(catalog.includes('構成を依頼されている場合だけ個別案を返し、担当候補の提示だけならそこで止める'));
  assert.equal(rows.length, 33, 'Every catalog purpose needs a Japanese specialist');
  assert.equal(new Set(rows.map(row => row[1])).size, 33);
  assert.equal(new Set(rows.map(row => row[4])).size, 7);
  for (const [family, { name, purposes }] of Object.entries(specialistFamilies)) {
    assert.deepEqual(rows.filter(row => row[2] === family).map(row => row[1]), purposes);
    for (const purpose of purposes) {
      const row = rows.find(row => row[1] === purpose);
      const specialist = purpose === 'technical-explanation' ? 'tech-deck-ja' : name;
      assert.equal(row[3], `${family}.md#${purpose}`);
      assert.equal(row[4], specialist);
      assert.equal(row[5], `../../${specialist}/SKILL.md`);
      await readFile(new URL(row[5], catalogUrl), 'utf8');
    }
  }
});

for (const [family, { name, purposes }] of Object.entries(specialistFamilies)) {
  test(`Japanese specialist ${name} preserves purpose detail and caller boundaries`, async () => {
    const entryUrl = new URL(`../.github/skills/${name}/SKILL.md`, import.meta.url);
    const entry = await readFile(entryUrl, 'utf8');
    const frontmatter = entry.match(/^\uFEFF---\r?\n([\s\S]*?)\r?\n---\r?\n/);
    assert.ok(frontmatter, 'Skill frontmatter must have opening and closing delimiters');
    assert.match(frontmatter[1], new RegExp(`^name: ${name}\\r?$`, 'm'));
    const description = frontmatter[1].match(/^description: '([^'\r\n]+)'\r?$/m)?.[1];
    assert.ok(description && description.length <= 1024, 'Use a compact, quoted discovery description');
    assert.ok(description.includes('日本語'));
    const windowsBytes = Buffer.byteLength(entry.replace(/\r?\n/g, '\r\n'));
    assert.ok(windowsBytes <= 6000, `${name} exceeds the Windows checkout skill budget: ${windowsBytes}`);
    for (const required of [
      'Japanese planning only', 'When delegated, return the same ledger to the caller.',
      'Do not call planners, specialists, editors or rendering.',
      'For standalone mismatches, hand off to `slide-planning` once; ask if unavailable.',
      'Stop after that handoff.', 'Wording-only edits skip planning.',
      'Explicit restructuring is required to change an approved outline.',
      'Read only the selected purpose sections.', 'Recommendation-only requests stop before planning.',
      'locate `## <purpose-id>` and read through the next level-two heading',
      'Do not reuse fictional F IDs as user evidence.',
      '## Purpose Decisions', '../japanese-editing/SKILL.md', '../aislide-authoring/SKILL.md',
      `../slide-planning/references/${family}.md`,
    ]) assert.ok(entry.includes(required), `Missing ${name} boundary: ${required}`);
    const decisions = [...entry.matchAll(/^\| `([a-z-]+)` \| ([^|]+) \| ([^|]+) \|$/gm)];
    assert.deepEqual(decisions.map(row => row[1]), purposes.filter(purpose => purpose !== 'technical-explanation'));
    for (const row of decisions) {
      assert.ok(row[2].trim().length >= 15, `Missing purpose-specific decision for ${row[1]}`);
      assert.ok(row[3].trim().length >= 15, `Missing visual or facilitation guidance for ${row[1]}`);
    }
    for (const match of entry.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
      assert.ok(!/^https?:/.test(match[1]), 'Specialists should use the existing local guidance');
      const target = new URL(match[1], entryUrl);
      const anchor = target.hash.slice(1);
      target.hash = '';
      const destination = await readFile(target, 'utf8');
      if (anchor) assert.ok(destination.split(/\r?\n/).includes(`## ${anchor}`));
    }
  });
}

test('Japanese specialist selection is disclosed before bounded delegation', async () => {
  const skill = await readFile(new URL('../.github/skills/slide-planning/SKILL.md', import.meta.url), 'utf8');
  for (const required of [
    'Show the selected specialist, reason and target slide IDs before delegation.',
    'Recommendation-only requests stop after the selection; do not invoke a specialist.',
    'Japanese passages only', 'English passages keep the purpose reference and `english-editing`',
    'Invoke each selected specialist at most once', 'secondary purpose is limited to its assigned slides',
    'If unavailable, disclose it and use the matching reference section without installing anything.',
    'specialist_selections',
  ]) assert.ok(skill.includes(required), `Missing specialist routing boundary: ${required}`);
  const recommendationExit = skill.indexOf('Recommendation-only requests stop after the selection; do not invoke a specialist.');
  for (const planningAction of ['## Build one plan', 'propose a custom outline', 'Invoke each selected specialist at most once']) {
    assert.ok(skill.indexOf(planningAction) > recommendationExit, `Recommendation-only exit must precede ${planningAction}`);
  }
});

test('Japanese specialist documentation preserves portable setup and routing scenarios', async () => {
  const guideUrl = new URL('../.github/skills/README.md', import.meta.url);
  const guide = await readFile(guideUrl, 'utf8');
  assert.ok(guide.includes('11スキル'));
  assert.doesNotMatch(guide, /五つとも/);
  assert.ok(guide.includes('slide-planning/references/'));
  const checksUrl = new URL('./slide-planning/references/checks-and-sources.md', guideUrl);
  const checks = await readFile(checksUrl, 'utf8');
  for (const { name } of Object.values(specialistFamilies)) {
    assert.ok(guide.includes(`[${name}](${name}/SKILL.md)`), `Missing specialist guide: ${name}`);
    assert.ok(guide.includes(`<repo>/.github/skills/${name}/`), `Missing setup source: ${name}`);
    assert.ok(guide.includes(`~/.copilot/skills/${name}/`), `Missing user setup: ${name}`);
    assert.ok(guide.includes(`<project>/.github/skills/${name}/`), `Missing workspace setup: ${name}`);
    assert.ok(checks.includes(name), `Missing specialist scenario: ${name}`);
  }
  for (const scenario of ['recommendation-only', 'recommendation-unknown', 'same-specialist', 'secondary-scope', 'mixed-language', 'missing-specialist', 'delegated-mismatch', 'standalone-mismatch']) {
    assert.ok(checks.includes(`| \`${scenario}\` |`), `Missing handoff scenario: ${scenario}`);
  }
  for (const [base, content] of [[guideUrl, guide], [checksUrl, checks]]) {
    for (const match of content.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)) {
      if (/^https?:/.test(match[1])) continue;
      const target = new URL(match[1], base);
      target.hash = '';
      await readFile(target, 'utf8');
    }
  }
});

for (const [name, rules] of [
  ['tech-deck-ja', [
    /description:.*採用・営業提案・会社紹介.*技術が題材にすぎない.*slide-planning/,
    /読者の到達点が技術の理解・判断ではなく/,
    /`slide-planning` へ一度だけ渡す/,
    /未配置なら用途の不一致を伝えて確認する/,
  ]],
  ['english-editing', [
    /Return edits to the caller with the supplied purpose and voice/,
    /start no other\s+deck or research/,
  ]],
  ['aislide-authoring', [
    /Reuse supplied ledgers without replanning/,
    /For new structure without a ledger, use `slide-planning` once; ask if absent/,
  ]],
  ['slide-planning', [
    /Specify delivery separately: live talk, read-alone PDF, hands-on session or recorded talk/,
    /not\s+new MCP parameters or guided `profile_id` values/,
  ]],
]) {
  test(`${name} retains its purpose and handoff boundary when selected directly`, async () => {
    const entry = await readFile(new URL(`../.github/skills/${name}/SKILL.md`, import.meta.url), 'utf8');
    for (const rule of rules) assert.match(entry, rule);
    const windowsBytes = Buffer.byteLength(entry.replace(/\r?\n/g, '\r\n'));
    if (name !== 'tech-deck-ja') assert.ok(windowsBytes <= 6000, `${name} exceeds the Windows checkout skill budget: ${windowsBytes}`);
  });
}

test('purpose catalog distinguishes workshop delivery and slide-only scope', async () => {
  const catalog = await readFile(new URL('../.github/skills/slide-planning/references/catalog.md', import.meta.url), 'utf8');
  assert.match(catalog, /「ワークショップ形式」.*開催形式/);
  assert.match(catalog, /参加者が成果物や合意を作る場合だけ `workshop` を用途にする/);
  assert.match(catalog, /冊子そのものではなく、その内容を説明するスライドを対象とする/);
});

test('skill setup documentation includes purpose planning and the shared installation guide', async () => {
  const guide = new URL('../.github/skills/README.md', import.meta.url);
  for (const path of ['../README.md', '../docs/authoring/README.md']) {
    const page = new URL(path, import.meta.url);
    const content = await readFile(page, 'utf8');
    assert.ok(content.includes('`slide-planning`'), `Missing planning setup in ${path}`);
    assert.doesNotMatch(content, /両方をユーザースコープ|paired installation/);
    const links = [...content.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)];
    assert.ok(links.some(match => new URL(match[1], page).href === guide.href), `Missing shared installation guide in ${path}`);
  }
  await readFile(guide, 'utf8');
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
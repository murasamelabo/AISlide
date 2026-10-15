import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { globSync } from 'node:fs';
import { lstat, mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const synthetic = 'SYNTHETIC / 架空データ';
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const calls = {};
const published = [];
let ownedDirectory;
const client = new AislideClient(async (request, options) => {
  calls[request.op] = (calls[request.op] ?? 0) + 1;
  assert.ok(Object.values(calls).reduce((total, count) => total + count, 0) <= 1200, 'Qualification request budget exceeded');
  return requestCore(request, options);
});

function parseOptions(argv) {
  const options = { smoke: false, skipFont: false, withParts: false, directory: undefined };
  for (const argument of argv) {
    if (argument === '--smoke') options.smoke = true;
    else if (argument === '--skip-font') options.skipFont = true;
    else if (argument === '--with-parts') options.withParts = true;
    else if (argument.startsWith('-')) throw new Error(`Unknown option: ${argument}`);
    else {
      assert.equal(options.directory, undefined, 'Only one new output directory is allowed');
      options.directory = argument;
    }
  }
  options.directory = resolve(options.directory ?? join(root, '.artifacts', `editing-${Date.now()}`));
  return options;
}

function counters(deck) {
  const result = { slides: deck.slides.length, width: deck.width, height: deck.height, masters: deck.design?.masters.length ?? 0, layouts: deck.design?.layouts.length ?? 0, charts: 0, tables: 0, pictures: 0, graphics: 0, groups: 0, notes: 0, objects: 0, connectors: 0, chartKinds: {}, perSlide: [] };
  const visit = (elements, count) => {
    for (const element of elements) {
      count.objects++;
      const field = { chart: 'charts', table: 'tables', picture: 'pictures', group: 'groups', connector: 'connectors' }[element.type];
      if (field) count[field]++;
      if (element.type === 'picture' && element.svg) count.graphics++;
      if (element.type === 'chart') result.chartKinds[element.kind] = (result.chartKinds[element.kind] ?? 0) + 1;
      if (element.type === 'group') visit(element.children, count);
    }
  };
  for (const [index, slide] of deck.slides.entries()) {
    const count = { slide: index + 1, id: slide.id, charts: 0, tables: 0, pictures: 0, graphics: 0, groups: 0, objects: 0, connectors: 0, notes: slide.notes ? 1 : 0, topLevelObjects: slide.elements.length };
    visit(slide.elements, count);
    for (const field of ['charts', 'tables', 'pictures', 'graphics', 'groups', 'objects', 'connectors', 'notes']) result[field] += count[field];
    result.perSlide.push(count);
  }
  return result;
}

async function publish(directory, filename, bytes) {
  assert.equal(filename, filename.split(/[\\/]/).at(-1), 'Flat artifact filenames only');
  assert.equal(directory, ownedDirectory, 'Only the newly created synthetic directory may receive files');
  bytes = Buffer.from(bytes);
  assert.ok(bytes.length <= 16 * 1024 * 1024, 'Artifact budget: 16 MiB per file');
  assert.ok(published.length < 96 && published.reduce((total, file) => total + file.bytes, bytes.length) <= 64 * 1024 * 1024, 'Artifact count/total byte budget exceeded');
  const path = join(directory, filename);
  await writeFile(path, bytes, { flag: 'wx' });
  const actual = await readFile(path);
  assert.deepEqual(actual, Buffer.from(bytes), `Published bytes changed: ${filename}`);
  const entry = { filename, sha256: sha256(actual), bytes: actual.length };
  published.push(entry);
  return entry;
}

function text(id, value, y = 220) {
  return { type: 'text', id, x: 72, y, width: 1120, height: 90, text: value, font_size: 28, color: '@dk1', bold: false };
}

function makeSlide(index, title, elements = [], notes = '') {
  return { id: `slide-${index}`, title, background: '@lt1', elements: [
    { ...text('title', title, 62), font_size: 34, bold: true, height: 80 },
    ...elements,
    { ...text('synthetic-label', synthetic, 656), font_size: 16, height: 36, color: '@dk2' },
  ], notes: `Synthetic fixture only. 架空の検証用データ。${notes}` };
}

const rectangle = (id, x, y, width, height, fill = '@accent1') => ({ type: 'shape', id, x, y, width, height, preset: 'roundRect', fill, stroke: '@lt1', stroke_width: 1, rotation: 0, text: '', font_size: 24, color: '@dk1', bold: false });
const caption = (id, value, x, y, width = 260) => ({ ...text(id, value, y), x, width, height: 92, font_size: 22 });
const elementAt = (session, slideIndex, id) => session.document.deck.slides[slideIndex].elements.find((element) => element.id === id);

async function putElement(session, slideIndex, id, candidate) {
  const index = session.document.deck.slides[slideIndex].elements.findIndex((element) => element.id === id);
  assert.ok(index >= 0, `Element not found: ${id}`);
  return session.transact([
    { op: 'test', path: `/deck/slides/${slideIndex}/elements/${index}/id`, value: id },
    { op: 'replace', path: `/deck/slides/${slideIndex}/elements/${index}`, value: candidate },
  ], { expectedRevision: session.revision });
}

async function fontFixture(options, directory) {
  if (options.skipFont) return { status: 'skipped', reason: 'Explicit --skip-font; embedding not qualified' };
  const matches = globSync(join(root, '.tools/cargo/registry/src/*/cosmic-text-0.19.0/fonts/NotoSans-Regular.ttf').replaceAll('\\', '/')).sort();
  if (!matches.length) return { status: 'skipped', reason: 'Local cosmic-text Noto Sans OFL fixture absent; no network or global font access attempted' };
  const bytes = await readFile(matches[0]);
  const license = await readFile(join(dirname(matches[0]), 'NotoSans-LICENSE'));
  assert.match(license.toString('utf8'), /SIL OPEN FONT LICENSE/);
  assert.ok(bytes.length <= 1024 * 1024, 'Qualification font budget is one font of at most 1 MiB');
  const info = await client.inspectFont(bytes.toString('base64'));
  assert.equal(info.family, 'Noto Sans'); assert.equal(info.usable, true); assert.equal(info.license_verified, false);
  await publish(directory, 'NotoSans-LICENSE.txt', license);
  return { status: 'available', base64: bytes.toString('base64'), info, licenseSha256: sha256(license), consentBasis: 'User authorized the local cosmic-text OFL fixture; accompanying license inspected. No global font registration.' };
}

async function featureDeck(design, font, options) {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180"><rect width="320" height="180" fill="#eef5f4"/><rect x="24" y="92" width="66" height="64" fill="#147d70"/><rect x="126" y="54" width="66" height="102" fill="#2463b4"/><rect x="228" y="24" width="66" height="132" fill="#cc4770"/></svg>';
  const vector = await client.createAsset({ id: 'svg', base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: 'Synthetic geometric bars, no measured quantities', size: 400 });
  assert.ok(vector.svg && vector.base64);
  const raster = await client.createAsset({ id: 'png', base64: vector.base64, mime_type: 'image/png', alt: 'Synthetic raster counterpart', size: 400 });
  const curve = { type: 'polygon', id: 'curve', x: 100, y: 232, width: 430, height: 260, points: [[0, 0], [1, 0], [0, 1]], fill: '@accent1', stroke: '@dk1', stroke_width: 2 };
  const table = await client.createObject({ id: 'table', kind: 'table', rows: 4, columns: 3 });
  Object.assign(table, { x: 72, y: 194, width: 1116, height: 330, font_size: 22, rows: [['架空計画', '', '状態'], ['編集', '12', '準備'], ['評価', '18', '検証'], ['出力', '3', '完了']] });
  const slides = [
    makeSlide(1, 'Rich text / 範囲書式', [text('rich', 'Synthetic rich text', 236), caption('rich-description', '架空の表記例 / Mixed runs', 72, 438, 1000)]),
    makeSlide(2, 'Paragraphs / 段落と箇条書き', [{ ...text('paragraphs', 'Synthetic paragraphs', 190), height: 360 }]),
    makeSlide(3, 'Tables / 列幅・結合・セル書式', [table]),
    makeSlide(4, 'Effects / 塗りと効果', [
      { ...rectangle('gradient', 80, 230, 230, 200), visual: { gradient: { kind: 'linear', angle: 45, stops: [{ offset: 0, color: '@accent1', opacity: 1 }, { offset: 1, color: '@accent2', opacity: 0.65 }] } } },
      { ...rectangle('shadow', 372, 230, 230, 200, '@accent2'), visual: { opacity: 0.75, shadow: { color: '303030', opacity: 0.3, blur: 8, distance: 10, angle: 45 } } },
      { ...rectangle('glow', 664, 230, 230, 200, '@accent3'), visual: { glow: { color: '@accent3', opacity: 0.45, radius: 10 }, soft_edge: 2 } },
      { ...rectangle('reflection', 956, 230, 230, 200, '@accent4'), visual: { reflection: { blur: 2, distance: 8, start_opacity: 0.35, end_opacity: 0, end_position: 0.5 } } },
      caption('effect-labels', 'Gradient     Alpha + shadow     Glow     Reflection', 80, 538, 1120),
    ]),
    makeSlide(5, 'SVG + PNG / 画像とマスク', [{ ...vector, x: 100, y: 212, width: 440, height: 248, visual: { picture_mask: 'round_rect' } }, { ...raster, x: 710, y: 212, width: 440, height: 248 }, caption('svg-caption', 'SVG + native PNG fallback', 100, 504, 510), caption('png-caption', 'PNG / synthetic pixels', 710, 504, 450)]),
    makeSlide(6, 'Vector paths / ベジェと変換', [curve, { ...rectangle('transform', 752, 250, 310, 190, '@accent3'), rotation: 15, visual: { flip_h: true, adjustments: [{ name: 'adj', value: 18000 }] } }]),
    makeSlide(7, 'Selection / 整列・グループ', [rectangle('select-a', 100, 250, 220, 170), rectangle('select-b', 470, 260, 220, 170, '@accent2'), rectangle('select-c', 870, 245, 220, 170, '@accent3'), caption('selection-label', '架空の構成要素 / A, B, C', 100, 494, 1000)]),
    makeSlide(8, 'Masters / 継承とトポロジー', [text('master-body', 'Synthetic master and layout', 240)]),
    makeSlide(9, 'Fields / 日付とスライド番号', [text('fields', 'Page 9 | 9/17/2026', 250)]),
    makeSlide(10, 'Review / ローカルコメント', [text('review-body', '架空レビュー / Local synthetic review', 248), caption('review-limit', 'Replies and resolved state are AISlide extensions.\nNot modern PowerPoint collaboration threads.', 72, 396, 1120)]),
    makeSlide(11, 'Fonts / 権利と可搬性', [{ ...text('font-latin', 'Synthetic Noto Sans: ABC xyz 0123', 226), format: { font_family: font.info?.family ?? '@minor' } }, caption('font-status', font.status === 'available' ? 'Local OFL fixture / one Latin font\n日本語は実際のローカル代替フォントで測定' : 'Embedding SKIPPED / 埋め込み未検証\nLocal OFL fixture unavailable or --skip-font', 72, 390, 1120)]),
    makeSlide(12, 'Recovery / 検査と静的出力', [text('recovery-body', 'Synthetic recovery checkpoint', 232), caption('scope', 'SDK qualification only\nOffice and schema checks are separate.', 72, 388, 1100)]),
  ];
  slides[11].notes = '';
  const session = await client.createDocument({ id: `editing-features-${Date.now()}`, deck: { version: 1, title: `${synthetic} Editing features`, width: 1280, height: 720, design: structuredClone(design), slides } });
  await session.updateParagraphs('slide-1', { id: 'rich', paragraphs: [{ runs: [
    { text: '日本語 ', style: { bold: true, color: '@accent1', language: 'ja-JP' } },
    { text: 'English ', style: { italic: true, underline: true, color: '@accent2', language: 'en-US' } },
    { text: 'H' }, { text: '2', style: { baseline: -25000, font_size: 21 } }, { text: 'O  x' },
    { text: '2', style: { baseline: 30000, font_size: 21 } }, { text: '  Highlight', style: { highlight: 'FFF09A', bold: true } },
  ] }] });
  await session.updateParagraphs('slide-2', { id: 'paragraphs', paragraphs: [
    { runs: [{ text: '架空の計画 / Plan' }], bullet: 'bullet', bullet_character: '•', alignment: 'left', margin_left: 285750, indent: -142875, line_spacing: { kind: 'percent', value: 115000 }, space_after: { kind: 'points', value: 1200 } },
    { runs: [{ text: '架空の評価 / Evaluate' }], bullet: 'numbered', numbering: 'arabicPeriod', number_start: 2, alignment: 'center', line_spacing: { kind: 'percent', value: 120000 }, space_before: { kind: 'points', value: 600 }, space_after: { kind: 'points', value: 1200 } },
    { runs: [{ text: '架空の出力 / Export' }], bullet: 'bullet', level: 1, alignment: 'right', margin_left: 285750, indent: -142875, line_spacing: { kind: 'points', value: 2800 } },
  ] });
  await session.editTable('slide-3', { id: 'table', operations: [
    { op: 'update_format', format: { column_widths: { unit: 'relative', values: [2, 1, 2] }, row_heights: { unit: 'absolute', values: [84, 82, 82, 82] } } },
    { op: 'merge', region: { row: 0, column: 0, row_span: 1, col_span: 2 } },
    { op: 'set_cell_style', row: 0, column: 0, style: { fill: '@accent1', vertical: 'middle', padding: { left: 16, right: 16, top: 8, bottom: 8 }, text_style: { color: '@lt1', bold: true, font_size: 24 } } },
    { op: 'set_cell_style', row: 1, column: 2, style: { fill: 'E4F2ED', outline: { color: '@accent1', width: 2 }, vertical: 'middle', text_style: { bold: true } } },
  ] });
  const path = { commands: [{ op: 'move', point: [0, 0] }, { op: 'cubic', control1: [0.5, 0], control2: [1, 0.5], point: [1, 1] }, { op: 'quadratic', control: [0, 1], point: [0, 0] }, { op: 'close' }] };
  await putElement(session, 5, 'curve', await client.editVector({ element: elementAt(session, 5, 'curve'), path }));
  await session.updateParagraphs('slide-9', { id: 'fields', paragraphs: [{ runs: [
    { text: 'Page ' }, { text: '9', field: { id: '{11111111-1111-4111-8111-111111111111}', kind: 'slidenum' } }, { text: ' | ' },
    { text: '9/17/2026', field: { id: '{22222222-2222-4222-8222-222222222222}', kind: 'datetime1' } },
  ] }] });
  await session.refreshFields('2026-09-17');
  await session.addComment('slide-10', { id: 'review-1', author: 'Fictional reviewer A', initials: 'FA', timestamp: '2026-09-17T10:00:00Z', text: '架空コメント / Check the synthetic example.' });
  if (font.status === 'available') {
    const hash = session.document.hash;
    await assert.rejects(session.embedFont({ base64: font.base64, license_acknowledged: false }), /license/i);
    assert.equal(session.document.hash, hash);
    await session.embedFont({ base64: font.base64, license_acknowledged: true });
    assert.equal((await session.listFonts()).fonts[0].sha256, font.info.sha256);
  }
  if (options.withParts) {
    await session.addPart('slide-12', { id: 'managed-part', spec: { version: 1, preset: 'flow/balanced', title: 'Synthetic stages', data: { kind: 'items', items: [{ label: 'Draft' }, { label: 'Check' }] } } });
    await session.editElements('slide-12', [{ op: 'remove', id: 'recovery-body' }, { op: 'remove', id: 'scope' }]);
  }
  return session;
}

async function qualifyFeatures(session, name, font) {
  const checks = {};
  const step = async (name, action, verify, retain = true) => {
    console.log(`EDITING_CHECK=${name}`);
    const before = session.document.hash;
    await action();
    assert.notEqual(session.document.hash, before, `${name} must edit the document`);
    await verify?.();
    const changed = session.document.hash;
    await session.undo(); assert.equal(session.document.hash, before, `${name} Undo`);
    if (retain) { await session.redo(); assert.equal(session.document.hash, changed, `${name} Redo`); }
    checks[name] = { applied: true, undoHashExact: true, retained: retain };
  };
  await step('G01-rich-range', () => session.formatText('slide-1', { id: 'rich', start: 0, end: 3, style: { bold: true, highlight: 'CDEDE7' } }), () => assert.equal(elementAt(session, 0, 'rich').format.paragraphs[0].runs[0].style.highlight, 'CDEDE7'));
  await step('G02-paragraph-spacing', () => {
    const paragraphs = elementAt(session, 1, 'paragraphs').format.paragraphs;
    paragraphs[0].space_after = { kind: 'points', value: 1500 };
    return session.updateParagraphs('slide-2', { id: 'paragraphs', paragraphs });
  });
  const search = { query: 'Synthetic master', case_sensitive: true };
  assert.equal((await session.searchText(search)).length, 1);
  await step('G04-search-replace', () => session.replaceText({ search, replacement: 'Fictional master', replace_all: true }), () => assert.match(elementAt(session, 7, 'master-body').text, /^Fictional/));
  const ids = ['select-a', 'select-b', 'select-c'];
  await step('G05-multi-move', () => session.editSelection('slide-7', { op: 'translate', ids, dx: 10, dy: 0 }), undefined, false);
  await step('G07-align', () => session.editSelection('slide-7', { op: 'align', ids, alignment: 'top', relative_to: 'selection' }));
  await step('G07-distribute', () => session.editSelection('slide-7', { op: 'distribute', ids, axis: 'horizontal', relative_to: 'selection' }));
  const copyHash = session.document.hash;
  const copied = await session.editSelection('slide-7', { op: 'copy', ids: ['select-a'], format: 'keep_source_formatting' });
  assert.equal(session.document.hash, copyHash); assert.ok(copied.clipboard);
  await step('G06-clipboard', () => session.editSelection('slide-7', { op: 'paste', id_prefix: 'synthetic-copy', dx: 0, dy: 240 }, { clipboard: copied.clipboard }), undefined, false);
  await step('G08-group', () => session.editSelection('slide-7', { op: 'group', ids, group_id: 'selection-group' }));
  await step('G08-ungroup', () => session.editSelection('slide-7', { op: 'ungroup', ids: ['selection-group'] }), undefined, false);
  await step('G10-curve-control', async () => {
    const element = elementAt(session, 5, 'curve'); const path = structuredClone(element.visual.path);
    path.commands[1].control1 = [0.7, 0];
    await putElement(session, 5, 'curve', await client.editVector({ element, path }));
  });
  await step('G11-opacity', () => {
    const element = elementAt(session, 3, 'shadow'); element.visual.opacity = 0.6;
    return putElement(session, 3, 'shadow', element);
  });
  await step('G12-image-pixels', async () => {
    const element = elementAt(session, 4, 'png');
    const image = await client.editImage({ base64: element.base64, mime_type: element.mime_type, params: { grayscale: true, brightness: 0.05, resize_longest_side: 480 } });
    await session.applyImageEdit('slide-5', { id: 'png', image });
  });
  await step('G13-cell-edit', () => session.editTable('slide-3', { id: 'table', operations: [{ op: 'set_cell_text', row: 1, column: 2, text: '確認済' }] }));
  await step('G13-split-merge', () => session.editTable('slide-3', { id: 'table', operations: [{ op: 'split', row: 0, column: 0 }] }), undefined, false);
  await step('G18-svg-mask-transform', () => {
    const element = elementAt(session, 4, 'svg'); element.visual = { picture_mask: 'ellipse', rotation: 8, flip_h: true, opacity: 0.9 };
    return putElement(session, 4, 'svg', element);
  });
  await step('G23-master-topology', () => {
    const design = session.document.deck.design;
    design.masters.push({ id: 'synthetic-master', name: 'Synthetic alternate', background: '@lt1', elements: [caption('master-mark', 'SYNTHETIC MASTER', 72, 594, 900)] });
    design.layouts.push({ id: 'synthetic-layout', name: 'Synthetic blank', master_id: 'synthetic-master', background: null, elements: [] });
    return session.updateDesign(design);
  });
  await step('G23-layout', () => session.assignLayout('slide-8', 'synthetic-layout', { preserveFreeform: true }));
  await step('G25-fields', () => session.refreshFields('2026-09-18'), () => assert.equal(elementAt(session, 8, 'fields').text, 'Page 9 | 9/18/2026'));
  await step('G27-new-notes', () => session.updateNotes('slide-12', 'Synthetic newly added note. 架空の発表者ノート。'));
  await step('G40-reply', () => session.replyComment('slide-10', 'review-1', { id: 'review-2', author: 'Fictional reviewer B', initials: 'FB', timestamp: '2026-09-17T10:05:00Z', text: 'Synthetic reply / 架空の返信' }));
  await step('G40-resolve-local', () => session.resolveComment('slide-10', 'review-1', true), () => assert.equal(session.document.deck.slides[9].review.comments[0].resolved, true));
  await step('G43-alt-text', () => session.setAccessibility('slide-5', 'svg', { title: 'Synthetic bars', description: 'Fictional geometric illustration, not a statistical observation.', decorative: false }));
  const order = session.document.deck.slides[4].elements.map((element) => element.id);
  [order[1], order[2]] = [order[2], order[1]];
  await step('G43-reading-order', () => session.setReadingOrder('slide-5', order));
  const measured = await measure(session);
  const exported = await session.exportPresentation();
  const reopened = (await client.openPresentation(`features-verified-${Date.now()}`, exported.base64)).session;
  assert.deepEqual(counters(reopened.document.deck), counters(session.document.deck));
  assert.equal(elementAt(reopened, 0, 'rich').format.paragraphs[0].runs[0].style.highlight, 'CDEDE7');
  assert.equal(elementAt(reopened, 2, 'table').rows[1][2], '確認済');
  assert.equal(elementAt(reopened, 4, 'svg').svg, elementAt(session, 4, 'svg').svg);
  assert.deepEqual(elementAt(reopened, 5, 'curve').visual.path, elementAt(session, 5, 'curve').visual.path);
  assert.equal(reopened.document.deck.slides[9].review.comments[0].resolved, true);
  assert.equal(reopened.document.deck.slides[9].review.comments[1].parent_id, 'review-1');
  assert.equal(reopened.document.deck.design.masters.length, session.document.deck.design.masters.length);
  assert.equal(elementAt(reopened, 8, 'fields').text, 'Page 9 | 9/18/2026');
  if (font?.status === 'available') {
    assert.equal(reopened.document.deck.embedded_fonts[0].license_acknowledged, false);
    await reopened.setFontUsage(font.info.sha256, true);
  }
  const orderBaseline = await reopened.exportPresentation();
  await reopened.editElements('slide-7', [{ op: 'order', id: 'selection-group', index: 0 }]);
  assert.equal(reopened.document.deck.slides[6].elements[0].id, 'selection-group');
  const ordered = await reopened.exportPresentation();
  const orderedSession = (await client.openPresentation('ordered-group-check', ordered.base64)).session;
  assert.equal(orderedSession.document.deck.slides[6].elements[0].id, 'selection-group');
  await reopened.undo();
  assert.equal((await reopened.exportPresentation()).base64, orderBaseline.base64);
  checks['G09-group-order'] = { applied: true, nativeReopen: true, undoPptxExact: true, limitation: 'Newly reparented groups must be exported and reopened before crossing unrelated native intervals.' };
  checks.featureReopen = true;
  return { checks, measured };
}

async function measure(session) {
  const result = await client.request({ op: 'measure_layout', deck: session.document.deck });
  assert.deepEqual(result.issues.filter((issue) => issue.severity === 'error'), [], 'Core text measurement errors');
  assert.ok(result.measurements.length > 0, 'Expected actual core measurements');
  assert.deepEqual(result.measurements.filter((item) => item.overflow || item.missing_glyphs > 0), [], 'All measured text must fit with locally resolved glyphs');
  return result;
}

async function chartDeck(design, capabilities, smoke) {
  const available = capabilities.charts.filter((entry) => entry.create === true);
  assert.ok(available.length > 0 && available.length <= 32, 'Expected 1..32 creatable chart kinds; split a future larger catalog before exporting');
  const entries = smoke ? available.filter((entry) => ['combo', 'bubble'].includes(entry.id)) : available;
  const slides = [];
  for (const entry of entries) {
    const chart = await client.createObject({ id: `chart-${entry.id}`, kind: 'chart', preset: entry.id });
    assert.equal(chart.type, 'chart');
    assert.equal(chart.kind, entry.id, 'Factory must create the advertised native kind');
    for (const series of chart.series) assert.ok(series.values.every(Number.isFinite));
    chart.x = 90; chart.y = 166; chart.width = 1100; chart.height = 444;
    chart.options = { ...chart.options, legend: 'right', ...(entry.data_labels ? { data_labels: { show_value: true, position: 'center' } } : {}) };
    if (entry.id === 'combo') {
      chart.series[1].kind = 'line'; chart.series[1].axis = 'secondary';
      chart.options.secondary_axis = { min: 0, max: 100, major_unit: 20, number_format: '0' };
      if (entry.trendlines) chart.series[1].trendline = { kind: 'linear', display_r_squared: true };
      if (entry.error_bars) chart.series[0].error_bars = { kind: 'fixed_value', value: 1 };
    }
    slides.push(makeSlide(slides.length + 1, `Synthetic ${entry.id.replaceAll('_', ' ')} / 架空値`, [chart], ` Native kind: ${entry.id}. All factory numbers are synthetic, not observed values.`));
  }
  return { version: 1, title: `${synthetic} Native charts`, width: 1280, height: 720, design: structuredClone(design), slides };
}

async function qualifyCharts(session, name) {
  const operations = session.document.deck.slides.map((slide, index) => {
    const element = slide.elements.findIndex((item) => item.type === 'chart');
    const chart = slide.elements[element];
    const suffix = chart.kind === 'histogram' ? '/options/histogram/samples/0' : chart.kind === 'box_whisker' ? '/options/box_whisker/samples/0/0' : '/series/0/values/0';
    const original = chart.kind === 'histogram' ? chart.options.histogram.samples[0] : chart.kind === 'box_whisker' ? chart.options.box_whisker.samples[0][0] : chart.series[0].values[0];
    return { op: 'replace', path: `/deck/slides/${index}/elements/${element}${suffix}`, value: original + 1 };
  });
  const baseline = await session.exportPresentation();
  const originalHash = session.document.hash;
  await session.transact(operations, { expectedRevision: session.revision });
  const measured = await measure(session);
  const edited = await session.exportPresentation();
  const reopened = (await client.openPresentation(`${name}-data-edit`, edited.base64)).session;
  for (const [index, slide] of session.document.deck.slides.entries()) {
    const expected = slide.elements.find((item) => item.type === 'chart');
    const actual = reopened.document.deck.slides[index].elements.find((item) => item.type === 'chart');
    assert.equal(actual.kind, expected.kind);
    assert.deepEqual(actual.series, expected.series);
    assert.deepEqual(actual.options, expected.options);
  }
  await session.undo();
  assert.equal(session.document.hash, originalHash);
  assert.equal((await session.exportPresentation()).base64, baseline.base64);
  await session.redo();
  assert.equal((await session.exportPresentation()).base64, edited.base64);
  return { checks: { allChartDataEdited: true, allChartSeriesOptionsReopened: true, dataEditUndoExact: true }, measured };
}

async function saveDeck(directory, manifest, name, session, edit, font) {
  assert.ok(session.document.deck.slides.length <= 32);
  const initialMeasure = await measure(session);
  const checks = await lifecycle(session, name);
  const baseline = await session.exportPresentation();
  const baselineInfo = await publish(directory, `${name}-baseline.pptx`, Buffer.from(baseline.base64, 'base64'));
  manifest.files.push({ ...baselineInfo, ...counters(session.document.deck), checks: { ...checks, measured: true } });
  manifest.sourceDeckBaseline.push({ filename: baselineInfo.filename, sha256: baselineInfo.sha256, documentHash: session.document.hash });
  if (edit) {
    session = (await client.openPresentation(`${name}-native-edit-${Date.now()}`, baseline.base64)).session;
    if (font?.status === 'available') {
      assert.equal(session.document.deck.embedded_fonts[0].license_acknowledged, false);
      const inventory = await client.inspectPptxFonts(baseline.base64);
      assert.equal(inventory.fonts[0].info.sha256, font.info.sha256);
      await session.setFontUsage(font.info.sha256, true);
      checks.localOflReacknowledgedForMeasurement = true;
      checks.reopenFontConsentReset = true;
    }
  }
  const edited = edit ? await edit(session, name, font) : { checks: {}, measured: initialMeasure };
  const exported = await session.exportPresentation();
  const info = await publish(directory, `${name}.pptx`, Buffer.from(exported.base64, 'base64'));
  manifest.files.push({ ...info, ...counters(session.document.deck), checks: { ...checks, ...edited.checks, measured: true } });
  await publish(directory, `${name}-layout.json`, JSON.stringify(edited.measured, null, 2));
  await publish(directory, `${name}-input.json`, JSON.stringify(session.document, null, 2));
  return session;
}

async function portraitDeck(design) {
  const slides = Array.from({ length: 3 }, (_, index) => ({
    id: `portrait-${index + 1}`, title: `Synthetic page ${index + 1}`, background: '@lt1', notes: 'Synthetic portrait and aspect-ratio qualification.',
    elements: [
      { ...text('title', `架空のページ ${index + 1}`, 70), width: 810, height: 100, font_size: 36, bold: true },
      { ...text('body', ['計画 / Plan', '評価 / Evaluate', '出力 / Export'][index], 250), width: 810, height: 130 },
      { ...rectangle('swatch', 72, 420, 810, 110, `@accent${index + 1}`), preset: 'rect' },
      { ...text('label', synthetic, 638), width: 810, height: 44, font_size: 18 },
    ],
  }));
  const session = await client.createDocument({ id: `editing-portrait-${Date.now()}`, deck: { version: 1, title: `${synthetic} Page formats`, width: 1280, height: 720, design: structuredClone(design), slides } });
  await session.resizeCanvas({ width: 960, height: 720, mode: 'scale' });
  return session;
}

async function qualifyPortrait(session) {
  const originalHash = session.document.hash;
  const baseline = await session.exportPresentation();
  await session.resizeCanvas({ width: 720, height: 960, mode: 'scale' }, { expectedRevision: session.revision });
  const measured = await measure(session);
  const edited = await session.exportPresentation();
  const opened = (await client.openPresentation('portrait-dimensions', edited.base64)).session;
  assert.equal(opened.document.deck.width, 720); assert.equal(opened.document.deck.height, 960);
  await session.undo(); assert.equal(session.document.hash, originalHash);
  assert.equal((await session.exportPresentation()).base64, baseline.base64);
  await session.redo(); assert.equal((await session.exportPresentation()).base64, edited.base64);
  return { measured, checks: { canvasScale: true, portraitReopen: true, canvasUndoExact: true } };
}

async function additionalOutputs(directory, manifest, session) {
  console.log('EDITING_CHECK=templates-static-recovery-inspection');
  for (const kind of ['potx', 'thmx']) {
    const exported = await session.exportTemplate(kind);
    const reopened = await client.importTemplate(`synthetic-${kind}-${Date.now()}`, { kind, base64: exported.base64 });
    assert.equal(reopened.revision, 0);
    assert.notEqual(reopened.document.id, session.document.id);
    assert.ok(!reopened.canUndo);
    const count = counters(reopened.document.deck);
    if (kind === 'potx') assert.equal(count.slides, session.document.deck.slides.length);
    const info = await publish(directory, `editing-template.${kind}`, Buffer.from(exported.base64, 'base64'));
    manifest.files.push({ ...info, ...count, counterBasis: 'New document returned by public importTemplate; THMX itself has no presentation slide list', officeExpected: null, checks: { newTemplateDocument: true, revisionZero: true } });
  }
  const current = session.document;
  const verified = await client.verifyRecovery(current);
  assert.equal(verified.hash, current.hash);
  assert.ok(Object.isFrozen(verified));
  const recovered = await client.recoverPresentation(verified);
  assert.equal(recovered.document.hash, current.hash); assert.equal(recovered.canUndo, false);
  assert.equal((await recovered.exportPresentation()).base64, (await session.exportPresentation()).base64);
  manifest.recovery = { verifiedHash: verified.hash, freshHistory: true, exportedPptxExact: true };
  const inspection = await session.inspectDocument();
  const accessibility = await session.checkAccessibility();
  assert.equal(inspection.complete_personal_data_detection, false); assert.equal(accessibility.wcag_certified, false);
  await publish(directory, 'inspection.json', JSON.stringify(inspection, null, 2));
  await publish(directory, 'accessibility.json', JSON.stringify(accessibility, null, 2));
  await assert.rejects(session.exportCleanCopy({ new_document_id: `unconfirmed-${Date.now()}`, categories: ['notes', 'comments'], confirmed: false }), /confirm/i);
  const clean = await session.exportCleanCopy({ new_document_id: `synthetic-clean-${Date.now()}`, categories: ['notes', 'comments'], confirmed: true });
  assert.equal(session.document.hash, current.hash);
  assert.ok(clean.document.deck.slides.every((slide) => !slide.notes && !(slide.review?.comments?.length)));
  const reopened = (await client.openPresentation('synthetic-clean-reopen', clean.base64)).session;
  assert.ok(reopened.document.deck.slides.every((slide) => !slide.notes && !(slide.review?.comments?.length)));
  manifest.files.push({ ...await publish(directory, 'editing-clean.pptx', Buffer.from(clean.base64, 'base64')), ...counters(reopened.document.deck), officeExpected: null, checks: { selectedSyntheticNotesCommentsRemoved: true, sourceUnchanged: true, cleanReopen: true, notGeneralPrivacyCertification: true } });
  for (const format of ['png', 'jpeg', 'pdf']) {
    const exported = await session.exportStatic({ format, page_indices: [4], scale: 0.75, max_output_bytes: 2 * 1024 * 1024 });
    assert.equal(exported.office_parity_verified, false);
    assert.ok(exported.files.length > 0);
    for (const file of exported.files) {
      const bytes = Buffer.from(file.base64, 'base64');
      assert.equal(bytes.length, file.byte_length);
      if (format === 'pdf') assert.equal(bytes.subarray(0, 5).toString(), '%PDF-');
      if (format === 'png') assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
      if (format === 'jpeg') assert.equal(bytes.subarray(0, 2).toString('hex'), 'ffd8');
      manifest.staticExports ??= [];
      manifest.staticExports.push({ ...await publish(directory, `editing-static-${file.filename}`, bytes), format, pages: file.page_indices, width: file.width, height: file.height, warnings: exported.warnings, officeParityVerified: false, pdfTextOutlined: exported.pdf_text_outlined, pdfTagged: exported.pdf_tagged });
    }
  }
  assert.equal(session.document.hash, current.hash, 'Read-only exports must not mutate the source');
}

function coverage(manifest) {
  const features = manifest.files.find((file) => file.filename === 'editing-features.pptx').checks;
  const descriptions = {
    G01: 'Mixed Japanese/English runs, bold/color, superscript/subscript, highlights; native range edit',
    G02: 'Bullets, numbering, alignment, indentation, line/paragraph spacing; native paragraph edit',
    G04: 'Exact text search and replacement; no spellcheck/translation qualification',
    G05: 'SDK multi-selection translation; GUI marquee/drag not exercised',
    G06: 'Typed local copy/paste, source formatting and Undo; not OS/PowerPoint clipboard interoperability',
    G07: 'SDK align/distribute; no GUI guide or ruler qualification',
    G08: 'Native grouping/ungrouping and Undo',
    G09: 'Reopened group z-order; export/reopen required after fresh native reparenting',
    G10: 'Bezier control-point editing, preset adjustment and transform; no boolean-shape operations',
    G11: 'Gradient, alpha, shadow, glow, soft edge, reflection; native opacity edit',
    G12: 'Synthetic PNG grayscale/brightness/resampling; no semantic background removal',
    G13: 'Merged cells, relative widths, absolute row heights, styles; cell edit and split/Undo',
    G14: 'All currently advertised creatable chart factories (smoke uses combo/bubble only)',
    G15: 'Native data labels/right legend, combo secondary axis, trendline/error bars; series data edit',
    G18: 'Retained safe synthetic SVG and PNG fallback, native mask/transform edit',
    G22: 'Three-page 4:3 baseline resized to portrait; native dimensions and Undo',
    G23: 'Master/layout addition after native reopen; one shared theme, not arbitrary foreign themes',
    G24: 'Native POTX/THMX exports and new-document imports; not arbitrary enterprise template fidelity',
    G25: 'Native slidenum/datetime1 fields and explicit reference-date refresh',
    G27: 'SDK existing-note edit and previously blank-note creation',
    G28: 'At most one local OFL Noto Sans Latin fixture; consent rejection/reset/reacknowledgement; no Office font recognition claim',
    G29: 'Generated native roundtrips only; no arbitrary imported corpus/Office parity claim',
    G35: 'Actual core PDF/PNG/JPEG of feature page 5 only; other pages and printing are not exercised by this demo',
    G37: 'Public recovery verification, fresh history and exact recovered PPTX; no GUI/crash qualification',
    G38: 'Bounded artifacts, documents <=32 slides and reported core budgets; not stress/large-deck qualification',
    G40: 'Synthetic local comment, reply, resolved state; not modern PowerPoint collaboration threads',
    G41: 'Read-only inspection and confirmed synthetic-only clean copy; not complete privacy detection',
    G43: 'Alt text, reading order (also z-order), checker; not WCAG certification',
  };
  return Object.entries(descriptions).map(([id, subset]) => ({ id, status: id === 'G28' && manifest.font.status !== 'available' ? 'skipped' : 'verified-subset', subset, evidence: Object.keys(features).filter((key) => key.startsWith(id)), scope: 'SDK/core only; Office/schema/GUI not executed' }));
}

async function lifecycle(session, name) {
  const original = session.document;
  const first = original.deck.slides[0];
  const before = await session.exportPresentation();
  await session.updateNotes(first.id, `${first.notes}\nSynthetic SDK edit.`, { expectedRevision: session.revision });
  assert.notEqual(session.document.hash, original.hash);
  await assert.rejects(session.updateNotes(first.id, 'Stale synthetic edit', { expectedRevision: original.revision }), /revision/i);
  await session.undo();
  assert.equal(session.document.hash, original.hash);
  assert.equal((await session.exportPresentation()).base64, before.base64);
  await session.redo();
  assert.match(session.document.deck.slides[0].notes, /Synthetic SDK edit/);
  await session.undo();
  const opened = await client.openPresentation(`${name}-reopen`, before.base64);
  assert.equal((await opened.session.exportPresentation()).base64, before.base64);
  assert.deepEqual(counters(opened.session.document.deck), counters(original.deck));
  return { sessionEdit: true, staleRevisionRejected: true, undoHashExact: true, undoPptxExact: true, redo: true, reopenNoopExact: true, reopenCounters: true, warnings: opened.warnings };
}

async function main() {
  const options = parseOptions(process.argv.slice(2));
  assert.ok((await lstat(dirname(options.directory))).isDirectory(), 'Output parent must already exist');
  await mkdir(options.directory);
  ownedDirectory = options.directory;
  console.log(`EDITING_OUTPUT=${options.directory}`);
  const capabilities = await client.authoringCapabilities();
  const catalog = await client.objectCatalog();
  const manifest = { sources: 'synthetic', status: 'qualification', mode: options.smoke ? 'smoke' : 'full', actualCapabilities: capabilities, objectCatalog: catalog, sourceDeckBaseline: [], files: [], officeParityVerified: false, schemaVerified: false, generatorSha256: sha256(await readFile(fileURLToPath(import.meta.url))), budgets: { slidesPerDeck: 32, outputFiles: 96, outputBytes: 64 * 1024 * 1024, fontFiles: 1, fontBytes: 1024 * 1024, requests: 1200 } };
  const design = await client.designDefaults();
  const font = await fontFixture(options, options.directory);
  manifest.font = { ...font }; delete manifest.font.base64;
  const session = await featureDeck(design, font, options);
  const editedFeatures = await saveDeck(options.directory, manifest, 'editing-features', session, qualifyFeatures, font);
  const charts = await client.createDocument({ id: `editing-charts-${Date.now()}`, deck: await chartDeck(design, capabilities, options.smoke) });
  await saveDeck(options.directory, manifest, 'editing-charts', charts, qualifyCharts);
  await saveDeck(options.directory, manifest, 'editing-portrait', await portraitDeck(design), qualifyPortrait);
  await additionalOutputs(options.directory, manifest, editedFeatures);
  manifest.staticExportScope = { pageIndices: [4], wholeFeatureDeck: 'Not exercised by this demo. The renderer supports modeled effects with explicit approximation/rasterization warnings; unsupported details reject.' };
  manifest.chartSelection = { created: charts.document.deck.slides.map((slide) => slide.elements.find((element) => element.type === 'chart').kind), notCreated: capabilities.charts.filter((entry) => !entry.create), quantitativeReadOnly: { capabilityDeclarationsOnly: true, reason: 'No recognized public statistical input contract is assumed. New creatable native kinds use the actual factory automatically.' } };
  manifest.coverage = coverage(manifest);
  assert.equal(manifest.coverage.length, 28);
  manifest.optionalPartMetadata = { enabled: options.withParts, count: editedFeatures.document.parts?.length ?? 0 };
  manifest.handoff = manifest.files.filter((file) => ['editing-features.pptx', 'editing-charts.pptx', 'editing-portrait.pptx'].includes(file.filename)).map((file) => ({
    filename: file.filename, sha256: file.sha256,
    office: { command: 'pwsh', args: ['-NoProfile', '-File', join(root, 'tools/verify-powerpoint.ps1'), '-Path', join(options.directory, file.filename), '-ExpectedSlides', String(file.slides), '-ExpectedCharts', String(file.charts), '-ExpectedPictures', String(file.pictures - file.graphics), '-ExpectedGraphics', String(file.graphics), '-ExpectedGroups', String(file.groups), '-ExpectedConnectors', String(file.connectors), '-MinimumTables', String(file.tables), ...(file.charts ? ['-VerifyChartData', '-FirstNumericChartCell'] : [])] },
    schema: { command: 'pwsh', args: ['-NoProfile', '-File', join(root, 'tools/validate-openxml.ps1'), '-Path', join(options.directory, file.filename)] },
    caveats: ['Commands recorded, NOT executed; run qualification separately.', 'Office captures retain native page aspect ratio with a 1280px longest side.', 'Histograms have two known SDK 3.5.1 schema errors per chart resource; report these explicitly, not as a schema pass.', 'Schema helper may download packages if its local cache is absent; use the approved local cache for offline validation.'],
  }));
  await publish(options.directory, 'README.md', '\uFEFF# Synthetic Editing Qualification\n\nEvery label, number, note, reviewer and image is fictional. No source presentation was read or overwritten.\n\nRun `node tools/editing-demo.mjs [new-directory] [--smoke] [--skip-font] [--with-parts]`. The parent directory must exist; the leaf must not. Default: `.artifacts/editing-<Date.now()>`. A new temporary directory outside OneDrive is allowed for these new synthetic artifacts only; never move or replace existing protected files. Every artifact uses exclusive creation.\n\nFull mode: 12 feature slides, catalog-driven native chart slides (currently 24), and 3 portrait slides, in separate PPTX files. Smoke reduces charts to combo/bubble. Baseline PPTX files precede scripted native edits. Input/layout JSON records evidence, not a replacement presentation engine.\n\nSee manifest.json for all 28 supported subsets, hashes, actual SDK document counters, native edits/Undo/reopen, font evidence, static renderer warnings and separately run Office/schema commands. THMX counts describe its imported new document, not native THMX slides. The clean copy deliberately has no notes/comments and is not suitable for the Office helper\'s notes-per-slide assumption.\n\nThe optional font is only the local cosmic-text-0.19.0 NotoSans-Regular.ttf OFL fixture, at most 1 MiB. No downloads or global font registration. Reopen resets usage consent; reacknowledgement is confined to this explicitly approved matching fixture. Missing fixture or --skip-font means embedding is unqualified. Japanese text is measured with actual local fallback fonts. --with-parts adds one optional managed flow group.\n\nNewly reparented native groups require export/reopen before crossing unrelated z-order intervals. Comments use local AISlide reply/resolved extensions, not modern PowerPoint threads. Fields cover only slidenum/datetime1 with an explicit date.\n\nNo Office, schema, GUI, printing, arbitrary imported-deck fidelity, complete privacy detection, WCAG certification or Office font recognition is claimed. Static exports are core renderings, not Office screenshots. Run the listed Office/schema commands separately. Office captures preserve the page aspect ratio with a 1280px longest side. Histogram bins use the documented Office-compatible encoding with two known SDK 3.5.1 schema errors per chart resource.\n');
  for (const file of published) {
    const bytes = await readFile(join(options.directory, file.filename));
    assert.equal(sha256(bytes), file.sha256, `Artifact changed during qualification: ${file.filename}`);
  }
  manifest.calls = calls;
  manifest.artifacts = [...published];
  manifest.status = 'passed-supported-subsets';
  await publish(options.directory, 'manifest.json', JSON.stringify(manifest, null, 2));
  console.log(JSON.stringify({ directory: options.directory, status: manifest.status, font: manifest.font.status, files: manifest.files.map(({ filename, sha256: hash, bytes, slides, charts, tables, pictures, groups, notes }) => ({ filename, sha256: hash, bytes, slides, charts, tables, pictures, groups, notes })) }, null, 2));
}

try { await main(); }
catch (error) {
  if (ownedDirectory) {
    await publish(ownedDirectory, 'failure.json', JSON.stringify({ status: 'failed', sources: 'synthetic', error: error.message, calls, partialArtifacts: [...published], officeParityVerified: false }, null, 2)).catch(() => {});
  }
  console.error(`EDITING_FAILED: ${error.stack ?? error}`);
  process.exitCode = 1;
}
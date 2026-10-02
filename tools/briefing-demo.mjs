import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const { values, positionals } = parseArgs({ options: { 'font-family': { type: 'string' }, editorial: { type: 'boolean', default: false } }, allowPositionals: true });
assert.ok(positionals.length <= 1, 'Usage: node tools/briefing-demo.mjs [new-output-directory] [--font-family installed-font] [--editorial]');
const directory = resolve(positionals[0] ?? `.artifacts/briefing-demo-${Date.now()}`);
await mkdir(directory, { recursive: true });
assert.equal((await readdir(directory)).length, 0, 'Use a new or empty output directory');
const font = values['font-family'] ?? 'Yu Gothic UI';
const theme = { name: 'Technical briefing', colors: { dk1: '1B2530', lt1: 'FFFFFF', dk2: '44546A', lt2: 'EEF3F8', accent1: '0F6CBD', accent2: '0B7A6E', accent3: 'C4314B', accent4: 'B98900', accent5: '4B58B5', accent6: '2E7D32', hlink: '0F6CBD', folHlink: '6B4C9A' }, fonts: { major: font, minor: font, east_asian: font, complex_script: font } };
const footer = '合成データによる表示例（内容の事実確認は行っていません）';
const body = { x: 48, y: 120, width: 1184, height: 540, show_title: false };
const environment = process.env.AISLIDE_CORE_BINARY ? { AISLIDE_CORE_BINARY: process.env.AISLIDE_CORE_BINARY } : {};
const transport = new StdioClientTransport({ command: process.execPath, args: [resolve('tools/mcp.mjs'), '--output-dir', directory, '--asset-dir', directory], stderr: 'pipe', env: environment });
const client = new Client({ name: 'briefing-example', version: '1.0.0' });
const calls = [];
const invoke = async (name, arguments_ = {}) => {
  const result = await client.callTool({ name, arguments: arguments_ }, undefined, { timeout: 320000, maxTotalTimeout: 320000 });
  assert.ok(!result.isError, `${name}: ${JSON.stringify(result.content)}`);
  calls.push(name);
  return result;
};
const call = async (name, arguments_) => JSON.parse((await invoke(name, arguments_)).content[0].text);
const text = (id, value, x, y, width, height, font_size, color, bold = false, alignment = 'left') => ({ type: 'text', id, text: value, x, y, width, height, font_size, color, bold, format: { font_family: '@minor', alignment } });

const iconRequests = [
  ['FileText', '0F6CBD'], ['Target', '0F6CBD'], ['Zap', '0F6CBD'], ['Users', '0F6CBD'], ['Flag', '0F6CBD'], ['CircleCheck', '0F6CBD'], ['Rocket', 'FFFFFF'],
  ['Layers', '0F6CBD'], ['RefreshCw', '0B7A6E'], ['Bot', '1B2530'], ['Search', 'C4314B'], ['Zap', 'C4314B'], ['Bot', 'C4314B'],
  ['Database', '0B7A6E'], ['Workflow', '0B7A6E'], ['Network', '0B7A6E'], ['Search', '0F6CBD'], ['Users', '4B58B5'],
];

async function editorialSlides(icon) {
  const slides = [];
  const pair = (title, symbol, before, after) => {
    for (const [variant, entry] of [['従来', before], ['新パーツ', after]]) slides.push({ title: `${title} / ${variant}`, section: `${String(slides.length + 1).padStart(2, '0')} 同じ内容による比較`, icon: symbol, ...entry });
  };
  const steps = [
    { label: '証拠を集める', detail: '承認済みのデータ源から、必要な情報を同じ基準で集める。' },
    { label: '優先度を決める', detail: '影響と前提を確認し、次に取り組む対象を決める。' },
    { label: '結果を確かめる', detail: '実行後の変化を記録し、次の判断へ戻す。' },
  ];
  pair('順序を一本の軸で表す', 'Flag', { preset: 'flow/cards', spec: { kind: 'step_cards', steps } }, { preset: 'flow/open-steps', spec: { kind: 'open_steps', steps, accent: '@accent2' } });
  pair('長い説明を縦の軸に揃える', 'Users', { preset: 'list/icon-rows', spec: { kind: 'icon_rows', rows: steps.map((step, index) => ({ ...step, label: `${index + 1}. ${step.label}` })) } }, { preset: 'vertical-flow/rail', spec: { kind: 'rail_steps', steps, accent: '@accent2' } });
  const phases = [
    { period: '今すぐ', label: '現状を測る', detail: '優先すべき差分から着手する。', points: ['対象と責任者を確認する', '保護の空白を記録する'], outcome: '共通の基準をつくる' },
    { period: '次の四半期', label: '能力を広げる', detail: '共通の制御を別の部門にも広げる。', points: ['制御を段階的に展開する', '復旧手順を演習する'], outcome: '繰り返せる運用にする' },
    { period: 'その後', label: '改善を定着させる', detail: '経営の判断と現場の記録をつなぐ。', points: ['根拠を定期的に報告する', '優先度を見直す'], outcome: '継続的な改善を回す' },
  ];
  pair('時期・施策・到達点を対応させる', 'Flag', { preset: 'flow/cards', spec: { kind: 'step_cards', step_label: 'PHASE', steps: phases.map(({ period, label, ...rest }) => ({ label: `${period}：${label}`, ...rest })) } }, { preset: 'flow/roadmap', spec: { kind: 'roadmap', phases, accent: '@accent2' } });
  const peers = [
    { label: '露出の把握', detail: '資産と保護の空白を継続して確認する。', icon: icon('Search', '0F6CBD') },
    { label: 'ID の保護', detail: 'アクセスの前提と権限を同じ境界で管理する。', icon: icon('Target', '0F6CBD') },
    { label: '復旧の準備', detail: '止まった後に戻せることを演習で確かめる。', icon: icon('RefreshCw', '0B7A6E') },
  ];
  pair('同格の概念を余白で区切る', 'Target', { preset: 'list-horizontal/icon-cards', spec: { kind: 'icon_cards', cards: peers } }, { preset: 'list-horizontal/icon-columns', spec: { kind: 'icon_columns', items: peers } });
  const facts = [
    { value: '24', unit: 'チーム', label: '確認対象', detail: '今回の例に含めた対象の数。', qualifier: '架空の対象数。実績値ではありません。' },
    { value: '85', unit: '%', label: '確認の完了率', detail: '別に定義した確認項目の進捗。', qualifier: '架空の率。対象数とは母数が異なります。' },
    { value: '12', unit: '日', label: '確認に要した期間', detail: '独立して与えた所要期間。', qualifier: '架空の期間。他の数値と量で比較しません。' },
  ];
  pair('数字と条件を先に読ませる', 'FileText', { preset: 'list-horizontal/icon-cards', spec: { kind: 'icon_cards', cards: facts.map(item => ({ label: `${item.value} ${item.unit}：${item.label}`, detail: item.detail, caption: item.qualifier })) } }, { preset: 'list-horizontal/fact-columns', spec: { kind: 'fact_columns', items: facts, accent: '@accent2' } });
  const { deck_id: sourceId } = await call('create_presentation', { title: 'Synthetic source figures', setup: { theme } });
  let source = await call('get_deck_summary', { deck_id: sourceId });
  await call('edit_slides', { deck_id: sourceId, expected_revision: source.revision, operations: [{ op: 'insert', id: 'slide-2', after: 'slide-1', title: 'Evidence B' }, { op: 'insert', id: 'slide-3', after: 'slide-2', title: 'Evidence C' }] });
  await call('get_tool_schema', { name: 'apply_operations' });
  source = await call('get_deck_summary', { deck_id: sourceId });
  const box = (id, value, x, y, width, height) => ({ type: 'shape', id, x, y, width, height, preset: 'rect', fill: 'E5F2F0', stroke: '@accent2', stroke_width: 3, text: value, font_size: 64, color: '@dk1', bold: true, format: { font_family: '@minor', alignment: 'center', vertical: 'middle', padding: { left: 20, top: 20, right: 20, bottom: 20 } } });
  const arrow = (id, x, y, width, height) => ({ type: 'connector', id, x, y, width, height, color: '@accent2', stroke_width: 6, arrow: true });
  const sourceScenes = [
    [text('source-a-title', '確認の流れ（架空例）', 60, 40, 1160, 80, 44, '@dk1', true),
      ...['把握', '判断', '対応'].map((value, index) => box(`source-a-${index}`, value, 60 + 430 * index, 260, 300, 180)),
      arrow('source-a-edge-1', 375, 350, 100, 1), arrow('source-a-edge-2', 805, 350, 100, 1)],
    [text('source-b-title', '評価の手順（架空例）', 60, 40, 300, 140, 44, '@dk1', true),
      ...['計測', '評価', '改善'].map((value, index) => box(`source-b-${index}`, value, 400, 100 + 200 * index, 500, 120)),
      arrow('source-b-edge-1', 650, 235, 1, 50), arrow('source-b-edge-2', 650, 435, 1, 50)],
    [text('source-c-title', '段階の計画（架空例）', 60, 40, 1160, 80, 44, '@dk1', true),
      ...['今', '次', '後'].map((value, index) => box(`source-c-${index}`, value, 60 + 430 * index, 260, 300, 180)),
      arrow('source-c-edge-1', 375, 350, 100, 1), arrow('source-c-edge-2', 805, 350, 100, 1),
      text('source-c-note', '段階の間隔は経過時間を表しません', 60, 520, 1160, 64, 36, '@dk2')],
  ];
  await call('apply_operations', { deck_id: sourceId, expected_revision: source.revision, expected_hash: source.hash, operations: sourceScenes.map((elements, index) => ({ op: 'add_elements', slide_id: `slide-${index + 1}`, elements })) });
  const preview = await invoke('preview_presentation', { deck_id: sourceId, options: { page_indices: [0, 1, 2], layout: 'pages', format: 'png', max_dimension: 960, max_output_bytes: 2 * 1048576, overflow: 'error' } });
  const rendered = preview.content.filter(content => content.type === 'image');
  assert.equal(rendered.length, 3);
  const evidence = [];
  for (const [index, image] of rendered.entries()) {
    const path = `source-${index + 1}.png`;
    await writeFile(join(directory, path), Buffer.from(image.data, 'base64'), { flag: 'wx' });
    const asset = await call('register_asset', { path });
    assert.ok(asset.asset_id);
    evidence.push({ image: { asset_id: asset.asset_id, mime_type: 'image/png', alt: `Synthetic evidence figure ${index + 1}` }, label: ['横の手順を確認する', '縦の手順を確認する', '段階の計画を確認する'][index], detail: '図に表れた関係を短い説明とともに提示する。', caption: '合成の説明図・内容は未測定' });
  }
  pair('説明図を全体保持して主役にする', 'FileText', { preset: 'list-horizontal/icon-cards', spec: { kind: 'icon_cards', cards: evidence.map(({ image, ...item }) => ({ ...item, icon: image })) } }, { preset: 'list-horizontal/image-columns', spec: { kind: 'image_columns', items: evidence } });
  return slides;
}

try {
  await client.connect(transport);
  const { deck_id } = await call('create_presentation', { title: 'Technical briefing example', setup: { theme } });
  await call('get_tool_schema', { name: 'lucide_icon_assets' });
  const requestedIcons = values.editorial ? iconRequests.filter(([name]) => ['FileText', 'Target', 'Users', 'Flag', 'RefreshCw', 'Search'].includes(name)) : iconRequests;
  const prepared = await call('lucide_icon_assets', { icons: requestedIcons.map(([name, color]) => ({ name, color })) });
  const icon = (name, color) => prepared.icons.find(entry => entry.name === name && entry.color === color).icon;
  const slides = values.editorial ? await editorialSlides(icon) : [
    { title: 'アジェンダ', section: '', icon: 'FileText', spec: { kind: 'agenda', items: [
      { label: '背景と方向性', detail: '運用が直面している変化と課題', meta: '10 分' },
      { label: '共通基盤の全体像', detail: 'データ、コンテキスト、制御の関係', meta: '15 分' },
      { label: '保護ループと自動化', detail: '直線型の対応からループ型の保護へ', meta: '10 分' },
      { label: '運用モデルの変化', detail: '役割、指標、人とエージェントの分担', meta: '15 分' },
      { label: '導入の進め方', detail: '3 つのステップと検討の観点', meta: '10 分' },
    ] }, preset: 'list/agenda' },
    { section: 'divider', title: '背景と方向性', number: '01', subtitle: '運用の前提が変わり、\n評価の軸も変わりつつある' },
    { title: '本日の要点', section: '00 概要', icon: 'Target', preset: 'list-horizontal/icon-cards', spec: { kind: 'icon_cards', numbered: true, message: { text: '戦略は人が担い、規模は自動化が支える。', detail: '人とエージェントの役割を明確に分けて運用する', color: 'D9F0A3' }, cards: [
      { label: '統合型の運用基盤', caption: 'Unified operations platform', detail: 'シグナル、コンテキスト、制御を一つの基盤で扱い、人とエージェントが同じ情報で判断する。', tag: '詳しくは第 2 部', icon: icon('Layers', '0F6CBD'), accent: '0F6CBD' },
      { label: '継続的な保護ループ', caption: 'Continuous protection loop', detail: '検知・予測・阻止と事後の知見を切れ目なく回し、侵害前の保護を強化する。', tag: '詳しくは第 3 部', icon: icon('RefreshCw', '0B7A6E'), accent: '0B7A6E' },
      { label: 'エージェント型の運用', caption: 'Agent-driven operating model', detail: '調査や調整はエージェントが担い、人は優先度・判断・成果の定義に集中する。', tag: '詳しくは第 4 部', icon: icon('Bot', '1B2530'), accent: '1B2530' },
    ] } },
    { title: '変化をもたらす要因', section: '01 背景と方向性', icon: 'Zap', preset: 'list/icon-rows', spec: { kind: 'icon_rows', message: { text: 'この差は構造的なもので、既存の手順を速めるだけでは埋まらない。', fill: 'FBE4E6', color: 'C4314B' }, rows: [
      { label: '露出の把握と標的化', detail: '環境の露出を自動的に洗い出し、手口を標的の環境に合わせて調整する。', icon: icon('Search', 'C4314B'), accent: 'C4314B' },
      { label: '機械の速度での反復', detail: '短時間に大量の操作を試し、遮断されても手順を変えて繰り返す。', icon: icon('Zap', 'C4314B'), accent: 'C4314B' },
      { label: '少人数での大規模な作業', detail: 'かつてはチーム全体が必要だった作業を、少人数と自動化で実行できる。', icon: icon('Bot', 'C4314B'), accent: 'C4314B' },
    ] } },
    { title: 'ロールの変化', section: '04 運用モデル', icon: 'Users', preset: 'before-after/shift', spec: { kind: 'shift_rows', from_label: 'これまで', to_label: 'これから', accent: '0B7A6E', message: { text: '専門性、戦略的な意思決定、測定可能な成果を優先する' }, rows: [
      { from: 'アナリスト', to: 'エージェント\nオーケストレーター', caption: 'Agent Orchestrator', detail: '所見を検証し、システムの提案を承認し、曖昧さを解消する。' },
      { from: '検知エンジニア', to: 'セキュリティ\nエンジニア', caption: 'Security Engineer', detail: 'ガードレールを定め、専門知識をコード化し、防御を検証する。' },
      { from: '脅威ハンター', to: '露出対策リード', caption: 'Exposure Lead', detail: '攻撃者視点で探索を指揮し、重要な経路の対策に焦点を合わせる。' },
      { from: '運用リーダー', to: 'リスクシステム\n責任者', caption: 'Risk System Owner', detail: 'システム全体の保護成果に責任を持ち、人とエージェントの協働を設計する。' },
    ] } },
    { title: '導入の 3 ステップ', section: '05 導入の進め方', icon: 'Flag', preset: 'flow/cards', spec: { kind: 'step_cards', accent: '0B7A6E', steps: [
      { label: '基盤から始める', detail: 'ID、エンドポイント、クラウド、データを共通の理解にまとめ、共有の基盤をつくる。', points: ['対象範囲とデータ源を棚卸しする', '共通の識別子と権限を揃える'], outcome: '全体を見渡せる基盤を先に整える', icon: icon('Database', '0B7A6E') },
      { label: 'ワークフローに組み込む', detail: '調査、露出の分析、優先度付けなど、定義が明確な作業から任せる。', points: ['承認が必要な操作を決める', '結果の確認手順を残す'], outcome: '自律より先に余力をつくる', icon: icon('Workflow', '0B7A6E') },
      { label: 'システムへ広げる', detail: 'エージェント同士が連携し、保護目標を追求するシステムを描く。', points: ['成果の指標で効果を測る', '自律の範囲を段階的に広げる'], outcome: '専門性と共有コンテキストを組み合わせる', icon: icon('Network', '0B7A6E') },
    ] } },
    { title: '検討の進め方', section: '05 導入の進め方', icon: 'CircleCheck', preset: 'list-horizontal/icon-cards', spec: { kind: 'icon_cards', message: { text: '提供範囲や前提条件は、検討時点の最新の公式情報で確認してください。', fill: 'FBE4E6', color: 'C4314B' }, cards: [
      { label: '1. 現状を把握する', points: ['現在のワークロードと対象範囲を棚卸しする', '利用状況とデータの取り込み範囲を確認する', '自動化の前提条件を確認する'], icon: icon('Search', '0F6CBD'), accent: '0F6CBD' },
      { label: '2. 小さく試す', points: ['検証環境で機能と運用画面を確認する', '定義が明確な作業でエージェントを試す', '承認者と人の判断を残す箇所を決める'], icon: icon('Flag', '0F6CBD'), accent: '0F6CBD' },
      { label: '3. 運用を設計し直す', points: ['ロールと責任分担を見直す', '成果の指標を決める', '自律の範囲を段階的に広げる統制を整える'], icon: icon('Users', '4B58B5'), accent: '4B58B5' },
    ] } },
  ];
  const managedCount = slides.filter(slide => slide.preset).length;
  let summary = await call('get_deck_summary', { deck_id });
  await call('edit_slides', { deck_id, expected_revision: summary.revision, operations: slides.slice(1).map((slide, index) => ({ op: 'insert', id: `slide-${index + 2}`, after: `slide-${index + 1}`, title: slide.title })) });
  await call('get_tool_schema', { name: 'apply_operations' });
  const operations = [];
  for (const [index, slide] of slides.entries()) {
    const slide_id = `slide-${index + 1}`;
    const page = text(`s${index}-page`, String(index + 1), 1184, 680, 48, 20, 11, '5B6F83', false, 'right');
    if (slide.section === 'divider') {
      operations.push({ op: 'add_elements', slide_id, elements: [
        { type: 'rect', id: `s${index}-background`, x: 0, y: 0, width: 1280, height: 720, fill: '@dk1' },
        { type: 'rect', id: `s${index}-rule`, x: 96, y: 300, width: 48, height: 3, fill: 'D9F0A3' },
        text(`s${index}-number`, slide.number, 96, 316, 400, 56, 44, 'D9F0A3', true),
        text(`s${index}-heading`, slide.title, 96, 372, 640, 56, 36, '@lt1', true),
        text(`s${index}-subtitle`, slide.subtitle, 96, 440, 640, 56, 16, 'C9D3DD'),
        { type: 'shape', id: `s${index}-disc`, x: 860, y: 240, width: 240, height: 240, preset: 'ellipse', fill: '26394B', stroke: '26394B', stroke_width: 0, text: '', font_size: 18, color: '@lt1', bold: false },
        { ...page, color: 'C9D3DD' },
      ] }, { op: 'add_picture', slide_id, id: `s${index}-symbol`, ...icon('Rocket', 'FFFFFF'), frame: { x: 920, y: 300, width: 120, height: 120 }, fit: 'contain' });
    } else {
      operations.push({ op: 'add_elements', slide_id, elements: [
        { type: 'shape', id: `s${index}-icon-background`, x: 48, y: 34, width: 48, height: 48, preset: 'ellipse', fill: 'E8F2FE', stroke: 'E8F2FE', stroke_width: 0, text: '', font_size: 18, color: '0F6CBD', bold: false },
        text(`s${index}-section`, slide.section, 108, 20, 1124, 20, 12, '0F6CBD', true),
        text(`s${index}-title`, slide.title, 108, 44, 1124, 48, 30, '@dk1', true),
        { type: 'rect', id: `s${index}-rule`, x: 48, y: 98, width: 1184, height: 1.2, fill: 'CBD7E3' },
        text(`s${index}-footer`, footer, 48, 680, 1100, 20, 11, '5B6F83'),
        page,
      ] }, { op: 'add_picture', slide_id, id: `s${index}-icon`, ...icon(slide.icon, '0F6CBD'), frame: { x: 58, y: 44, width: 28, height: 28 }, fit: 'contain' },
      { op: 'add_part', slide_id, id: `s${index}-body`, spec: { version: 1, preset: slide.preset, title: '', data: slide.spec, layout: body } });
    }
    operations.push({ op: 'update_notes', slide_id, notes: '合成データによる表示例です。内容の事実確認は行っていません。' });
  }
  summary = await call('get_deck_summary', { deck_id });
  await call('apply_operations', { deck_id, expected_revision: summary.revision, expected_hash: summary.hash, operations });
  const document = await call('get_document', { deck_id });
  assert.equal(document.deck.slides.length, slides.length);
  assert.equal(document.parts.length, managedCount);
  assert.ok(document.parts.every(part => !part.stale));
  const measured = await call('measure_layout', { deck_id });
  assert.ok(!measured.measurements.some(measurement => measurement.overflow || measurement.missing_glyphs), 'Text layout must fit with available glyphs');
  const pages = slides.map((_slide, index) => index);
  const preflight = { findings: [] };
  for (let offset = 0; offset < pages.length; offset += 8) {
    const report = await call('preflight_presentation', { deck_id, options: { page_indices: pages.slice(offset, offset + 8), min_font_size: 11 } });
    preflight.findings.push(...report.findings);
  }
  const blocking = preflight.findings.filter(finding => ['TEXT_OVERFLOW', 'TEXT_CLIPPED', 'MISSING_GLYPHS', 'IMAGE_ASPECT_DISTORTED', 'CONTAINER_PADDING', 'OFF_SLIDE', 'TEXT_OVERLAP'].includes(finding.code));
  assert.deepEqual(blocking, [], 'Briefing geometry and text must pass blocking preflight checks');
  if (values['font-family']) assert.ok(!preflight.findings.some(finding => finding.code === 'FONT_FALLBACK'), 'Use an installed font that covers all text');
  const images = [];
  for (let offset = 0; offset < pages.length; offset += 4) {
    const group = pages.slice(offset, offset + 4);
    const preview = await invoke('preview_presentation', { deck_id, options: { page_indices: group, layout: 'pages', format: 'png', max_dimension: 1280, max_output_bytes: 2 * 1048576, overflow: 'error' } });
    images.push(...preview.content.filter(content => content.type === 'image'));
  }
  assert.equal(images.length, slides.length);
  for (const [index, image] of images.entries()) await writeFile(join(directory, `slide-${String(index + 1).padStart(2, '0')}.png`), Buffer.from(image.data, 'base64'), { flag: 'wx' });
  const filename = values.editorial ? 'editorial-comparison.pptx' : 'briefing-demo.pptx';
  await call('export_pptx', { deck_id, filename });
  const bytes = await readFile(join(directory, filename));
  assert.equal(bytes.subarray(0, 2).toString(), 'PK');
  const reopened = await call('open_pptx', { base64: bytes.toString('base64') });
  const restored = await call('get_document', { deck_id: reopened.deck_id });
  assert.equal(restored.parts.length, managedCount);
  assert.ok(restored.parts.every(part => !part.stale));
  const target = restored.parts.find(part => part.spec.preset === (values.editorial ? 'flow/open-steps' : 'before-after/shift'));
  const changed = structuredClone(target.spec);
  if (values.editorial) changed.data.steps[0].detail = '承認済みの情報を揃え、根拠を確認する。';
  else changed.data.rows[0].detail = '所見を検証し、提案を承認する。';
  await call('update_part', { deck_id: reopened.deck_id, expected_revision: restored.revision, slide_id: target.slide_id, id: target.element_id, spec: changed });
  await call('undo', { deck_id: reopened.deck_id });
  assert.equal((await call('get_deck_summary', { deck_id: reopened.deck_id })).hash, restored.hash);
  await call('export_pptx', { deck_id: reopened.deck_id, filename: 'briefing-demo-undo.pptx' });
  assert.deepEqual(await readFile(join(directory, 'briefing-demo-undo.pptx')), bytes);
  const proof = { format: values.editorial ? 'aislide.editorial-example' : 'aislide.briefing-example', version: 1, page_count: slides.length, preflight_page_indices: pages, sha256: createHash('sha256').update(bytes).digest('hex'), presets: [...new Set(slides.map(slide => slide.preset).filter(Boolean))], font_family: font,
    icon_source: 'Lucide (ISC), locally installed library via lucide_icon_assets', icon_assets: prepared.icons.length, content: 'synthetic', native_reopen_verified: true, native_update_undo_verified: true, office_visual_parity: false,
    preflight_findings: preflight.findings.map(({ code, severity, page_index }) => ({ code, severity, page_index })), calls };
  await writeFile(join(directory, 'proof.json'), JSON.stringify(proof, null, 2), { flag: 'wx' });
  console.log(JSON.stringify({ directory, pptx: join(directory, filename), sha256: proof.sha256, pages: slides.length, preflight_findings: preflight.findings.length, calls: calls.length }));
} finally { await client.close(); await transport.close(); }

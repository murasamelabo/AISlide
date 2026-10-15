import { mkdir, readFile, readdir, writeFile } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { createHash, randomUUID } from 'node:crypto';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { publishNewFile } from './atomic-output.mjs';
import sharp from 'sharp';

const directory = resolve(process.argv[2] ?? `.artifacts/parts-${Date.now()}`);
await mkdir(directory, { recursive: true });
if ((await readdir(directory)).length) throw new Error('The parts demo requires a new or empty output directory');
const client = new AislideClient(requestCore);
const catalog = await client.partCatalog();
const japanese = process.argv.includes('--japanese');
const design = await client.designDefaults();
if (japanese) {
  design.theme.fonts.major = 'Yu Gothic'; design.theme.fonts.minor = 'Yu Gothic'; design.theme.fonts.east_asian = 'Yu Gothic';
  design.theme.colors.accent1 = '1976D2'; design.theme.colors.lt2 = 'EEF3F6';
  const labels = ['現状把握', '方針策定', '施策実行', '効果確認', '対象拡大', '継続改善'];
  for (const preset of catalog.presets) {
    const spec = preset.example;
    spec.title = '日本語パーツの配置検証'; spec.subtitle = '検証用の架空データ / 数値は実績ではありません';
    const data = spec.data;
    switch (data.kind) {
      case 'chart':
        data.categories = data.categories.map((value, index) => preset.category === 'scatter' ? value : `第${index + 1}四半期`);
        data.series.forEach((series, index) => { series.name = `検証系列${index + 1}`; });
        data.x_axis = preset.category === 'horizontal-bar-graph' ? '件数' : '期間'; data.y_axis = preset.category === 'horizontal-bar-graph' ? '期間' : '件数';
        break;
      case 'items':
        data.center = '共通の目的';
        data.items.forEach((item, index) => { item.label = preset.category === 'grow' ? ['全体市場', '対象市場', '獲得可能'][index] : labels[index % labels.length]; item.detail = '対象と判断基準を確認'; });
        break;
      case 'tree':
        data.nodes.forEach((node, index) => { node.label = ['全体方針', '業務設計', '運用体制', '利用体験', '共通基盤'][index % 5]; });
        break;
      case 'network':
        data.nodes.forEach((node, index) => { node.label = labels[index % labels.length]; });
        data.edges.forEach((edge, index) => { edge.label = ['情報共有', '検討依頼', '結果報告', '改善提案'][index % 4]; });
        break;
      case 'matrix':
        data.rows = data.rows.map((_, index) => ['処理速度', '運用管理', '実行負荷', '改善余地'][index]);
        data.columns = data.columns.map((_, index) => `選択肢${index + 1}`);
        data.cells = data.cells.map((row) => row.map((_, index) => index % 2 ? '条件を確認' : '対応可能'));
        break;
      case 'groups':
        data.groups.forEach((group, index) => { group.label = `対象領域${index + 1}`; group.items = group.items.map((_, index) => labels[index % labels.length]); });
        break;
      case 'timeline':
        data.periods = data.periods.map((_, index) => `期間${index + 1}`); data.tasks.forEach((task, index) => { task.label = labels[index % labels.length]; });
        break;
      case 'waterfall':
        data.unit = '検証値'; data.steps.forEach((step, index) => { step.label = step.total ? index === 0 ? '開始値' : '最終値' : `変動要因${index}`; });
        break;
      case 'map':
        data.points.forEach((point, index) => { point.label = ['東京', 'ロンドン', 'ニューヨーク'][index]; });
        break;
      default: throw new Error(`Unexpected catalog data kind: ${data.kind}`);
    }
  }
}
const evidence = { generated_at: new Date().toISOString(), preset_count: catalog.presets.length, language: japanese ? 'ja' : 'en', data: 'Synthetic examples, not factual claims', native_editable: true, office_visual_parity: false, files: [] };
let browser;
let page;
let expect;
const pageErrors = [];
try {
if (japanese) {
  const playwright = await import('@playwright/test'); expect = playwright.expect;
  browser = await playwright.chromium.launch({ channel: 'msedge', headless: true });
  page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
  page.on('pageerror', (error) => pageErrors.push(error.message));
  await page.goto('http://127.0.0.1:4174/');
  await expect(page.getByRole('button', { name: 'Open PPTX', exact: true })).toBeEnabled();
  await mkdir(join(directory, 'studio-previews'));
}
for (let offset = 0; offset < catalog.presets.length; offset += 27) {
  const batch = catalog.presets.slice(offset, offset + 27);
  const deck = { version: 1, title: `AISlide parts ${offset / 27 + 1}`, width: 1280, height: 720, ...(japanese ? { design } : {}), slides: batch.map((preset, index) => ({ id: `slide-${index + 1}`, title: `${preset.category_name} / ${preset.name}`, background: '@lt1', notes: `Synthetic example\n${preset.id}\nOriginal AISlide layout. Map outlines: Natural Earth public-domain data.`, elements: [] })) };
  const session = await client.createDocument({ id: `parts-demo-${offset}`, deck });
  for (const [index, preset] of batch.entries()) await session.addPart(`slide-${index + 1}`, { id: `part-${index + 1}`, spec: preset.example });
  const exported = await session.exportPresentation();
  const bytes = Buffer.from(exported.base64, 'base64');
  const filename = `parts-${String(offset / 27 + 1).padStart(2, '0')}.pptx`;
  await publishNewFile(join(directory, `.parts-${randomUUID()}.tmp`), join(directory, filename), bytes);
  const reopened = await client.openPresentation(`reopened-${offset}`, exported.base64);
  if (reopened.session.document.parts?.some((part) => part.stale)) throw new Error(`Stale metadata after opening ${filename}`);
  if ((await reopened.session.exportPresentation()).base64 !== exported.base64) throw new Error(`No-op bytes differ for ${filename}`);
  const count = {};
  function visit(elements) { for (const element of elements) { count[element.type] = (count[element.type] ?? 0) + 1; if (element.type === 'group') visit(element.children); } }
  for (const slide of session.document.deck.slides) visit(slide.elements);
  evidence.files.push({ filename, slides: batch.length, sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length, objects: count, presets: batch.map((preset) => preset.id), no_op_identical: true, metadata_current: true, studio_inserted_slides_checked: japanese });
  if (japanese) {
    await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: filename, mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: bytes });
    await expect(page.locator('.thumbnail')).toHaveCount(batch.length);
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  }
  const overlays = [];
  for (const [index, preset] of batch.entries()) {
    let pixels;
    if (japanese) {
      await page.getByRole('button', { name: `Slide ${index + 1}: ${preset.category_name} / ${preset.name}`, exact: true }).click();
      await expect(page.locator('.slide-stage').getByText(preset.example.title, { exact: true })).toBeVisible();
      await expect(page.locator('.slide-stage [role="status"]')).toHaveCount(0);
      await page.evaluate(() => document.fonts.ready);
      const overflow = await page.locator('.slide-stage .slide-text').evaluateAll((nodes) => nodes.filter((node) => node.scrollWidth > node.clientWidth + 2 || node.scrollHeight > node.clientHeight + 2).map((node) => node.textContent));
      if (overflow.length) throw new Error(`${preset.id}: ${JSON.stringify(overflow)}`);
      await expect(page.getByRole('alert')).toHaveCount(0);
      const screenshot = await page.locator('.slide-stage .slide-surface').screenshot({ path: join(directory, 'studio-previews', `${preset.id.replaceAll('/', '-')}.png`) });
      if (!(await sharp(screenshot).stats()).channels.some((channel) => channel.stdev > 8)) throw new Error(`Blank ${preset.id}`);
      pixels = await sharp(screenshot).resize(384, 216).png().toBuffer();
    } else {
      const source = resolve('.artifacts/parts-previews', `${preset.id.replaceAll('/', '-')}.png`);
      pixels = await sharp(await readFile(source)).resize(384, 171).png().toBuffer();
    }
    overlays.push({ input: pixels, left: index % 3 * 392 + 4, top: Math.floor(index / 3) * (japanese ? 224 : 179) + 4 });
  }
  await sharp({ create: { width: 1176, height: Math.ceil(batch.length / 3) * (japanese ? 224 : 179), channels: 3, background: '#dce2df' } }).composite(overlays).jpeg({ quality: 90 }).toFile(join(directory, `overview-${offset / 27 + 1}.jpg`));
}
if (pageErrors.length) throw new Error(JSON.stringify(pageErrors));
await writeFile(join(directory, 'evidence.json'), JSON.stringify(evidence, null, 2), { encoding: 'utf8', flag: 'wx' });
console.log(JSON.stringify({ directory, ...evidence }, null, 2));
} finally { await browser?.close(); }
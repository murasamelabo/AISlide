import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { join } from 'node:path';
import { requestCore } from './core-client.mjs';
import { AislideClient } from '../packages/client/index.mjs';

const url = 'https://api.worldbank.org/v2/country/JPN/indicator/SP.POP.TOTL?date=2020:2023&format=json';
const directory = join('.artifacts', `benchmark-${Date.now()}`);
await mkdir(directory, { recursive: true });
const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(30000) });
if (!response.ok) throw new Error(`World Bank API returned HTTP ${response.status}`);
const chunks = []; let length = 0;
for await (const chunk of response.body) { length += chunk.length; if (length > 256 * 1024) throw new Error('Public data response exceeded 256 KiB'); chunks.push(chunk); }
const raw = Buffer.concat(chunks);
const payload = JSON.parse(raw.toString('utf8'));
const values = payload[1];
if (!Array.isArray(values) || values.length !== 4 || values.some((entry) => entry.countryiso3code !== 'JPN' || entry.indicator?.id !== 'SP.POP.TOTL' || !Number.isSafeInteger(entry.value) || !['2020', '2021', '2022', '2023'].includes(entry.date))) throw new Error('Unexpected World Bank data contract');
values.sort((left, right) => left.date.localeCompare(right.date));
const csv = Buffer.from(`Year,Population\n${values.map((entry) => `${entry.date},${entry.value}`).join('\n')}\n`);
await writeFile(join(directory, 'world-bank-response.json'), raw, { flag: 'wx' });
await writeFile(join(directory, 'japan-population.csv'), csv, { flag: 'wx' });
const client = new AislideClient(requestCore);
const source = await client.ingest({ name: 'World Bank - Japan population 2020-2023.csv', format: 'csv', base64: csv.toString('base64'), attribution: {
  citation: `World Bank, World Development Indicators, SP.POP.TOTL, Japan, 2020-2023. API last updated ${payload[0].lastupdated}. Retrieved ${new Date().toISOString()}.`,
  url, license: 'CC BY 4.0 with World Bank additional terms: https://datacatalog.worldbank.org/public-licenses#cc-by',
  derived_from_sha256: createHash('sha256').update(raw).digest('hex'),
  transformation: 'Selected date and value for JPN/SP.POP.TOTL. Sorted ascending by year, renamed columns Year and Population. Values unchanged. Original API response retained beside this CSV.',
} });
const result = await client.dataReport(source, { title: 'Japan population / \u65e5\u672c\u306e\u4eba\u53e3', period: '2020-2023 / WORLD BANK WDI', table_index: 0, category_column: 0, value_columns: [1], row_start: 0, row_count: 4, chart_kind: 'column' });
const titles = [
  'Japan population\n\u65e5\u672c\u306e\u4eba\u53e3', 'Source and selection / \u51fa\u5178\u3068\u9078\u629e', 'Data scope / \u30c7\u30fc\u30bf\u7bc4\u56f2',
  'Population by year / \u5e74\u5225\u4eba\u53e3', 'Recorded trend / \u63a8\u79fb', 'Year comparison / \u5e74\u5225\u6bd4\u8f03',
  'Source values / \u5143\u30c7\u30fc\u30bf', 'Evidence / \u8a3c\u62e0', 'Native assets / \u7de8\u96c6\u53ef\u80fd\u306a\u753b\u50cf',
  'Reproducible workflow / \u518d\u73fe\u624b\u9806', 'Interpretation limits / \u89e3\u91c8\u306e\u9650\u754c', 'Source identity / \u51fa\u5178\u8b58\u5225',
];
for (const [index, slide] of result.compiled.deck.slides.entries()) {
  slide.title = titles[index]; slide.elements.find((element) => element.id === 'title').text = titles[index]; result.report.sections[index].title = titles[index];
}
const icon = await readFile('apps/studio/src-tauri/icons/app.png');
const picture = await requestCore({ op: 'create_picture', id: 'asset-icon', base64: icon.toString('base64'), mime_type: 'image/png', alt: 'AISlide application icon; not population evidence' });
const assetSlide = result.compiled.deck.slides[8];
assetSlide.elements = assetSlide.elements.filter((element) => ['accent', 'period', 'title', 'footer-rule', 'footer', 'page'].includes(element.id));
assetSlide.elements.push({ ...picture, x: 890, y: 265, width: 240, height: 240 });
assetSlide.elements.push({ type: 'text', id: 'asset-caption', x: 64, y: 260, width: 700, height: 290, text: 'Native PowerPoint picture\n\nPosition and crop remain editable.\nThis application icon is not data evidence.\n\nAsset: repository-generated AISlide icon.', font_size: 25, color: '202525', bold: false });
assetSlide.notes += `\nAsset SHA-256: ${createHash('sha256').update(icon).digest('hex')}\nAsset path: apps/studio/src-tauri/icons/app.png`;
const session = await client.createDocument({ id: 'world-bank-benchmark', deck: result.compiled.deck, report: result.report, sources: [source], bindings: result.bindings });
const layout = await requestCore({ op: 'measure_layout', deck: session.document.deck });
const exported = await session.exportProject();
await writeFile(join(directory, 'report.pptx'), Buffer.from(exported.base64, 'base64'), { flag: 'wx' });
await writeFile(join(directory, 'report.aislide.json'), JSON.stringify(exported.checkpoint), { flag: 'wx' });
await writeFile(join(directory, 'report.scene.json'), JSON.stringify(session.document.deck), { flag: 'wx' });
await writeFile(join(directory, 'layout.json'), JSON.stringify(layout, null, 2), { flag: 'wx' });
const errors = layout.issues.filter((issue) => issue.severity === 'error');
console.log(JSON.stringify({ directory, source_url: url, source_sha256: source.sha256, upstream_sha256: source.attribution.derived_from_sha256, values, slides: 12, charts: 3, pictures: 1, bindings: result.bindings.length, measured_frames: layout.measurements.length, fonts: layout.fonts, layout_errors: errors }, null, 2));
if (errors.length) process.exitCode = 1;
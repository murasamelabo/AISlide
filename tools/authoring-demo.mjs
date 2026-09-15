import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { requestCore } from './core-client.mjs';

const directory = join('.artifacts', `authoring-${Date.now()}`);
const singleMaster = process.argv.includes('--single-master');
const design = await requestCore({ op: 'design_defaults' });
design.theme.name = 'AISlide authoring verification';
design.theme.colors.accent1 = 'B53055';
design.theme.fonts.major = 'Arial';
design.masters[0].elements.push({ type: 'text', id: 'master-footer', x: 64, y: 664, width: 900, height: 32, text: 'Synthetic authoring verification | Master content', font_size: 14, color: '@dk2', bold: false });
const logo = await requestCore({ op: 'create_picture', id: 'master-logo', base64: (await readFile('apps/studio/src-tauri/icons/app.png')).toString('base64'), mime_type: 'image/png', alt: 'AISlide repository application icon' });
design.masters[0].elements.push({ ...logo, x: 1130, y: 34, width: 72, height: 72 });
design.masters.push({ id: 'master-2', name: 'Alternate master', background: '@lt2', elements: [{ type: 'text', id: 'alternate-footer', x: 64, y: 664, width: 900, height: 32, text: 'Alternate master content', font_size: 14, color: '@accent3', bold: false }] });
design.layouts.push({ ...structuredClone(design.layouts[1]), id: 'alternate-content', name: 'Alternate content', master_id: 'master-2' });
if (singleMaster) { design.masters.pop(); design.layouts.pop(); }
let deck = { version: 1, title: 'AISlide authoring verification', width: 1280, height: 720, design, slides: [] };
function slide(title, elements = []) {
  const id = `slide-${deck.slides.length + 1}`;
  const heading = { type: 'text', id: 'title', x: 64, y: 70, width: 1000, height: 100, text: title, font_size: 38, color: '@dk1', bold: true, format: { font_family: '@major' } };
  deck.slides.push({ id, title, background: '@lt1', layout_id: 'blank', inherit_background: true, elements: [heading, ...elements], notes: 'Synthetic demonstration values. Generated for native authoring verification, not factual business results.' });
  return id;
}
const first = slide('Editable masters, layouts and themes');
deck = await requestCore({ op: 'assign_layout', deck, slide_id: first, layout_id: 'title-content' });
deck.slides[0].elements.find((element) => element.id === 'body').text = 'Native placeholders\nTheme fonts and colors\nCommon master text and logo';
const catalog = await requestCore({ op: 'object_catalog' });
for (let offset = 0; offset < catalog.shapes.length; offset += 8) {
  const elements = [];
  for (const [index, entry] of catalog.shapes.slice(offset, offset + 8).entries()) {
    const element = await requestCore({ op: 'create_object', id: `shape-${entry.id}`, kind: 'shape', preset: entry.id });
    elements.push({ ...element, x: 64 + index % 4 * 296, y: 210 + Math.floor(index / 4) * 210, width: 240, height: 158, text: entry.name, font_size: 16, format: { alignment: 'center', vertical: 'middle', hyperlink: entry.id === 'rightArrow' ? 'https://example.invalid/authoring' : null } });
  }
  slide(`Native shapes ${offset + 1}-${Math.min(offset + 8, catalog.shapes.length)}`, elements);
}
for (const kind of catalog.charts) {
  const chart = await requestCore({ op: 'create_object', id: `chart-${kind}`, kind: 'chart', preset: kind });
  slide(`Synthetic ${kind.replaceAll('_', ' ')} chart`, [{ ...chart, x: 100, y: 190, width: 1080, height: 420 }]);
}
const table = await requestCore({ op: 'create_object', id: 'native-table', kind: 'table', rows: 4, columns: 3 });
table.rows = [['Object', 'Source', 'State'], ['Table', 'Native cells', 'Editable'], ['Picture', 'Repository icon', 'Embedded'], ['Chart', 'Synthetic values', 'Embedded workbook']];
slide('Editable table and embedded picture', [{ ...table, x: 64, y: 230, width: 780, height: 260 }, { ...logo, id: 'slide-picture', x: 938, y: 260, width: 200, height: 200 }]);
slide('Native grouped process objects', [await requestCore({ op: 'create_diagram', id: 'native-flow', steps: ['Author', 'Verify', 'Export'] })]);
const last = slide('A second master and inherited layout');
deck = await requestCore({ op: 'assign_layout', deck, slide_id: last, layout_id: singleMaster ? 'title-content' : 'alternate-content' });
deck.slides.at(-1).elements.find((element) => element.id === 'body').text = 'The alternate master controls this slide.\nThe slide retains editable native placeholders.';
const measurements = await requestCore({ op: 'measure_layout', deck });
assert.deepEqual(measurements.issues.filter((issue) => issue.severity === 'error'), []);
const document = await requestCore({ op: 'new_document', id: `authoring-${Date.now()}`, deck });
const exported = await requestCore({ op: 'export_project', document });
await mkdir(directory, { recursive: true });
await writeFile(join(directory, 'authoring.pptx'), Buffer.from(exported.base64, 'base64'), { flag: 'wx' });
await writeFile(join(directory, 'authoring.aislide.json'), JSON.stringify(exported.checkpoint, null, 2), { flag: 'wx' });
await writeFile(join(directory, 'layout.json'), JSON.stringify(measurements, null, 2), { flag: 'wx' });
console.log(JSON.stringify({ directory, slides: deck.slides.length, shapes: catalog.shapes.length, charts: catalog.charts.length, masters: design.masters.length, layouts: design.layouts.length, sha256: exported.checkpoint.pptx_sha256 }));
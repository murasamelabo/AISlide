import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { requestCore } from './core-client.mjs';

const report = await requestCore({ op: 'sample' });
for (const [index, kind] of ['column', 'bar', 'line'].entries()) {
  report.sections[4 + index] = {
    title: `Synthetic ${kind} comparison`, layout: 'chart',
    body: ['Synthetic demonstration values. Not observed business results.'], metrics: [], rows: [],
    chart: { kind, categories: ['Q1', 'Q2', 'Q3', 'Q4'], series: [
      { name: 'Synthetic actual', values: [-2, 0, 8, 12], color: '087F73' },
      { name: 'Synthetic plan', values: [1, 3, 6, 10], color: 'CF5847' },
    ] },
  };
}
const { deck } = await requestCore({ op: 'compile', report });
const diagram = await requestCore({ op: 'create_diagram', id: 'evidence-flow', steps: ['Collect evidence', 'Validate values', 'Publish a copy'] });
deck.slides[7].elements = deck.slides[7].elements.filter((element) => ['accent', 'period', 'title', 'footer-rule', 'footer', 'page'].includes(element.id));
deck.slides[7].elements.push(diagram);
const picture = await requestCore({ op: 'create_picture', id: 'aislide-icon', base64: (await readFile('apps/studio/src-tauri/icons/app.png')).toString('base64'), mime_type: 'image/png', alt: 'AISlide application icon, generated within this repository' });
deck.slides[10].title = 'Editable pictures, native objects';
deck.slides[10].elements = deck.slides[10].elements.filter((element) => ['accent', 'period', 'title', 'footer-rule', 'footer', 'page'].includes(element.id));
deck.slides[10].elements.find((element) => element.id === 'title').text = deck.slides[10].title;
deck.slides[10].elements.push({ type: 'text', id: 'picture-caption', x: 64, y: 260, width: 640, height: 260, text: 'AISlide application icon\n\nEmbedded PNG with editable geometry and crop.\n\nSource: repository-generated application asset.', font_size: 26, color: '202525', bold: false });
deck.slides[10].elements.push({ ...picture, x: 868, y: 255, width: 256, height: 256 });
const exported = await requestCore({ op: 'export', deck });
await mkdir('.artifacts', { recursive: true });
const filename = join('.artifacts', `poc-${Date.now()}.pptx`);
await writeFile(filename, Buffer.from(exported.base64, 'base64'), { flag: 'wx' });
console.log(filename);
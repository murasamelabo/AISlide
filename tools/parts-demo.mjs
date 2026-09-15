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
const evidence = { generated_at: new Date().toISOString(), preset_count: catalog.presets.length, data: 'Synthetic examples, not factual claims', native_editable: true, office_visual_parity: false, files: [] };
for (let offset = 0; offset < catalog.presets.length; offset += 27) {
  const batch = catalog.presets.slice(offset, offset + 27);
  const deck = { version: 1, title: `AISlide parts ${offset / 27 + 1}`, width: 1280, height: 720, slides: batch.map((preset, index) => ({ id: `slide-${index + 1}`, title: `${preset.category_name} / ${preset.name}`, background: '@lt1', notes: `Synthetic example\n${preset.id}\nOriginal AISlide layout. Map outlines: Natural Earth public-domain data.`, elements: [] })) };
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
  evidence.files.push({ filename, slides: batch.length, sha256: createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length, objects: count, presets: batch.map((preset) => preset.id), no_op_identical: true, metadata_current: true });
  const overlays = [];
  for (const [index, preset] of batch.entries()) {
    const source = resolve('.artifacts/parts-previews', `${preset.id.replaceAll('/', '-')}.png`);
    const pixels = await sharp(await readFile(source)).resize(384, 171).png().toBuffer();
    overlays.push({ input: pixels, left: index % 3 * 392 + 4, top: Math.floor(index / 3) * 179 + 4 });
  }
  await sharp({ create: { width: 1176, height: Math.ceil(batch.length / 3) * 179, channels: 3, background: '#dce2df' } }).composite(overlays).jpeg({ quality: 90 }).toFile(join(directory, `overview-${offset / 27 + 1}.jpg`));
}
await writeFile(join(directory, 'evidence.json'), JSON.stringify(evidence, null, 2), { encoding: 'utf8', flag: 'wx' });
console.log(JSON.stringify({ directory, ...evidence }, null, 2));
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { publishNewBundle } from './atomic-output.mjs';

const { values } = parseArgs({ options: {
  'runtime-url': { type: 'string' }, catalog: { type: 'string' },
  out: { type: 'string', default: '.artifacts\\office-interactive' },
  'experimental-copilot': { type: 'boolean', default: false },
} });
if (values['experimental-copilot'] ? Boolean(values['runtime-url'] || values.catalog) : !values['runtime-url'] || !values.catalog) {
  throw new Error('Choose --experimental-copilot alone, OR supply both --runtime-url https://.../office.html and --catalog \\\\server\\share. The tool neither trusts nor installs them.');
}
const directory = resolve(values.out);
await mkdir(directory, { recursive: true });
const client = new AislideClient(requestCore);
const catalog = await client.partCatalog();
const session = await client.createPresentation('office-prototype', 'AISlide Office interactive prototype — synthetic data');
const templates = ['summary', 'dashboard', 'timeline', 'comparison', 'map'];
await session.editSlides([
  { op: 'rename', slide_id: 'slide-1', title: 'Summary — synthetic data' },
  ...templates.slice(1).map((name, index) => ({ op: 'insert', id: `slide-${index + 2}`, after: `slide-${index + 1}`, title: `${name} — synthetic data` })),
]);
const fixtures = [];
for (const [index, template] of templates.entries()) {
  const spec = structuredClone(catalog.presets.find(preset => preset.id === `interactive/${template}`).example);
  spec.layout = { x: 64, y: 112, width: 1152, height: 544, show_title: true };
  await session.addPart(`slide-${index + 1}`, { id: `office-${template}`, spec });
  const payload = await client.compileInteractive({ id: `office-${template}`, spec });
  fixtures.push({ filename: `${template}.json`, bytes: Buffer.from(JSON.stringify(payload)) });
}
const before = structuredClone(session.document);
const source = await session.exportPresentation();
const playback = values['experimental-copilot'] ? await session.exportCopilotInteractive({ acknowledged_experimental: true }) : await session.exportPowerpointInteractive({
  runtime_url: values['runtime-url'], catalog: { kind: 'file_system', location: values.catalog },
});
assert.deepEqual(session.document, before);
const items = [
  { filename: 'interactive-source.pptx', bytes: Buffer.from(source.base64, 'base64') },
  { filename: values['experimental-copilot'] ? 'interactive-copilot-experimental.pptx' : 'interactive-powerpoint.pptx', bytes: Buffer.from(playback.base64, 'base64') },
  ...('manifest' in playback ? [{ filename: 'aislide.addin.xml', bytes: Buffer.from(playback.manifest) }] : []),
  ...fixtures,
];
const receipt = { format: 'aislide.office-content-demo', version: 1, components: playback.components, experimental: values['experimental-copilot'],
  ...('catalog_id' in playback ? { catalog_id: playback.catalog_id, store_type: playback.store_type, public_authoring_api_verified: false, limitations: playback.limitations } : { addin_id: playback.addin_id }),
  powerpoint_playback_verified: false, runtime_url: values['runtime-url'], catalog: values.catalog, warnings: playback.warnings,
  files: items.map(item => ({ filename: item.filename, bytes: item.bytes.length, sha256: createHash('sha256').update(item.bytes).digest('hex') })) };
items.push({ filename: values['experimental-copilot'] ? 'interactive-copilot-experimental.manifest.json' : 'interactive-powerpoint.manifest.json', bytes: Buffer.from(JSON.stringify(receipt, null, 2)) });
const result = await publishNewBundle(directory, items);
console.log(JSON.stringify({ ...result, components: playback.components, experimental: values['experimental-copilot'], ...('catalog_id' in playback ? { catalog_id: playback.catalog_id } : { addin_id: playback.addin_id }), powerpoint_playback_verified: false, runtime_url: values['runtime-url'], catalog: values.catalog }, null, 2));

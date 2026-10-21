import { realpathSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { requestCore } from './core-client.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const catalog = await requestCore({ op: 'part_catalog' });
if (!catalog.presets.some(preset => preset.id === 'interactive/summary')) {
  throw new Error('This core does not support interactive parts. Build this branch and set AISLIDE_CORE_BINARY to its CLI executable.');
}
const server = await createServer({
  configFile: join(root, 'apps', 'studio', 'vite.config.ts'),
  root: join(root, 'apps', 'studio'),
  server: {
    host: '127.0.0.1', port: 4197, strictPort: true,
    fs: { allow: [root, realpathSync(join(root, 'node_modules'))] },
  },
});
await server.listen();
console.log('Interactive prototype: Studio interactions; editable static PPTX only.');
server.printUrls();

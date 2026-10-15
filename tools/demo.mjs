import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { requestCore } from './core-client.mjs';

await mkdir('.artifacts', { recursive: true });
const report = await requestCore({ op: 'sample' });
const { deck } = await requestCore({ op: 'compile', report });
const { base64 } = await requestCore({ op: 'export', deck });
const path = join('.artifacts', `demo-${Date.now()}.pptx`);
await writeFile(path, Buffer.from(base64, 'base64'), { flag: 'wx' });
console.log(path);
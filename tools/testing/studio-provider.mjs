import { createServer } from 'vite';
import { resolve } from 'node:path';
import { requestCore } from '../core-client.mjs';
import { fixtureEnvironment, startProviderFixture } from './provider-fixture.mjs';

const report = await requestCore({ op: 'sample' });
const fixture = await startProviderFixture(report);
Object.assign(process.env, fixtureEnvironment(fixture.endpoint));
const server = await createServer({ root: resolve('apps/studio'), configFile: resolve('apps/studio/vite.config.ts'), server: { host: '127.0.0.1', port: 4176, strictPort: true } });
await server.listen();
console.log('Test-only Studio with synthetic model responses: http://127.0.0.1:4176');
async function close() { await server.close(); await fixture.close(); }
process.once('SIGINT', () => void close());
process.once('SIGTERM', () => void close());
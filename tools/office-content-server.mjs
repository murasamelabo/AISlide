import { createServer as createHttpServer } from 'node:http';
import { createServer as createHttpsServer } from 'node:https';
import { readFile, stat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const { values } = parseArgs({ options: {
  port: { type: 'string', default: '4198' },
  'https-cert': { type: 'string' }, 'https-key': { type: 'string' },
  'http-preview': { type: 'boolean', default: false },
  preview: { type: 'string' },
} });
const port = Number(values.port);
if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('port must be an integer from 1024 through 65535');
const encrypted = Boolean(values['https-cert'] && values['https-key']);
if (Boolean(values['https-cert']) !== Boolean(values['https-key']) || encrypted === values['http-preview']) {
  throw new Error('Supply both --https-cert and --https-key, or explicitly choose --http-preview for browser-only tests. This server never creates or trusts a certificate.');
}
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const assets = new Map([
  ['/office.html', ['office.html', 'text/html; charset=utf-8']],
  ['/office-boot.mjs', ['office-boot.mjs', 'text/javascript; charset=utf-8']],
  ['/runtime.mjs', ['runtime.mjs', 'text/javascript; charset=utf-8']],
  ['/runtime.css', ['runtime.css', 'text/css; charset=utf-8']],
  ['/preview.html', ['preview.html', 'text/html; charset=utf-8']],
  ['/preview-boot.mjs', ['preview-boot.mjs', 'text/javascript; charset=utf-8']],
]);
let preview;
if (values.preview) {
  const path = resolve(values.preview);
  const info = await stat(path);
  if (!info.isFile() || info.size > 2 * 1024 * 1024) throw new Error('Explicit preview fixture must be a JSON file no larger than 2 MiB');
  preview = await readFile(path);
  JSON.parse(preview.toString('utf8').replace(/^\uFEFF/, ''));
}
const handler = async (request, response) => {
  const finish = (status, text) => { response.writeHead(status, { 'Content-Type': 'text/plain; charset=utf-8' }); response.end(text); };
  if (!['GET', 'HEAD'].includes(request.method)) { finish(405, 'Read-only asset server'); return; }
  if (![ `localhost:${port}`, `127.0.0.1:${port}` ].includes(request.headers.host)) { finish(403, 'Invalid host'); return; }
  const headers = {
    'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff', 'Referrer-Policy': 'no-referrer',
    'Content-Security-Policy': "default-src 'none'; script-src 'self' https://appsforoffice.microsoft.com; style-src 'self' 'unsafe-inline'; connect-src 'self' https://appsforoffice.microsoft.com; img-src 'self' data:; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'",
  };
  try {
    let bytes, type;
    // Match only the original allowlisted path; Office may append host/query data.
    const pathname = request.url?.split('?', 1)[0];
    if (pathname === '/preview.json' && preview) { bytes = preview; type = 'application/json; charset=utf-8'; }
    else {
      const asset = assets.get(pathname);
      if (!asset) { finish(404, 'Unknown asset'); return; }
      bytes = await readFile(join(root, 'apps', 'office-content', asset[0]));
      type = asset[1];
    }
    response.writeHead(200, { ...headers, 'Content-Type': type, 'Content-Length': bytes.length });
    response.end(request.method === 'HEAD' ? undefined : bytes);
  } catch (reason) {
    console.error('AISlide content asset could not be served:', reason instanceof Error ? reason.message : String(reason));
    finish(500, 'Content asset is unavailable; inspect the server error');
  }
};
const server = encrypted
  ? createHttpsServer({ cert: await readFile(resolve(values['https-cert'])), key: await readFile(resolve(values['https-key'])), minVersion: 'TLSv1.2' }, (request, response) => { void handler(request, response); })
  : createHttpServer((request, response) => { void handler(request, response); });
server.on('error', reason => { console.error(reason.message); process.exitCode = 1; });
server.listen(port, '127.0.0.1', () => {
  console.log(`AISlide content runtime: ${encrypted ? 'https' : 'http'}://localhost:${port}/office.html`);
  if (!encrypted) console.log('HTTP preview only: PowerPoint manifests require trusted HTTPS; no add-in is installed.');
  if (preview) console.log(`Browser fixture: ${encrypted ? 'https' : 'http'}://localhost:${port}/preview.html (not Office playback verification)`);
});

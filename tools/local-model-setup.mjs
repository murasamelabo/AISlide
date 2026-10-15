import { createReadStream, createWriteStream } from 'node:fs';
import { mkdir, rename, stat, unlink, writeFile } from 'node:fs/promises';
import { createHash, randomUUID } from 'node:crypto';
import { Readable, Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { join } from 'node:path';

if (process.platform !== 'win32') throw new Error('This optional qualification setup targets Windows only');
const directory = join('.tools', 'local-model');
await mkdir(directory, { recursive: true });
async function fileHash(path) {
  const hash = createHash('sha256'); for await (const bytes of createReadStream(path)) hash.update(bytes); return hash.digest('hex');
}
const runtime = process.arch === 'arm64'
  ? { name: 'llama-b10809-bin-win-cpu-arm64.zip', size: 11974499, sha256: 'c1058fe5764a687275c8d20d6bbc1454e787cdbb8ebb8c37a2f959f2b144dc77' }
  : { name: 'llama-b10809-bin-win-cpu-x64.zip', size: 18407457, sha256: '9df3158ed228a641a4b127942d7f459f24c9e13f04682659d05c00c80099b6b5' };
const model = { name: 'qwen2.5-1.5b-instruct-q4_k_m.gguf', size: 1117320736, sha256: '6a1a2eb6d15622bf3c96857206351ba97e1af16c30d7a74ee38970e434e9407e' };
async function download(asset, url) {
  const destination = join(directory, asset.name);
  try {
    const existing = await stat(destination);
    if (existing.size !== asset.size || await fileHash(destination) !== asset.sha256) throw new Error(`Existing asset does not match the pinned hash: ${asset.name}`);
    console.log(`Verified cached ${asset.name}`); return;
  } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const response = await fetch(url, { signal: AbortSignal.timeout(900000) });
  if (!response.ok || !response.url.startsWith('https://')) throw new Error(`Download failed for ${asset.name}: HTTP ${response.status}`);
  const temporary = join(directory, `${asset.name}.${randomUUID()}.tmp`);
  const hash = createHash('sha256'); let length = 0;
  try {
    await pipeline(Readable.fromWeb(response.body), new Transform({ transform(bytes, _encoding, callback) {
      length += bytes.length;
      if (length > asset.size) { callback(new Error('Download exceeded pinned asset size')); return; }
      hash.update(bytes); callback(null, bytes);
    } }), createWriteStream(temporary, { flags: 'wx' }));
    if (length !== asset.size || hash.digest('hex') !== asset.sha256) throw new Error(`Download hash or length mismatch: ${asset.name}`);
    await rename(temporary, destination);
    console.log(`Downloaded and verified ${asset.name} (${length} bytes)`);
  } catch (error) { await unlink(temporary).catch((cleanup) => { if (cleanup.code !== 'ENOENT') throw cleanup; }); throw error; }
}
await download(runtime, `https://github.com/ggml-org/llama.cpp/releases/download/b10809/${runtime.name}`);
await download(model, `https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/91cad51170dc346986eccefdc2dd33a9da36ead9/${model.name}?download=true`);
for (const [name, url] of [
  ['MODEL-LICENSE.txt', 'https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/raw/91cad51170dc346986eccefdc2dd33a9da36ead9/LICENSE'],
  ['RUNTIME-LICENSE.txt', 'https://raw.githubusercontent.com/ggml-org/llama.cpp/b10809/LICENSE'],
]) {
  const response = await fetch(url, { signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`License download failed: ${name}`);
  const text = await response.text(); if (text.length > 100000) throw new Error('Unexpected license size');
  await writeFile(join(directory, name), text);
}
await writeFile(join(directory, 'provenance.json'), JSON.stringify({ runtime, model, runtime_revision: 'b10809', model_revision: '91cad51170dc346986eccefdc2dd33a9da36ead9', purpose: 'Optional local model qualification; never bundled with application source' }, null, 2));
console.log(`Assets ready in ${directory}. Extract the runtime ZIP there before running the local model qualifier.`);
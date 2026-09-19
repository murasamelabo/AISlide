import { createHash } from 'node:crypto';
import { mkdir, readFile, open, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export const model = Object.freeze({
  name: 'u2netp.onnx', bytes: 4574861,
  sha256: '309c8469258dda742793dce0ebea8e6dd393174f89934733ecc8b14c76f4ddd8',
  md5: '8e83ca70e441ab06c318d82300c84806',
  url: 'https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx',
  license: 'https://github.com/xuebinqin/U-2-Net/issues/208',
  releaseCommit: '7fb6683169d588f653281d53c3c258838194c950',
});

export function verifyModel(bytes) {
  if (bytes.length !== model.bytes || createHash('sha256').update(bytes).digest('hex') !== model.sha256 || createHash('md5').update(bytes).digest('hex') !== model.md5) throw new Error('Unsupported model: size or digest mismatch');
  return bytes;
}

async function readBounded(path) {
  const file = await open(path, 'r');
  try {
    const info = await file.stat();
    if (!info.isFile() || info.size !== model.bytes) throw new Error('Unsupported model file size');
    const bytes = Buffer.alloc(model.bytes + 1);
    let length = 0;
    while (length < bytes.length) { const result = await file.read(bytes, length, bytes.length - length); if (!result.bytesRead) break; length += result.bytesRead; }
    return verifyModel(bytes.subarray(0, length));
  } finally { await file.close(); }
}

async function download() {
  let url = new URL(model.url);
  for (let redirects = 0; redirects <= 3; redirects++) {
    const response = await fetch(url, { redirect: 'manual', signal: AbortSignal.timeout(90000) });
    if (response.status >= 300 && response.status < 400) {
      await response.body?.cancel();
      url = new URL(response.headers.get('location'), url);
      if (url.protocol !== 'https:' || !['github.com', 'release-assets.githubusercontent.com', 'objects.githubusercontent.com'].includes(url.hostname) || url.username || url.password) throw new Error('Unexpected model download redirect');
      continue;
    }
    if (!response.ok) throw new Error(`Model download HTTP ${response.status}`);
    const chunks = []; let length = 0;
    for await (const chunk of response.body) { length += chunk.length; if (length > model.bytes) throw new Error('Model download exceeds pinned size'); chunks.push(chunk); }
    return verifyModel(Buffer.concat(chunks));
  }
  throw new Error('Too many model download redirects');
}

export async function setup(args) {
  if (!args.includes('--accept-model-and-dataset-terms')) throw new Error(`Explicit acknowledgement required: --accept-model-and-dataset-terms. Author states Apache-2.0 for code/models; DUTS dataset terms remain your responsibility. ${model.license}`);
  const position = args.indexOf('--file');
  const allowed = position < 0 ? ['--accept-model-and-dataset-terms'] : ['--accept-model-and-dataset-terms', '--file', args[position + 1]];
  if (args.some(arg => !allowed.includes(arg)) || (position >= 0 && !args[position + 1])) throw new Error('Usage: --accept-model-and-dataset-terms [--file existing-u2netp.onnx]');
  if (!process.env.LOCALAPPDATA) throw new Error('LOCALAPPDATA is required; other hosts can set AISLIDE_U2NETP_MODEL and AISLIDE_SEGMENTATION_LICENSE_ACCEPTED=1');
  const directory = join(process.env.LOCALAPPDATA, 'AISlide', 'models');
  const target = join(directory, model.name);
  let existing;
  try { existing = await readBounded(target); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const bytes = existing ?? (position >= 0 ? await readBounded(resolve(args[position + 1])) : await download());
  await mkdir(directory, { recursive: true });
  if (!existing) await writeFile(target, bytes, { flag: 'wx' });
  const consent = JSON.stringify({ sha256: model.sha256, accepted_dataset_terms: true });
  const consentPath = join(directory, 'u2netp-consent.json');
  try { await writeFile(consentPath, consent, { flag: 'wx' }); }
  catch (error) { if (error.code !== 'EEXIST') throw error; if ((await readFile(consentPath, 'utf8')) !== consent) throw new Error('Existing consent file differs; inspect it locally before retrying'); }
  return { path: target, ...model, additional_model_bytes: existing ? 0 : bytes.length, weights_bundled: false, release_immutable: false };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { console.log(JSON.stringify(await setup(process.argv.slice(2)), null, 2)); }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
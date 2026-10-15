import { mkdir, readFile, writeFile, stat } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';

const revision = '66a36c8c94b1a5d992ee4e7f392fccfe4945767c';
const root = resolve('.tools/fonts/phase5');
const base = `https://raw.githubusercontent.com/google/fonts/${revision}/ofl/notosansjp/`;
if (process.argv.slice(2).join(' ') !== '--accept-ofl') throw new Error('Usage: node tools/cjk-font-fixture.mjs --accept-ofl (requires isolated fonttools==4.59.2 in .tools/phase5-fonttools)');
await mkdir(root, { recursive: true });
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
async function download(name, filename, expectedSize) {
  const destination = join(root, filename);
  let bytes;
  try { bytes = await readFile(destination); } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    const response = await fetch(base + name, { redirect: 'error', signal: AbortSignal.timeout(60000) });
    if (!response.ok) throw new Error(`Font source returned ${response.status}`);
    const chunks = []; let length = 0;
    for await (const chunk of response.body) {
      length += chunk.length;
      if (length > expectedSize) throw new Error('Font download exceeded its pinned byte count');
      chunks.push(chunk);
    }
    bytes = Buffer.concat(chunks);
    if (bytes.length !== expectedSize) throw new Error('Font download size mismatch');
    await writeFile(destination, bytes, { flag: 'wx' });
  }
  if (bytes.length !== expectedSize) throw new Error('Existing fixture differs from the pinned source size');
  return { path: destination, bytes: bytes.length, sha256: sha256(bytes), url: base + name };
}
const license = await download('OFL.txt', 'OFL.txt', 4388);
if (!(await readFile(license.path, 'utf8')).includes('SIL OPEN FONT LICENSE Version 1.1')) throw new Error('Expected OFL license missing');
const original = await download('NotoSansJP%5Bwght%5D.ttf', 'NotoSansJP-variable.ttf', 9589900);
const output = join(root, 'NotoSansJP-Regular.ttf');
const env = { ...process.env, PYTHONPATH: resolve('.tools/phase5-fonttools') };
let exists = true;
try { await stat(output); } catch (error) { if (error.code !== 'ENOENT') throw error; exists = false; }
if (!exists) {
  const generated = spawnSync('python', ['-m', 'fontTools.varLib.instancer', original.path, 'wght=400', '--update-name-table', '--output', output], { env, stdio: ['ignore', 'inherit', 'inherit'], shell: false });
  if (generated.status !== 0) throw new Error('Static FontTools instancing failed');
}
const verify = spawnSync('python', ['-c', 'import sys,json,fontTools; from fontTools.ttLib import TTFont; original=TTFont(sys.argv[1]); font=TTFont(sys.argv[2]); assert fontTools.__version__=="4.59.2"; assert "fvar" not in font; assert len(original.getGlyphOrder())==len(font.getGlyphOrder()); assert font["OS/2"].fsType==original["OS/2"].fsType; print(json.dumps({"fonttools":fontTools.__version__,"glyphs":len(font.getGlyphOrder()),"fsType":font["OS/2"].fsType,"family":font["name"].getDebugName(1),"subsetting":False}))', original.path, output], { env, encoding: 'utf8', shell: false });
if (verify.status !== 0) throw new Error(verify.stderr);
const metadata = JSON.parse(verify.stdout);
const bytes = await readFile(output);
if (bytes.length <= 1048576 || bytes.length > 12 * 1048576) throw new Error('Fixture must be a real static font between 1 and 12 MiB');
const result = { revision, original, license, static: { path: output, bytes: bytes.length, sha256: sha256(bytes), ...metadata }, office_verified: false };
await writeFile(join(root, 'fixture.json'), JSON.stringify(result, null, 2), { flag: 'wx' }).catch(error => { if (error.code !== 'EEXIST') throw error; });
console.log(JSON.stringify(result, null, 2));
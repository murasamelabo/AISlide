import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { join } from 'node:path';
import { requestCore } from './core-client.mjs';

const spec = JSON.parse(await readFile('docs/testing/corpus.json', 'utf8'));
const directory = join('.artifacts', 'corpus', spec.revision);
await mkdir(directory, { recursive: true });
async function cached(name, url) {
  const path = join(directory, name);
  try { return await readFile(path); } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`Corpus download failed: ${name}, HTTP ${response.status}`);
  const parts = []; let size = 0;
  for await (const chunk of response.body) {
    size += chunk.length;
    if (size > 2 * 1024 * 1024) throw new Error(`Corpus download exceeded 2 MiB: ${name}`);
    parts.push(chunk);
  }
  const bytes = Buffer.concat(parts); await writeFile(path, bytes, { flag: 'wx' }); return bytes;
}
await cached('LICENSE', spec.license_url);
await cached('NOTICE', spec.notice_url);
const results = [];
for (const name of spec.files) {
  const url = `https://raw.githubusercontent.com/${spec.repository}/${spec.revision}/${spec.directory}/${encodeURIComponent(name)}`;
  const bytes = await cached(name, url);
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  assert.equal(sha256, spec.sha256[name], `Public corpus fixture hash changed: ${name}`);
  const base64 = bytes.toString('base64');
  const result = { name, url, bytes: bytes.length, sha256, noop: false, patch: 'not-tested', preview: 'not-tested' };
  try {
    const inspection = await requestCore({ op: 'inspect', base64 });
    result.slides = inspection.slides.length;
    const saved = await requestCore({ op: 'roundtrip', base64 });
    assert.deepEqual(Buffer.from(saved.base64, 'base64'), bytes);
    result.noop = true;
    const candidates = inspection.slides.flatMap((slide) => slide.texts.map((text) => ({ part: slide.part, ...text }))).filter((text) => text.text && text.text.length < 3900).slice(0, 8);
    result.patch = candidates.length ? 'unsupported-text-structure' : 'no-simple-text';
    for (const candidate of candidates) {
      let patched;
      try { patched = await requestCore({ op: 'patch_text', base64, part: candidate.part, shape_id: candidate.shape_id, run_index: candidate.run_index, expected: candidate.text, text: `${candidate.text} [AISlide test]` }); }
      catch (error) { if (/Unsupported:/.test(error.message)) continue; throw error; }
      const originalParts = await requestCore({ op: 'package_manifest', base64 });
      const changedParts = await requestCore({ op: 'package_manifest', base64: patched.base64 });
      assert.equal(changedParts.parts.length, originalParts.parts.length);
      const changed = new Map(changedParts.parts.map((part) => [part.path, part]));
      for (const part of originalParts.parts) { if (part.path !== candidate.part) assert.deepEqual(changed.get(part.path), part); }
      result.patch = 'passed'; result.changed_part = candidate.part;
      await writeFile(join(directory, `patched-${name}`), Buffer.from(patched.base64, 'base64'));
      break;
    }
    try {
      const preview = await requestCore({ op: 'import_pptx', base64 });
      result.preview = 'approximate'; result.preview_warnings = preview.warnings;
      result.preview_objects = preview.deck.slides.reduce((count, slide) => count + slide.elements.length, 0);
    } catch (error) { result.preview = 'rejected'; result.preview_reason = error.message; }
  } catch (error) { result.error = error.message; }
  results.push(result);
  console.log(`${name}: no-op=${result.noop}, patch=${result.patch}, preview=${result.preview}${result.error ? `, ${result.error}` : ''}`);
}
const report = { checked_at: new Date().toISOString(), repository: spec.repository, revision: spec.revision, license_url: spec.license_url, notice_url: spec.notice_url, scope: 'Package preservation and bounded preview, not Office visual parity or unrestricted redistribution rights', results };
await writeFile(join(directory, 'results.json'), JSON.stringify(report, null, 2));
console.log(JSON.stringify({ total: results.length, noop_passed: results.filter((entry) => entry.noop).length, patched: results.filter((entry) => entry.patch === 'passed').length, previewed: results.filter((entry) => entry.preview === 'approximate').length, report: join(directory, 'results.json') }));
if (results.some((result) => !result.noop || result.error)) process.exitCode = 1;
import test from 'node:test';
import assert from 'node:assert/strict';
import sharp from 'sharp';
import { requestCore } from './core-client.mjs';

test('installed Windows OCR extracts source text and word geometry locally', { timeout: 30000, skip: process.platform !== 'win32' }, async (context) => {
  const status = await requestCore({ op: 'ocr_status' });
  if (!status.available) { context.skip(status.message); return; }
  const language = status.languages.find((language) => language.startsWith('en')) ?? status.languages[0];
  assert.ok(language, 'Available OCR must report at least one installed language');
  const bitmap = await sharp(Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="240"><rect width="1000" height="240" fill="white"/><text x="40" y="140" font-family="Arial" font-size="72" fill="black">Revenue 42</text></svg>')).png().toBuffer();
  const source = await requestCore({ op: 'ingest', input: { name: 'known-text.png', format: 'png', base64: bitmap.toString('base64'), ocr: true, ocr_language: language } });
  assert.match(source.text, /Revenue\s+42/i);
  assert.ok(source.pages[0].regions.length >= 2);
  assert.match(source.pages[0].method, /^windows_ocr:/);
  assert.ok(source.warnings.some((warning) => warning.includes('unverified')));
  assert.equal(source.tables.length, 0);
});
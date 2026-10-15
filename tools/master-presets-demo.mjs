import assert from 'node:assert/strict';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import { AislideClient } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';

const client = new AislideClient(requestCore);
await mkdir('.artifacts', { recursive: true });
const directory = await mkdtemp(resolve('.artifacts/master-presets-'));
const records = [];
const parts = await client.partCatalog();
const flow = parts.presets.find((entry) => entry.id === 'flow/balanced').example;
for (const preset of await client.designPresets()) {
  const session = await client.createPresentation(`demo-${preset.id}`, preset.name);
  await session.applyDesignPreset(preset.id);
  await session.assignLayout('slide-1', 'preset-cover');
  await session.editSlides(preset.design.layouts.filter((layout) => layout.id !== 'preset-cover').map((layout) => ({ op: 'insert', id: layout.id, title: layout.name, layout_id: layout.id })));
  const deck = session.document.deck;
  for (const slide of deck.slides) {
    slide.notes = 'Original design preset. Demonstration wording only; no factual business data or AI-generated content.';
    for (const element of slide.elements) {
      if (element.type !== 'text') continue;
      if (slide.layout_id === 'preset-cover') element.text = element.id === 'title' ? '事業計画' : '計画と次のステップ / Project briefing';
      else if (element.id === 'title') element.text = slide.layout_id === 'preset-section' ? '次のステップ' : '取り組みの概要 / Overview';
      else element.text = '課題を整理する\n選択肢を比較する\n次の行動を決める';
    }
  }
  await session.replaceDeck(deck);
  await session.addPart('preset-visual-content', { id: 'visual-flow', spec: flow });
  const exported = await session.exportPresentation();
  const bytes = Buffer.from(exported.base64, 'base64');
  const opened = (await client.openPresentation(`verify-${preset.id}`, exported.base64)).session;
  assert.equal(opened.document.deck.slides.length, 7);
  assert.equal(opened.document.deck.design.layouts.length, 11);
  assert.equal(opened.document.parts[0].stale, false);
  await opened.applyDesignPreset(preset.id);
  const filename = `${preset.id}.pptx`;
  await writeFile(join(directory, filename), bytes, { flag: 'wx' });
  records.push({ preset_id: preset.id, filename, slides: 7, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), rules: preset.rules, fonts: preset.design.theme.fonts, colors: preset.design.theme.colors });
}
await writeFile(join(directory, 'manifest.json'), `${JSON.stringify(records, null, 2)}\n`, { flag: 'wx' });
console.log(JSON.stringify({ directory, presentations: records.length, slides: records.length * 7, records: records.map(({ rules, fonts, colors, ...record }) => record) }, null, 2));
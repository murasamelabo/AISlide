import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { icons } from 'lucide-react';

test('bundled Lucide metadata covers every installed canonical icon and category', async () => {
  const data = JSON.parse(await readFile(new URL('../apps/studio/src/lucide-categories.json', import.meta.url), 'utf8'));
  const installed = JSON.parse(await readFile(new URL('../node_modules/lucide-react/package.json', import.meta.url), 'utf8'));
  assert.equal(data.version, installed.version);
  assert.equal(data.source, `https://codeload.github.com/lucide-icons/lucide/tar.gz/refs/tags/${installed.version}`);
  assert.equal(data.sha256, 'f218860ba3a517abbbdb3a2e183a4ff15cc1e193d8e1283b21787f10226cc2d5');
  assert.deepEqual(Object.keys(data.icons).sort(), Object.keys(icons).sort());
  assert.equal(Object.keys(data.categories).length, 42);
  const used = new Set();
  for (const [name, entry] of Object.entries(data.icons)) {
    assert.ok(entry.categories.length > 0, name);
    assert.equal(new Set(entry.categories).size, entry.categories.length, name);
    assert.ok(entry.tags.every((tag) => typeof tag === 'string'), name);
    for (const category of entry.categories) {
      assert.ok(typeof data.categories[category] === 'string', `${name}: ${category}`);
      used.add(category);
    }
  }
  assert.deepEqual([...used].sort(), Object.keys(data.categories).sort());
  assert.deepEqual(data.icons.Database.categories, ['devices', 'development']);
  assert.ok(data.icons.Cloud.categories.includes('weather'));
});

test('redistributed Lucide metadata retains the complete upstream license', async () => {
  const bundled = await readFile(new URL('../apps/studio/public/lucide-LICENSE.txt', import.meta.url), 'utf8');
  const original = await readFile(new URL('../node_modules/lucide-react/LICENSE', import.meta.url), 'utf8');
  assert.equal(bundled.replace(/^\uFEFF/, ''), original.replace(/^\uFEFF/, ''));
});
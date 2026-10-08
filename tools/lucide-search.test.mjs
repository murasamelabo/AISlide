import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import * as lucideExports from 'lucide-react';
import { searchLucideIcons, resolveLucideIcon } from './lucide-search.mjs';

const metadata = JSON.parse(await readFile(new URL('../apps/studio/src/lucide-categories.json', import.meta.url), 'utf8'));
const entries = Object.entries(metadata.icons);
const names = (results) => results.map(([name]) => name);
const entry = (name, tags = [], categories = []) => [name, { tags, categories }];

test('canonical queries normalize kebab, snake, PascalCase and camelCase', () => {
  const expected = names(searchLucideIcons(entries, 'list checks'));
  assert.equal(expected[0], 'ListChecks');
  for (const query of ['list-checks', 'ListChecks', 'listChecks', 'list_checks', ' LIST_CHECKS ', 'list\nchecks']) {
    assert.deepEqual(names(searchLucideIcons(entries, query)), expected, query);
  }
  assert.equal(names(searchLucideIcons(entries, 'message-square'))[0], 'MessageSquare');
});

test('Lock, Eye and Table lead real catalog searches without cross-boundary infixes', () => {
  const locks = names(searchLucideIcons(entries, 'lock'));
  assert.equal(locks[0], 'Lock');
  assert.equal(locks.some((name) => name.startsWith('AlarmClock')), false);
  const eyes = names(searchLucideIcons(entries, 'eye'));
  assert.equal(eyes[0], 'Eye');
  assert.equal(eyes.includes('BadgeJapaneseYen'), false);
  const tables = names(searchLucideIcons(entries, 'table'));
  assert.equal(tables[0], 'Table');
  assert.equal(tables.includes('Carrot'), false);
});

test('name, tag and category tokens match whole words or prefixes, never infixes', () => {
  const candidates = [entry('AlarmClock'), entry('BadgeJapaneseYen'), entry('Carrot', ['vegetable']), entry('Circle', [], ['stable'])];
  for (const query of ['lock', 'eye', 'table']) {
    assert.deepEqual(searchLucideIcons(candidates, query), [], query);
  }
  assert.deepEqual(names(searchLucideIcons(candidates, 'clock')), ['AlarmClock']);
  assert.deepEqual(names(searchLucideIcons(candidates, 'veg')), ['Carrot']);
  assert.deepEqual(names(searchLucideIcons(candidates, 'sta')), ['Circle']);
});

test('ranking prefers canonical name, name words, name prefix, metadata words, then word prefix', () => {
  const candidates = [
    entry('Alarm', ['locksmith']),
    entry('Badge', ['lock']),
    entry('Circle', [], ['lock']),
    entry('DoorLock'),
    entry('Lockbox'),
    entry('LockOpen'),
    entry('Lock'),
    entry('DoorLockbox'),
  ];
  assert.deepEqual(names(searchLucideIcons(candidates, 'lock')), [
    'Lock', 'DoorLock', 'LockOpen', 'Lockbox', 'Badge', 'Circle', 'Alarm', 'DoorLockbox',
  ]);
});

test('multiline queries require every word across name, tags and categories', () => {
  const candidates = [
    entry('MessageSquare', ['chat', 'team work'], ['communication']),
    entry('MessageCircle', ['chat'], ['communication']),
    entry('Square', ['shape']),
  ];
  assert.deepEqual(names(searchLucideIcons(candidates, 'square\nmessage')), ['MessageSquare']);
  assert.deepEqual(names(searchLucideIcons(candidates, 'message\nteam\tcomm')), ['MessageSquare']);
  assert.deepEqual(searchLucideIcons(candidates, 'message\nmissing'), []);
  assert.deepEqual(searchLucideIcons(candidates, 'square\nmessage\nmissing'), []);
});

test('acronyms and punctuation create word boundaries without losing Unicode metadata', () => {
  const candidates = [entry('XMLFile', ['read-only', 'team_work', 'caf\u00e9'], ['file-management'])];
  for (const query of ['xml-file', 'xmlFile', 'XMLFile', 'read only', 'team\nwork', 'file management', 'caf\u00e9']) {
    assert.deepEqual(searchLucideIcons(candidates, query), candidates, query);
  }
  assert.deepEqual(searchLucideIcons(candidates, 'ile'), []);
  assert.deepEqual(searchLucideIcons(candidates, 'caf\u00e9\nmissing'), []);
});

test('official tags and categories remain searchable', () => {
  const database = entries.find(([name]) => name === 'Database');
  assert.ok(database);
  assert.ok(database[1].tags.length > 0);
  for (const query of [...database[1].tags, ...database[1].categories]) {
    assert.ok(names(searchLucideIcons(entries, query)).includes('Database'), query);
  }
  const development = entries.filter(([, value]) => value.categories.includes('development'));
  assert.ok(names(searchLucideIcons(development, 'database')).includes('Database'));
  assert.ok(searchLucideIcons(development, '').every((result) => development.includes(result)));
});

test('empty queries are alphabetical and do not mutate, duplicate or truncate canonical metadata', async () => {
  const installed = JSON.parse(await readFile(new URL('../node_modules/lucide-react/package.json', import.meta.url), 'utf8'));
  assert.equal(installed.version, '1.43.0');
  assert.equal(metadata.version, installed.version);
  assert.deepEqual(names(entries).sort(), Object.keys(lucideExports.icons).sort());
  const candidates = Object.freeze(entries.toReversed().map(([name, value]) => Object.freeze([
    name, Object.freeze({ tags: Object.freeze([...value.tags]), categories: Object.freeze([...value.categories]) }),
  ])));
  const before = JSON.stringify(candidates);
  const expected = Object.keys(lucideExports.icons).sort();
  for (const query of [undefined, null, '', ' \n\t ', '---___']) {
    const results = searchLucideIcons(candidates, query);
    assert.deepEqual(names(results), expected);
    assert.equal(results.length, candidates.length);
    assert.ok(results.every((result) => candidates.includes(result)));
  }
  searchLucideIcons(candidates, 'lock');
  assert.equal(JSON.stringify(candidates), before);
});

test('equal relevance has deterministic canonical ordering across pagination boundaries', () => {
  const candidates = Array.from({ length: 55 }, (_, index) => entry(`Item${String(index).padStart(2, '0')}`, ['shared']));
  const results = searchLucideIcons(candidates.toReversed(), 'shared');
  assert.deepEqual(names(results), names(candidates));
  const pages = [results.slice(0, 20), results.slice(20, 40), results.slice(40, 60)];
  assert.deepEqual(names(pages.flat()), names(candidates));
  assert.deepEqual(searchLucideIcons(candidates, 'shared'), results);
});

test('canonical lookup returns the exact installed component', () => {
  for (const [name, component] of Object.entries(lucideExports.icons)) {
    assert.deepEqual(resolveLucideIcon(lucideExports, name), { name, component });
  }
});

test('real History and Fingerprint exports resolve by canonical component identity', () => {
  for (const [alias, canonical] of [['History', 'RotateCcwClock'], ['Fingerprint', 'FingerprintPattern']]) {
    assert.equal(Object.hasOwn(lucideExports.icons, alias), false);
    assert.equal(lucideExports[alias], lucideExports.icons[canonical]);
    const resolved = resolveLucideIcon(lucideExports, alias, lucideExports.icons);
    assert.deepEqual(resolved, { name: canonical, component: lucideExports.icons[canonical] });
  }
});

test('every real alias resolves without adding duplicate canonical icons', () => {
  const canonical = new Map(Object.entries(lucideExports.icons).map(([name, component]) => [component, name]));
  const before = Object.keys(lucideExports.icons);
  let aliasCount = 0;
  for (const [name, component] of Object.entries(lucideExports)) {
    if (Object.hasOwn(lucideExports.icons, name) || !canonical.has(component)) continue;
    aliasCount += 1;
    assert.deepEqual(resolveLucideIcon(lucideExports, name), { name: canonical.get(component), component });
  }
  assert.ok(aliasCount > 0);
  assert.deepEqual(Object.keys(lucideExports.icons), before);
  assert.equal(searchLucideIcons(entries, '').length, before.length);
});

test('lookup rejects helpers, missing exports, inherited names and invented aliases', () => {
  for (const name of ['Icon', 'createLucideIcon', 'icons', 'default', 'toString', 'constructor', '__proto__', 'MissingIcon', 'history']) {
    assert.equal(resolveLucideIcon(lucideExports, name), undefined, name);
  }
  assert.equal(resolveLucideIcon({ icons: lucideExports.icons }, 'History'), undefined);
  assert.equal(resolveLucideIcon({ icons: lucideExports.icons }, 'Fingerprint'), undefined);
  assert.equal(resolveLucideIcon(Object.create(lucideExports), 'History', lucideExports.icons), undefined);
  const unrelated = { ...lucideExports.icons.Lock };
  assert.equal(resolveLucideIcon({ icons: lucideExports.icons, Impostor: unrelated }, 'Impostor'), undefined);
  assert.equal(resolveLucideIcon({ icons: lucideExports.icons, Helper() {} }, 'Helper'), undefined);
});

test('both new helper files contain exactly one leading UTF-8 BOM', async () => {
  for (const filename of ['lucide-search.mjs', 'lucide-search.test.mjs']) {
    const bytes = await readFile(new URL(filename, import.meta.url));
    assert.equal(bytes.subarray(0, 3).toString('hex'), 'efbbbf', filename);
    assert.notEqual(bytes.subarray(3, 6).toString('hex'), 'efbbbf', filename);
  }
});
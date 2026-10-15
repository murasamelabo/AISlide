import { mkdir, mkdtemp, readFile, writeFile, rm, readdir } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { join, resolve } from 'node:path';
import { icons } from 'lucide-react';

const root = resolve(import.meta.dirname, '..');
const version = JSON.parse(await readFile(join(root, 'node_modules/lucide-react/package.json'), 'utf8')).version;
if (version !== '1.43.0') throw new Error('Review Lucide metadata before updating the pinned version');
const source = `https://codeload.github.com/lucide-icons/lucide/tar.gz/refs/tags/${version}`;
const response = await fetch(source);
if (!response.ok) throw new Error(`Lucide source download failed: ${response.status}`);
const archive = Buffer.from(await response.arrayBuffer());
if (archive.length > 64 * 1024 * 1024) throw new Error('Lucide archive exceeds 64 MiB');
const sha256 = createHash('sha256').update(archive).digest('hex');
if (sha256 !== 'f218860ba3a517abbbdb3a2e183a4ff15cc1e193d8e1283b21787f10226cc2d5') throw new Error('Lucide source archive integrity changed');
await mkdir(join(root, '.artifacts'), { recursive: true });
const temporary = await mkdtemp(join(root, '.artifacts/lucide-categories-'));
try {
  const archivePath = join(temporary, 'source.tar.gz');
  await writeFile(archivePath, archive, { flag: 'wx' });
  const prefix = `lucide-${version}`;
  execFileSync('tar', ['-xzf', archivePath, '-C', temporary, `${prefix}/icons`, `${prefix}/categories`, `${prefix}/LICENSE`], { stdio: 'inherit', shell: false });
  const upstream = join(temporary, prefix);
  const metadata = new Map();
  for (const file of await readdir(join(upstream, 'icons'))) {
    if (!file.endsWith('.json')) continue;
    const data = JSON.parse(await readFile(join(upstream, 'icons', file), 'utf8'));
    metadata.set(file.slice(0, -5).replaceAll('-', '').toLowerCase(), data);
  }
  const categories = {};
  for (const file of (await readdir(join(upstream, 'categories'))).sort()) {
    if (!file.endsWith('.json')) continue;
    const data = JSON.parse(await readFile(join(upstream, 'categories', file), 'utf8'));
    categories[file.slice(0, -5)] = data.title;
  }
  const entries = {};
  for (const id of Object.keys(icons)) {
    const data = metadata.get(id.toLowerCase());
    if (!data || !Array.isArray(data.categories) || data.categories.length === 0 || data.categories.some((category) => !Object.hasOwn(categories, category))) throw new Error(`Missing category metadata: ${id}`);
    entries[id] = { categories: data.categories, tags: data.tags ?? [] };
  }
  const result = { version, source, sha256, categories, icons: entries };
  await writeFile(join(root, 'apps/studio/src/lucide-categories.json'), `${JSON.stringify(result)}\n`, 'utf8');
  await writeFile(join(root, 'apps/studio/public/lucide-LICENSE.txt'), `\uFEFF${await readFile(join(upstream, 'LICENSE'), 'utf8')}`, 'utf8');
  console.log(`Generated ${Object.keys(entries).length} icons in ${Object.keys(categories).length} official categories; source SHA-256 ${result.sha256}`);
} finally {
  await rm(temporary, { recursive: true, force: true });
}
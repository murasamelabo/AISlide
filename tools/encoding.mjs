import { readFile, writeFile, readdir } from 'node:fs/promises';
import { join, extname } from 'node:path';

const ignored = new Set(['.git', '.tools', '.artifacts', 'node_modules', 'target', 'dist', 'gen']);
const sourceExtensions = new Set(['.rs', '.mjs', '.ts', '.mts', '.tsx', '.css', '.html', '.md', '.toml', '.ps1', '.psm1', '.nsh', '.yml', '.yaml']);
const fix = process.argv.includes('--fix');
let checked = 0;
let mismatches = 0;
async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (ignored.has(entry.name)) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) { await walk(path); continue; }
    const extension = extname(path);
    if (!sourceExtensions.has(extension) && extension !== '.json') continue;
    const value = await readFile(path, 'utf8');
    const normalized = `${extension === '.json' ? '' : '\uFEFF'}${value.replace(/^\uFEFF+/, '')}`;
    checked += 1;
    if (value === normalized) continue;
    mismatches += 1;
    if (fix) await writeFile(path, normalized, 'utf8');
    else console.error(`Encoding mismatch: ${path}`);
  }
}
await walk('.');
console.log(`Checked ${checked} source/config files; ${mismatches} ${fix ? 'normalized' : 'mismatches'}.`);
if (mismatches && !fix) process.exitCode = 1;
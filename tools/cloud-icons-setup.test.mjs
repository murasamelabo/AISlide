import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import childProcess from 'node:child_process';
import { spawnSync } from 'node:child_process';
import filesystem from 'node:fs/promises';
import { mkdir, mkdtemp, readFile, readdir, rm, stat, symlink, writeFile } from 'node:fs/promises';
import { syncBuiltinESMExports } from 'node:module';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';
import sharp from 'sharp';
import {
  buildCatalog, catalogPath, defaultPackDirectory, downloadArchive, inspectPng, installIconPacks,
  prepareIconPng, readArchiveEntries, verifyCatalog,
} from './cloud-icons-setup.mjs';

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const readCatalog = async () => verifyCatalog(await readFile(catalogPath));
const noNetwork = () => { throw new Error('Unexpected network access'); };
const png = (width = 24, height = 12, alpha = 1) => sharp({
  create: { width, height, channels: 4, background: { r: 50, g: 120, b: 220, alpha } },
}).png().toBuffer();
const imageSource = (bytes, entry = 'synthetic.png') => ({
  id: 'azure/test/synthetic', provider: 'azure',
  source: { archive_id: 'azure-v24.zip', entry, sha256: hash(bytes) },
});

async function temporary(context) {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-cloud-icons-test-'));
  context.after(() => rm(directory, { recursive: true, force: true }));
  return directory;
}

test('cloud icon setup requires explicit consent before inspecting paths or fetching', async () => {
  const options = {
    get packDirectory() { throw new Error('Destination accessed before consent'); },
    get fetch() { throw new Error('Network accessed before consent'); },
  };
  await assert.rejects(installIconPacks(options), /--accept-vendor-terms/);
});

test('Windows remote and unknown backing drives reject before any filesystem access', {
  skip: process.platform !== 'win32',
}, async context => {
  let fileCalls = 0;
  let driveCalls = 0;
  const queries = [];
  let answer = '4\r\n';
  let rejectAllDrives = true;
  for (const method of ['lstat', 'realpath', 'open', 'mkdir', 'mkdtemp', 'rmdir']) {
    context.mock.method(filesystem, method, async () => {
      fileCalls += 1;
      throw new Error('Unexpected filesystem access before drive rejection');
    });
  }
  context.mock.method(childProcess, 'execFileSync', (executable, args, options) => {
    driveCalls += 1;
    queries.push({ executable, args, options });
    if (!rejectAllDrives && !args.at(-1).includes("'Z:\\'")) return '3\r\n';
    if (answer instanceof Error) throw answer;
    return answer;
  });
  syncBuiltinESMExports();
  try {
    const options = { acceptVendorTerms: true, fetch: noNetwork };
    const calls = [
      () => installIconPacks({ ...options, packDirectory: 'Z:\\synthetic-pack' }),
      () => installIconPacks(options),
      () => installIconPacks({ ...options, packDirectory: defaultPackDirectory({ env: { AISLIDE_ICON_PACK_ROOT: 'Z:\\synthetic-pack' } }) }),
      () => installIconPacks({ ...options, packDirectory: defaultPackDirectory({ platform: 'win32', env: { LOCALAPPDATA: 'Z:\\synthetic-data' } }) }),
      () => buildCatalog({ ...options, researchDirectory: 'Z:\\synthetic-research' }),
    ];
    for (answer of ['0', '1', '4', '7', '', '3 garbage', new Error('Synthetic drive query failed')]) {
      for (const invoke of calls) {
        const before = driveCalls;
        await assert.rejects(invoke(), /local backing drive/);
        assert.equal(fileCalls, 0);
        assert.equal(driveCalls, before + 1);
      }
      rejectAllDrives = false;
      for (const invoke of [
        () => installIconPacks({ ...options, packDirectory: 'C:\\synthetic-pack', cacheDirectory: 'Z:\\synthetic-cache' }),
        () => buildCatalog({ ...options, researchDirectory: 'C:\\synthetic-research', cacheDirectory: 'Z:\\synthetic-cache' }),
      ]) {
        await assert.rejects(invoke(), /local backing drive/);
        assert.equal(fileCalls, 0);
      }
      rejectAllDrives = true;
    }
    for (const { executable, args, options } of queries) {
      assert.match(executable, /^[A-Za-z]:\\.*\\System32\\WindowsPowerShell\\v1\.0\\powershell\.exe$/i);
      assert.deepEqual(args.slice(0, -1), ['-NoLogo', '-NoProfile', '-NonInteractive', '-Command']);
      assert.match(args.at(-1), /^\[int\]\[System\.IO\.DriveInfo\]::new\('[A-Z]:\\'\)\.DriveType$/);
      assert.equal(options.shell, false);
      assert.equal(options.windowsHide, true);
      assert.equal(options.timeout, 10000);
      assert.equal(options.encoding, 'utf8');
      assert.equal(options.maxBuffer, 1024);
      assert.deepEqual(options.stdio, ['ignore', 'pipe', 'pipe']);
    }
  } finally {
    context.mock.restoreAll();
    syncBuiltinESMExports();
  }
});

test('Windows known local drive types pass classification before directory inspection', {
  skip: process.platform !== 'win32',
}, async context => {
  const events = [];
  let driveType = 3;
  context.mock.method(childProcess, 'execFileSync', () => {
    events.push('drive');
    return `${driveType}\r\n`;
  });
  context.mock.method(filesystem, 'lstat', async () => {
    events.push('filesystem');
    throw new Error('Synthetic filesystem boundary');
  });
  syncBuiltinESMExports();
  try {
    for (driveType of [2, 3, 5, 6]) {
      events.length = 0;
      await assert.rejects(installIconPacks({ acceptVendorTerms: true, packDirectory: 'Z:\\synthetic-pack', fetch: noNetwork }), /Synthetic filesystem boundary/);
      assert.equal(events.at(-1), 'filesystem');
      assert.ok(events.slice(0, -1).length > 0);
      assert.ok(events.slice(0, -1).every(event => event === 'drive'));
    }
  } finally {
    context.mock.restoreAll();
    syncBuiltinESMExports();
  }
});

test('trusted SVG conversion preserves local fragment gradients but rejects external references', async () => {
  const make = source => {
    const bytes = Buffer.from(source);
    return { bytes, icon: { id: 'azure/test/gradient', provider: 'azure', source: {
      archive_id: 'azure-v24.zip', entry: 'synthetic.svg', sha256: createHash('sha256').update(bytes).digest('hex'),
    } } };
  };
  const source = '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="18" height="9"><defs><linearGradient id="123-source"><stop stop-color="#f00"/><stop offset="1" stop-color="#00f"/></linearGradient><linearGradient id="copy" xlink:href="#123-source"/></defs><rect width="18" height="9" fill="url(#copy)"/></svg>';
  const valid = make(source);
  const prepared = await prepareIconPng(valid.icon, valid.bytes);
  assert.equal(prepared.width, 256);
  assert.equal(prepared.height, 128);
  for (const reference of ['https://example.invalid/gradient.svg#id', 'file:///C:/private.svg', 'data:image/svg+xml,test']) {
    const invalid = make(source.replace('xlink:href="#123-source"', `xlink:href="${reference}"`));
    await assert.rejects(prepareIconPng(invalid.icon, invalid.bytes), /Active or external/);
  }
});

test('default paths are release-specific and operator overrides must be absolute local paths', () => {
  const base = resolve(tmpdir());
  assert.equal(defaultPackDirectory({ platform: 'win32', env: { LOCALAPPDATA: base } }), join(base, 'AISlide', 'icon-packs', '2026-09-20'));
  assert.equal(defaultPackDirectory({ platform: 'linux', env: { XDG_DATA_HOME: base } }), join(base, 'AISlide', 'icon-packs', '2026-09-20'));
  assert.equal(defaultPackDirectory({ platform: 'linux', env: {}, homeDirectory: base }), join(base, '.local', 'share', 'AISlide', 'icon-packs', '2026-09-20'));
  const custom = join(base, 'isolated-version');
  assert.equal(defaultPackDirectory({ env: { AISLIDE_ICON_PACK_ROOT: custom } }), custom);
  for (const value of ['relative', '', 'https://example.invalid/pack', '//server/share', '\\\\server\\share', '\\\\?\\C:\\pack', `${base}/../escape`]) {
    assert.throws(() => defaultPackDirectory({ env: { AISLIDE_ICON_PACK_ROOT: value } }), /absolute local/);
  }
});

test('CLI help and missing consent print vendor terms without creating or reading the pack', async context => {
  const directory = await temporary(context);
  const destination = join(directory, 'not-created');
  for (const args of [['--help'], []]) {
    const result = spawnSync(process.execPath, ['tools/cloud-icons-setup.mjs', ...args], {
      cwd: resolve('.'), env: { ...process.env, AISLIDE_ICON_PACK_ROOT: destination },
      encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], timeout: 15000,
    });
    assert.equal(result.status, args.length ? 0 : 1, result.stderr);
    assert.match(result.stdout, /Microsoft Azure and Entra/);
    assert.match(result.stdout, /Amazon Web Services/);
    assert.match(result.stdout, /Google Cloud/);
    assert.match(result.stdout, /redistribution/);
  }
  assert.deepEqual(await readdir(directory), []);
});

test('public metadata preserves complete coverage, PNG provenance and searchable full names', async () => {
  const bytes = await readFile(catalogPath);
  const catalog = verifyCatalog(bytes);
  const counts = {};
  for (const icon of catalog.icons) counts[icon.provider] = (counts[icon.provider] ?? 0) + 1;
  assert.deepEqual(counts, { aws: 808, azure: 645, gcp: 45 });
  assert.equal(catalog.icons.filter(icon => icon.kind === 'entra-supplement').length, 7);
  assert.equal(catalog.icons.filter(icon => icon.source.entry.endsWith('.png')).length, 853);
  for (const icon of catalog.icons.filter(item => item.source.entry.endsWith('.png') && item.id !== 'gcp/core-product/looker')) assert.equal(icon.png_sha256, icon.source.sha256);
  const looker = catalog.icons.find(icon => icon.id === 'gcp/core-product/looker');
  assert.notEqual(looker.png_sha256, looker.source.sha256);
  assert.equal(looker.width, 256);
  assert.equal(looker.height, 256);
  assert.equal(catalog.icons.find(icon => icon.id === 'azure/service-resource/10076-icon-service-application-gateways').name, 'Azure Application Gateway');
  assert.equal(catalog.icons.find(icon => icon.id === 'azure/entra/microsoft-entra-id').name, 'Microsoft Entra ID');
  const networking = catalog.icons.find(icon => icon.id === 'gcp/category/networking');
  assert.ok(networking.aliases.includes('Google Cloud VPC') || networking.aliases.some(alias => /VPC/.test(alias)));
  assert.ok(networking.aliases.some(alias => /Load Balanc/i.test(alias)));
  assert.match(catalog.icons.find(icon => /vertex-ai$/.test(icon.id)).name, /Vertex AI/);
  assert.doesNotMatch(bytes.toString('utf8'), /C:\\|OneDrive|\.artifacts|data:image|<svg|base64|localPath/);
  assert.equal(catalog.providers.flatMap(provider => provider.archives).reduce((total, archive) => total + archive.bytes, 0), 23842521);
});

test('catalog rejects excess counts, unsafe paths, duplicates, foreign URLs and malformed dimensions', async () => {
  const catalog = await readCatalog();
  const mutations = [
    candidate => candidate.icons.push(candidate.icons[0]),
    candidate => { candidate.icons[1].id = candidate.icons[0].id; },
    candidate => { candidate.icons[0].source.entry = '../escape.png'; },
    candidate => { candidate.icons[0].source.entry = 'C:/escape.png'; },
    candidate => { candidate.icons[0].source.entry = 'bad\\file.png'; },
    candidate => { candidate.icons[0].png_sha256 = '../escape'; },
    candidate => { candidate.icons[0].width = 4097; },
    candidate => { candidate.icons[0].png_bytes = 1048577; },
    candidate => { candidate.icons[0].localPath = 'private'; },
    candidate => { candidate.providers[0].archives[0].url = 'https://example.invalid/icons.zip'; },
  ];
  for (const mutate of mutations) {
    const changed = structuredClone(catalog);
    mutate(changed);
    assert.throws(() => verifyCatalog(Buffer.from(JSON.stringify(changed))));
  }
  assert.throws(() => verifyCatalog(Buffer.concat([Buffer.from([239, 187, 191]), Buffer.from(JSON.stringify(catalog))])), /without BOM/);
});

test('PNG preparation preserves official bytes and rejects hash, magic, size, dimension and alpha failures', async () => {
  const bytes = await png();
  const prepared = await prepareIconPng(imageSource(bytes), bytes);
  assert.deepEqual(prepared.bytes, bytes);
  assert.equal(prepared.png_sha256, hash(bytes));
  assert.equal(prepared.width, 24);
  assert.equal(prepared.height, 12);
  const large = await png(1024, 512);
  const resized = await prepareIconPng(imageSource(large), large);
  assert.equal(resized.width, 256);
  assert.equal(resized.height, 128);
  assert.notEqual(resized.png_sha256, hash(large));
  await assert.rejects(prepareIconPng(imageSource(bytes), Buffer.from('changed')), /SHA-256/);
  const badMagic = Buffer.from('not-a-png');
  await assert.rejects(prepareIconPng(imageSource(badMagic), badMagic), /PNG magic/);
  await assert.rejects(inspectPng(Buffer.alloc(1048577)), /byte limit/);
  await assert.rejects(inspectPng(await png(513, 1)), /dimensions/);
  await assert.rejects(inspectPng(await png(8, 8, 0)), /Blank/);
  await assert.rejects(prepareIconPng({ ...imageSource(bytes), ...prepared, png_sha256: '0'.repeat(64) }, bytes), /pin mismatch/);
  await assert.rejects(prepareIconPng(imageSource(bytes, '../../escape.png'), bytes), /entry path/);
});

test('pinned SVG path rejects executable, external and entity-based inputs before decoding', async () => {
  const wrap = content => `<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18">${content}</svg>`;
  const malicious = [
    wrap('<script>alert(1)</script>'),
    wrap('<rect width="18" height="18" onclick="alert(1)"/>'),
    wrap('<image href="file:///C:/private.png"/>'),
    wrap('<use href="#other"/>'),
    wrap('<foreignObject/>'),
    wrap('<style>@import "https://example.invalid/test.css";</style>'),
    wrap('<rect fill="url(https://example.invalid/fill.svg)"/>'),
    wrap('<rect fill="url(&#104;ttps://example.invalid/fill.svg)"/>'),
    '<!DOCTYPE svg [<!ENTITY secret SYSTEM "file:///C:/private">]>' + wrap('&secret;'),
  ];
  for (const source of malicious) {
    const bytes = Buffer.from(source);
    await assert.rejects(prepareIconPng(imageSource(bytes, 'synthetic.svg'), bytes), /Active or external/);
  }
});

test('download rejects unapproved URLs before fetch and enforces no redirects, status, size and hash', async () => {
  const archive = (await readCatalog()).providers[0].archives[0];
  await assert.rejects(downloadArchive({ ...archive, url: 'https://example.invalid/archive.zip' }, noNetwork), /archive pin/);
  const responses = [
    new Response(null, { status: 302, headers: { location: 'https://example.invalid/archive.zip' } }),
    new Response(null, { status: 404 }),
    new Response(Buffer.alloc(1), { headers: { 'content-length': '1' } }),
    new Response(Buffer.alloc(archive.bytes + 1)),
    new Response(Buffer.alloc(archive.bytes)),
  ];
  for (const response of responses) {
    await assert.rejects(downloadArchive(archive, async (url, options) => {
      assert.equal(url, archive.url);
      assert.equal(options.redirect, 'error');
      assert.equal(options.credentials, 'omit');
      assert.ok(options.signal instanceof AbortSignal);
      return response;
    }), /response rejected|size|SHA-256/);
  }
  await assert.rejects(downloadArchive(archive, async () => ({
    status: 200, redirected: true, url: 'https://example.invalid/archive.zip', body: { cancel: async () => {} },
  })), /redirects/);
});

test('archive authorization rejects modified source bytes before ZIP parsing', async () => {
  const catalog = await readCatalog();
  const archive = catalog.providers[0].archives[0];
  assert.throws(() => readArchiveEntries(archive, Buffer.from('PK malformed traversal duplicate zip'), catalog.icons), /before ZIP decoding/);
});

test('existing incompatible consent and corrupt asset collisions are never overwritten', async context => {
  const directory = await temporary(context);
  const first = join(directory, 'consent-collision');
  await mkdir(first);
  await writeFile(join(first, 'consent.json'), '{"unrelated":true}');
  await assert.rejects(installIconPacks({ acceptVendorTerms: true, packDirectory: first, fetch: noNetwork }), /refusing to overwrite/);
  assert.equal(await readFile(join(first, 'consent.json'), 'utf8'), '{"unrelated":true}');
  const second = join(directory, 'asset-collision');
  const icon = (await readCatalog()).icons[0];
  await mkdir(join(second, icon.provider), { recursive: true });
  const path = join(second, icon.provider, `${icon.png_sha256}.png`);
  await writeFile(path, 'unrelated-user-content');
  await assert.rejects(installIconPacks({ acceptVendorTerms: true, packDirectory: second, fetch: noNetwork }), /refusing to overwrite/);
  assert.equal(await readFile(path, 'utf8'), 'unrelated-user-content');
  assert.deepEqual(await readdir(second), [icon.provider]);
});

test('setup rejects directory junctions and symlinks without writing through them', async context => {
  const directory = await temporary(context);
  const target = join(directory, 'target');
  const redirected = join(directory, 'redirected');
  await mkdir(target);
  await symlink(target, redirected, process.platform === 'win32' ? 'junction' : 'dir');
  for (const packDirectory of [redirected, join(redirected, 'not-created')]) {
    await assert.rejects(installIconPacks({ acceptVendorTerms: true, packDirectory, fetch: noNetwork }), /Symlink|junction|Redirected/);
  }
  assert.deepEqual(await readdir(target), []);
});

test('a damaged cached archive fails closed without fallback fetch or destination writes', async context => {
  const directory = await temporary(context);
  const cache = join(directory, 'cache');
  const destination = join(directory, 'pack');
  await mkdir(cache);
  const archive = (await readCatalog()).providers[0].archives[0];
  await writeFile(join(cache, archive.id), Buffer.alloc(archive.bytes));
  await assert.rejects(installIconPacks({ acceptVendorTerms: true, packDirectory: destination, cacheDirectory: cache, fetch: noNetwork }), /SHA-256/);
  assert.deepEqual(await readdir(directory), ['cache']);
});

test('all 1498 pinned official icons install nonblank, consent is final, and offline reinstall is immutable', {
  skip: !process.env.AISLIDE_CLOUD_ICON_TEST_CACHE,
  timeout: 180000,
}, async context => {
  const directory = await temporary(context);
  const packDirectory = join(directory, 'pack');
  const catalogBytes = await readFile(catalogPath);
  const catalog = verifyCatalog(catalogBytes);
  const cacheDirectory = process.env.AISLIDE_CLOUD_ICON_TEST_CACHE;
  const result = await installIconPacks({ acceptVendorTerms: true, packDirectory, cacheDirectory, fetch: noNetwork });
  assert.equal(result.icons, 1498);
  assert.equal(result.catalog_sha256, hash(catalogBytes));
  assert.equal(result.ready, true);
  const unique = new Set(catalog.icons.map(icon => `${icon.provider}/${icon.png_sha256}.png`));
  assert.equal(result.files, unique.size);
  assert.equal(result.written, unique.size);
  const consentPath = join(packDirectory, 'consent.json');
  const consentBytes = await readFile(consentPath);
  const consentStat = await stat(consentPath);
  assert.deepEqual(JSON.parse(consentBytes), { version: 1, catalog_sha256: hash(catalogBytes), accepted_vendor_terms: true });
  const times = new Map();
  for (const icon of catalog.icons) {
    const path = join(packDirectory, icon.provider, `${icon.png_sha256}.png`);
    const bytes = await readFile(path);
    const dimensions = await inspectPng(bytes);
    assert.equal(hash(bytes), icon.png_sha256, icon.id);
    assert.equal(bytes.length, icon.png_bytes, icon.id);
    assert.equal(dimensions.width, icon.width, icon.id);
    assert.equal(dimensions.height, icon.height, icon.id);
    assert.ok(dimensions.paintedPixels > 0, icon.id);
    times.set(path, (await stat(path)).mtimeMs);
  }
  for (const provider of catalog.providers) {
    const entries = await readdir(join(packDirectory, provider.id));
    assert.equal(entries.length, [...unique].filter(path => path.startsWith(`${provider.id}/`)).length);
    assert.ok(entries.every(name => /^[a-f0-9]{64}\.png$/.test(name)));
  }
  assert.deepEqual((await readdir(packDirectory)).sort(), ['aws', 'azure', 'consent.json', 'gcp']);
  const repeated = await installIconPacks({ acceptVendorTerms: true, packDirectory, fetch: noNetwork });
  assert.equal(repeated.written, 0);
  assert.deepEqual(await readFile(consentPath), consentBytes);
  assert.equal((await stat(consentPath)).mtimeMs, consentStat.mtimeMs);
  for (const [path, time] of times) assert.equal((await stat(path)).mtimeMs, time);
});
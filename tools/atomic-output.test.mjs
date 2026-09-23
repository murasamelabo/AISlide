import test from 'node:test';
import assert from 'node:assert/strict';
import { publishNewFile } from './atomic-output.mjs';
import * as publisher from './atomic-output.mjs';
import { basename, join, resolve } from 'node:path';
import * as filesystem from 'node:fs/promises';
import { tmpdir } from 'node:os';

test('failed write still closes and removes temporary output', async () => {
  const actions = [];
  const io = {
    open: async () => ({ writeFile: async () => { throw new Error('disk full'); }, sync: async () => {}, close: async () => { actions.push('close'); } }),
    link: async () => { actions.push('link'); },
    unlink: async () => { actions.push('unlink'); },
  };
  await assert.rejects(publishNewFile('temp', 'output', Buffer.from('data'), undefined, io), /disk full/);
  assert.deepEqual(actions, ['close', 'unlink']);
});

test('collision never removes an existing destination', async () => {
  const removed = [];
  const io = {
    open: async () => ({ writeFile: async () => {}, sync: async () => {}, close: async () => {} }),
    link: async () => { throw new Error('already exists'); },
    unlink: async (path) => { removed.push(path); },
  };
  await assert.rejects(publishNewFile('temp', 'output', Buffer.from('data'), undefined, io), /already exists/);
  assert.deepEqual(removed, ['temp']);
});

function bundleFixture(directory = resolve('bundle-output')) {
  const items = [
    { filename: 'report.pptx', bytes: Buffer.from('presentation') },
    { filename: 'report.pdf', bytes: Buffer.from('preview') },
    { filename: 'report.manifest.json', bytes: Buffer.from('{}') },
  ];
  const actions = [];
  const files = new Map();
  const io = {
    lstat: async (path) => {
      actions.push(['lstat', path]);
      if (path === directory) return { isDirectory: () => true, isSymbolicLink: () => false };
      if (files.has(path)) return { isDirectory: () => false, isSymbolicLink: () => false };
      throw Object.assign(new Error('missing'), { code: 'ENOENT' });
    },
    realpath: async (path) => { actions.push(['realpath', path]); return path; },
    open: async (path, flags, mode) => {
      actions.push(['open', path, flags, mode]);
      assert.equal(flags, 'wx');
      assert.equal(mode, 0o600);
      assert.equal(files.has(path), false);
      files.set(path, Buffer.alloc(0));
      return {
        writeFile: async (bytes) => { actions.push(['write', path]); files.set(path, Buffer.from(bytes)); },
        datasync: async () => { actions.push(['datasync', path]); },
        close: async () => { actions.push(['close', path]); },
      };
    },
    link: async (temporary, destination) => {
      actions.push(['link', temporary, destination]);
      if (files.has(destination)) throw Object.assign(new Error('already exists'), { code: 'EEXIST' });
      assert.equal(files.has(temporary), true);
      files.set(destination, files.get(temporary));
    },
    unlink: async (path) => { actions.push(['unlink', path]); assert.equal(files.delete(path), true); },
  };
  return { directory, items, actions, files, io };
}

test('bundle stages all files before publishing in order with the manifest last', async () => {
  assert.equal(typeof publisher.publishNewBundle, 'function');
  const { directory, items, actions, files, io } = bundleFixture();
  const paths = items.map(({ filename }) => join(directory, filename));
  const result = await publisher.publishNewBundle(directory, items, undefined, io);
  assert.deepEqual(result, { paths, multi_file_atomic: false });
  assert.deepEqual(actions.filter(([action]) => action === 'link').map((action) => action[2]), paths);
  const firstOpen = actions.findIndex(([action]) => action === 'open');
  assert.deepEqual(actions.slice(0, firstOpen).filter(([action, path]) => action === 'lstat' && path !== directory).map((action) => action[1]), paths);
  const firstLink = actions.findIndex(([action]) => action === 'link');
  assert.equal(actions.slice(0, firstLink).filter(([action]) => action === 'datasync').length, items.length);
  assert.equal(actions.slice(0, firstLink).filter(([action]) => action === 'close').length, items.length);
  const opens = actions.filter(([action]) => action === 'open');
  assert.equal(new Set(opens.map((action) => action[1])).size, items.length);
  for (const [, temporary] of opens) assert.match(basename(temporary), /^\.aislide-[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}\.tmp$/);
  assert.equal(actions.filter(([action]) => action === 'realpath').length, 2);
  assert.equal(actions.filter(([action, path]) => action === 'lstat' && path === directory).length, 2);
  assert.deepEqual([...files.keys()], paths);
  for (const [index, path] of paths.entries()) assert.deepEqual(files.get(path), items[index].bytes);
});

function assertBundleFailure(error, published, pending) {
  assert.ok(error instanceof publisher.BundlePublicationError);
  assert.equal(error.code, 'BUNDLE_PUBLICATION_FAILED');
  assert.deepEqual(error.published_paths, published);
  assert.deepEqual(error.pending_filenames, pending);
  assert.ok(Array.isArray(error.cleanup_errors));
  assert.match(error.message, /not crash-atomic/i);
  assert.match(error.message, /never deleted/i);
  return true;
}

test('bundle existing manifest prevents all staging and publication', async () => {
  const { directory, items, actions, files, io } = bundleFixture();
  const manifest = join(directory, items.at(-1).filename);
  const existing = Buffer.from('existing manifest');
  files.set(manifest, existing);
  await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
    assertBundleFailure(error, [], items.map(({ filename }) => filename));
    assert.deepEqual(error.cleanup_errors, []);
    return true;
  });
  assert.equal(actions.some(([action]) => ['open', 'link', 'unlink'].includes(action)), false);
  assert.deepEqual([...files], [[manifest, existing]]);
});

for (const operation of ['writeFile', 'datasync', 'close']) {
  test(`bundle ${operation} failure closes handles and leaves no final outputs`, async () => {
    const { directory, items, actions, files, io } = bundleFixture();
    const primary = new Error(`${operation} failed`);
    const open = io.open;
    let opened = 0;
    io.open = async (...args) => {
      const file = await open(...args);
      opened += 1;
      if (opened === 2) {
        const original = file[operation];
        file[operation] = async (...values) => { await original(...values); throw primary; };
      }
      return file;
    };
    await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
      assertBundleFailure(error, [], items.map(({ filename }) => filename));
      assert.equal(error.cause, primary);
      return true;
    });
    assert.equal(actions.filter(([action]) => action === 'close').length, 2);
    assert.equal(actions.some(([action]) => action === 'link'), false);
    assert.equal(files.size, 0);
  });
}

test('bundle pre-cancellation makes no filesystem calls', async () => {
  const { directory, items, actions, io } = bundleFixture();
  await assert.rejects(publisher.publishNewBundle(directory, items, AbortSignal.abort(), io), (error) => {
    assertBundleFailure(error, [], items.map(({ filename }) => filename));
    assert.match(error.cause.message, /cancel/i);
    return true;
  });
  assert.deepEqual(actions, []);
});

for (const boundary of ['preflight', 'open', 'writeFile', 'datasync', 'close', 'recheck', 'link', 'last-link']) {
  test(`bundle cancellation during ${boundary} stops subsequent side effects but cleans owned temps`, async () => {
    const { directory, items, actions, files, io } = bundleFixture();
    const controller = new AbortController();
    if (boundary === 'preflight') {
      const lstat = io.lstat;
      io.lstat = async (path) => {
        try { return await lstat(path); }
        finally { if (path === join(directory, items.at(-1).filename)) controller.abort(); }
      };
    } else if (boundary === 'recheck') {
      const realpath = io.realpath;
      let checked = 0;
      io.realpath = async (path) => {
        const result = await realpath(path);
        checked += 1;
        if (checked === 2) controller.abort();
        return result;
      };
    } else if (boundary === 'link' || boundary === 'last-link') {
      const link = io.link;
      let linked = 0;
      io.link = async (...args) => {
        await link(...args);
        linked += 1;
        if (linked === (boundary === 'link' ? 1 : items.length)) controller.abort();
      };
    } else {
      const open = io.open;
      io.open = async (...args) => {
        const file = await open(...args);
        if (boundary === 'open') controller.abort();
        else {
          const original = file[boundary];
          file[boundary] = async (...values) => { await original(...values); controller.abort(); };
        }
        return file;
      };
    }
    const count = boundary === 'link' ? 1 : boundary === 'last-link' ? items.length : 0;
    const published = items.slice(0, count).map(({ filename }) => join(directory, filename));
    await assert.rejects(publisher.publishNewBundle(directory, items, controller.signal, io), (error) => {
      assertBundleFailure(error, published, items.slice(count).map(({ filename }) => filename));
      assert.match(error.cause.message, /cancel/i);
      return true;
    });
    assert.deepEqual([...files.keys()], published);
    const staged = ['preflight'].includes(boundary) ? 0 : ['recheck', 'link', 'last-link'].includes(boundary) ? items.length : 1;
    assert.equal(actions.filter(([action]) => action === 'open').length, staged);
    assert.equal(actions.filter(([action]) => action === 'close').length, staged);
    if (boundary === 'open') assert.equal(actions.some(([action]) => action === 'write'), false);
    if (boundary === 'writeFile') assert.equal(actions.some(([action]) => action === 'datasync'), false);
  });
}

for (const raceIndex of [1, 2]) {
  test(`bundle racing destination ${raceIndex} retains partial files and the unrelated collision`, async () => {
    const { directory, items, actions, files, io } = bundleFixture();
    const link = io.link;
    const collision = join(directory, items[raceIndex].filename);
    const unrelated = Buffer.from('unrelated file');
    io.link = async (temporary, destination) => {
      if (destination === collision) files.set(destination, unrelated);
      await link(temporary, destination);
    };
    const published = items.slice(0, raceIndex).map(({ filename }) => join(directory, filename));
    await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
      assertBundleFailure(error, published, items.slice(raceIndex).map(({ filename }) => filename));
      assert.equal(error.cause.code, 'EEXIST');
      return true;
    });
    assert.deepEqual([...files.keys()], [...published, collision]);
    assert.equal(files.get(collision), unrelated);
    for (const [, path] of actions.filter(([action]) => action === 'unlink')) assert.match(basename(path), /^\.aislide-.*\.tmp$/);
  });
}

test('bundle temp creation collision never removes the unowned temp', async () => {
  const { directory, items, actions, files, io } = bundleFixture();
  const open = io.open;
  const primary = Object.assign(new Error('temp already exists'), { code: 'EEXIST' });
  let unowned;
  let opened = 0;
  io.open = async (...args) => {
    opened += 1;
    if (opened === 2) {
      unowned = args[0];
      files.set(unowned, Buffer.from('unowned'));
      throw primary;
    }
    return open(...args);
  };
  await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
    assertBundleFailure(error, [], items.map(({ filename }) => filename));
    assert.equal(error.cause, primary);
    return true;
  });
  assert.deepEqual([...files.keys()], [unowned]);
  assert.equal(actions.some(([action, path]) => action === 'unlink' && path === unowned), false);
});

for (const withPrimary of [true, false]) {
  test(`bundle cleanup failure reports actual paths and ${withPrimary ? 'preserves primary failure' : 'does not report success'}`, async () => {
    const { directory, items, actions, files, io } = bundleFixture();
    const primary = new Error('primary link failure');
    const cleanup = Object.assign(new Error('secret cleanup details'), { code: 'EPERM' });
    if (withPrimary) {
      const link = io.link;
      io.link = async (temporary, destination) => {
        if (destination === join(directory, items[1].filename)) throw primary;
        await link(temporary, destination);
      };
    }
    const unlink = io.unlink;
    let retained;
    io.unlink = async (path) => {
      if (!retained) { retained = path; actions.push(['unlink', path]); throw cleanup; }
      await unlink(path);
    };
    const count = withPrimary ? 1 : items.length;
    const published = items.slice(0, count).map(({ filename }) => join(directory, filename));
    await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
      assertBundleFailure(error, published, items.slice(count).map(({ filename }) => filename));
      assert.equal(error.cause, withPrimary ? primary : cleanup);
      assert.equal(error.cleanup_errors.length, 1);
      assert.equal(typeof error.cleanup_errors[0], 'string');
      assert.doesNotMatch(error.cleanup_errors[0], /secret cleanup details/);
      assert.doesNotMatch(error.message, /secret cleanup details/);
      return true;
    });
    assert.equal(actions.filter(([action]) => action === 'unlink').length, items.length);
    assert.deepEqual([...files.keys()].sort(), [retained, ...published].sort());
  });
}

test('bundle write error survives close and unlink failures without leaking cleanup messages', async () => {
  const { directory, items, io } = bundleFixture();
  const primary = new Error('primary write failure');
  const open = io.open;
  io.open = async (...args) => {
    const file = await open(...args);
    file.writeFile = async () => { throw primary; };
    file.close = async () => { throw new Error('secret close details'); };
    return file;
  };
  io.unlink = async () => { throw new Error('secret unlink details'); };
  await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
    assertBundleFailure(error, [], items.map(({ filename }) => filename));
    assert.equal(error.cause, primary);
    assert.equal(error.cleanup_errors.length, 2);
    assert.doesNotMatch(error.cleanup_errors.join(' '), /secret/);
    return true;
  });
});

for (const invalidRoot of ['symlink', 'not-directory', 'realpath-mismatch']) {
  for (const late of [false, true]) {
    test(`bundle rejects ${invalidRoot} ${late ? 'before publication' : 'immediately'}`, async () => {
      const { directory, items, actions, files, io } = bundleFixture();
      const lstat = io.lstat;
      const realpath = io.realpath;
      let checked = 0;
      io.lstat = async (path) => {
        const result = await lstat(path);
        if (path === directory) {
          checked += 1;
          if (checked === (late ? 2 : 1)) {
            if (invalidRoot === 'symlink') return { isDirectory: () => true, isSymbolicLink: () => true };
            if (invalidRoot === 'not-directory') return { isDirectory: () => false, isSymbolicLink: () => false };
          }
        }
        return result;
      };
      io.realpath = async (path) => {
        await realpath(path);
        return invalidRoot === 'realpath-mismatch' && checked === (late ? 2 : 1) ? join(path, 'different') : path;
      };
      await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => assertBundleFailure(error, [], items.map(({ filename }) => filename)));
      assert.equal(actions.some(([action]) => action === 'link'), false);
      assert.equal(actions.filter(([action]) => action === 'open').length, late ? items.length : 0);
      if (!late && invalidRoot === 'symlink') assert.deepEqual(actions, [['lstat', directory]]);
      assert.equal(files.size, 0);
    });
  }
}

test('bundle preflight errors other than ENOENT fail closed', async () => {
  const { directory, items, actions, io } = bundleFixture();
  const lstat = io.lstat;
  const primary = Object.assign(new Error('access denied'), { code: 'EACCES' });
  io.lstat = async (path) => path === directory ? lstat(path) : Promise.reject(primary);
  await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
    assert.equal(error.cause, primary);
    return assertBundleFailure(error, [], items.map(({ filename }) => filename));
  });
  assert.equal(actions.some(([action]) => action === 'open'), false);
});

test('bundle rejects invalid names, duplicates, item types, counts and byte limits before filesystem access', async (context) => {
  const reserved = ['CON', 'PRN', 'AUX', 'NUL', ...Array.from({ length: 9 }, (_, index) => `COM${index + 1}`), ...Array.from({ length: 9 }, (_, index) => `LPT${index + 1}`)];
  const names = ['', '../escape.txt', '/absolute.txt', 'C:\\absolute.txt', 'C:drive.txt', '\\\\server\\file.txt', 'dir/file.txt', 'dir\\file.txt', 'name:stream.txt', '.hidden.txt', 'double..dot.txt', 'trailing.txt.', 'trailing.txt ', 'no-extension', 'bad.ext-', 'bad.ext_', 'with space.txt', 'bad\u0000.txt', 'bad\n.txt', 'bad\u00e9.txt', `${'a'.repeat(117)}.txt`, ...reserved.flatMap((stem) => [`${stem}.txt`, `${stem.toLowerCase()}.extra.txt`])];
  const manifest = { filename: 'report.manifest.json', bytes: Buffer.from('{}') };
  const cases = names.map((filename) => ({ label: `name ${JSON.stringify(filename)}`, items: [{ filename, bytes: Buffer.from('data') }, manifest] }));
  cases.push(
    { label: 'duplicate', items: [{ filename: 'report.txt', bytes: Buffer.alloc(0) }, { filename: 'REPORT.TXT', bytes: Buffer.alloc(0) }, manifest] },
    { label: 'manifest is not last', items: [manifest, { filename: 'report.txt', bytes: Buffer.alloc(0) }] },
    { label: 'wrong manifest suffix', items: [{ filename: 'report.Manifest.json', bytes: Buffer.alloc(0) }] },
    { label: 'empty items', items: [] },
    { label: 'not an array', items: {} },
    { label: 'null items', items: null },
    { label: 'too many files', items: [...Array.from({ length: 13 }, (_, index) => ({ filename: `file${index}.txt`, bytes: Buffer.alloc(0) })), manifest] },
    { label: 'null item', items: [null, manifest] },
    { label: 'missing filename', items: [{ bytes: Buffer.alloc(0) }, manifest] },
    { label: 'non-string filename', items: [{ filename: 12, bytes: Buffer.alloc(0) }, manifest] },
    { label: 'missing bytes', items: [{ filename: 'report.txt' }, manifest] },
    { label: 'string bytes', items: [{ filename: 'report.txt', bytes: 'data' }, manifest] },
    { label: 'typed array bytes', items: [{ filename: 'report.txt', bytes: new Uint8Array(1) }, manifest] },
    { label: 'relative directory', directory: 'relative', items: [manifest] },
    { label: 'noncanonical directory', directory: `${resolve('bundle-output')}/../bundle-output`, items: [manifest] },
    { label: 'non-string directory', directory: 123, items: [manifest] },
    { label: 'null-byte directory', directory: `${resolve('bundle-output')}\u0000`, items: [manifest] },
  );
  for (const entry of cases) {
    await context.test(entry.label, async () => {
      const { directory, actions, io } = bundleFixture();
      await assert.rejects(publisher.publishNewBundle(entry.directory ?? directory, entry.items, undefined, io), (error) => {
        assert.equal(error.code, 'BUNDLE_PUBLICATION_FAILED');
        assert.deepEqual(error.published_paths, []);
        assert.deepEqual(error.cleanup_errors, []);
        return true;
      });
      assert.deepEqual(actions, []);
    });
  }
  const limit = 32 * 1024 * 1024;
  const large = Buffer.alloc(limit + 1);
  for (const [label, items] of [
    ['single file exceeds limit', [{ filename: 'report.manifest.json', bytes: large }]],
    ['combined bytes exceed limit', [{ filename: 'report.txt', bytes: large.subarray(0, limit - 1) }, manifest]],
  ]) {
    await context.test(label, async () => {
      const { directory, actions, io } = bundleFixture();
      await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => assertBundleFailure(error, [], items.map(({ filename }) => filename)));
      assert.deepEqual(actions, []);
    });
  }
});

test('bundle accepts the exact byte limit, maximum file count, maximum filename and nonreserved stems', async () => {
  const { directory, files, io } = bundleFixture();
  const names = [`${'a'.repeat(116)}.txt`, 'COM0.txt', 'COM10.txt', 'LPT0.txt', 'LPT10.txt', 'CONSOLE.txt', 'CON-extra.txt', 'PRN_extra.txt', 'AUX-extra.txt', 'NUL-extra.txt', 'archive.tar.gz', 'normal.123', 'report.manifest.json'];
  const items = names.map((filename) => ({ filename, bytes: Buffer.alloc(0) }));
  items[0].bytes = Buffer.alloc(32 * 1024 * 1024 - 2);
  items.at(-1).bytes = Buffer.from('{}');
  const result = await publisher.publishNewBundle(directory, items, undefined, io);
  assert.deepEqual(result, { paths: names.map((filename) => join(directory, filename)), multi_file_atomic: false });
  assert.equal(files.size, 13);
});

test('bundle permits a single manifest file', async () => {
  const { directory, items, io } = bundleFixture();
  assert.deepEqual(await publisher.publishNewBundle(directory, items.slice(-1), undefined, io), {
    paths: [join(directory, items.at(-1).filename)], multi_file_atomic: false,
  });
});

test('bundle cancellation during cleanup reports all published files', async () => {
  const { directory, items, files, io } = bundleFixture();
  const controller = new AbortController();
  const unlink = io.unlink;
  io.unlink = async (path) => { await unlink(path); controller.abort(); };
  const paths = items.map(({ filename }) => join(directory, filename));
  await assert.rejects(publisher.publishNewBundle(directory, items, controller.signal, io), (error) => {
    assertBundleFailure(error, paths, []);
    assert.match(error.cause.message, /cancel/i);
    assert.deepEqual(error.cleanup_errors, []);
    return true;
  });
  assert.deepEqual([...files.keys()], paths);
});

test('bundle cleanup diagnostics retain complete temp names for a drive root', async () => {
  const { directory, items, io } = bundleFixture(resolve('/'));
  const retained = [];
  io.unlink = async (path) => { retained.push(path); throw new Error('secret'); };
  await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => {
    assertBundleFailure(error, items.map(({ filename }) => join(directory, filename)), []);
    assert.equal(error.cleanup_errors.length, items.length);
    for (const [index, path] of retained.entries()) assert.ok(error.cleanup_errors[index].endsWith(basename(path)));
    return true;
  });
});

test('bundle rejects trailing line breaks before filesystem access', async () => {
  for (const suffix of ['\n', '\r', '\r\n']) {
    const { directory, items, actions, io } = bundleFixture();
    items[0].filename += suffix;
    await assert.rejects(publisher.publishNewBundle(directory, items, undefined, io), (error) => assertBundleFailure(error, [], items.map(({ filename }) => filename)));
    assert.deepEqual(actions, []);
  }
});

for (const scenario of ['success', 'existing-manifest', 'racing-manifest']) {
  test(`bundle real filesystem ${scenario} uses exclusive hardlinks and leaves no temps`, async (context) => {
    const temporaryRoot = await filesystem.mkdtemp(join(tmpdir(), 'aislide-bundle-test-'));
    context.after(() => filesystem.rm(temporaryRoot, { recursive: true, force: true }));
    const directory = await filesystem.realpath(temporaryRoot);
    const { items } = bundleFixture(directory);
    const paths = items.map(({ filename }) => join(directory, filename));
    const manifest = paths.at(-1);
    const unrelated = Buffer.from('unrelated manifest');
    if (scenario === 'existing-manifest') await filesystem.writeFile(manifest, unrelated, { flag: 'wx' });
    const io = {
      ...filesystem,
      link: async (temporary, destination) => {
        if (destination === manifest) await filesystem.writeFile(manifest, unrelated, { flag: 'wx' });
        await filesystem.link(temporary, destination);
      },
    };
    if (scenario === 'success') {
      assert.deepEqual(await publisher.publishNewBundle(directory, items), { paths, multi_file_atomic: false });
      for (const [index, path] of paths.entries()) assert.deepEqual(await filesystem.readFile(path), items[index].bytes);
    } else {
      const published = scenario === 'existing-manifest' ? [] : paths.slice(0, -1);
      await assert.rejects(publisher.publishNewBundle(directory, items, undefined, scenario === 'racing-manifest' ? io : filesystem), (error) => {
        assertBundleFailure(error, published, items.slice(published.length).map(({ filename }) => filename));
        assert.deepEqual(error.cleanup_errors, []);
        return true;
      });
      assert.deepEqual(await filesystem.readFile(manifest), unrelated);
      for (const [index, path] of published.entries()) assert.deepEqual(await filesystem.readFile(path), items[index].bytes);
    }
    assert.deepEqual((await filesystem.readdir(directory)).sort(), (scenario === 'existing-manifest' ? [items.at(-1).filename] : items.map(({ filename }) => filename)).sort());
  });
}
import * as filesystem from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { basename, isAbsolute, join, resolve } from 'node:path';

export async function publishNewFile(temporary, destination, bytes, signal, io = filesystem) {
  const file = await io.open(temporary, 'wx', 0o600);
  try {
    try {
      await file.writeFile(bytes);
      await file.sync();
    } finally { await file.close(); }
    if (signal?.aborted) throw new Error('Operation cancelled');
    await io.link(temporary, destination);
  } finally {
    await io.unlink(temporary);
  }
}

export class BundlePublicationError extends Error {
  constructor(cause, publishedPaths, pendingFilenames, cleanupErrors) {
    super('Bundle publication failed. Multi-file publication is not crash-atomic; partial published files are never deleted.', { cause });
    this.name = 'BundlePublicationError';
    this.code = 'BUNDLE_PUBLICATION_FAILED';
    this.published_paths = [...publishedPaths];
    this.pending_filenames = [...pendingFilenames];
    this.cleanup_errors = [...cleanupErrors];
  }
}

export async function publishNewBundle(directory, items, signal, io = filesystem) {
  const paths = [];
  const temporaries = [];
  const cleanupErrors = [];
  let filenames = [];
  let failed = false;
  let primaryError;
  const checkCancellation = () => {
    if (signal?.aborted) throw new Error('Operation cancelled');
  };
  const checkDirectory = async () => {
    checkCancellation();
    const stat = await io.lstat(directory);
    if (stat.isSymbolicLink() || !stat.isDirectory()) throw new Error('Output directory must be a real directory');
    checkCancellation();
    if (await io.realpath(directory) !== directory) throw new Error('Output directory must match its real path');
  };
  try {
    if (!Array.isArray(items) || items.length < 1 || items.length > 13) throw new Error('Bundle requires 1 to 13 files');
    filenames = items.map((item) => item?.filename).filter((filename) => typeof filename === 'string');
    checkCancellation();
    if (typeof directory !== 'string' || directory.includes('\0') || !isAbsolute(directory) || resolve(directory) !== directory) {
      throw new Error('Output directory must be an absolute resolved path');
    }
    const names = new Set();
    const entries = [];
    let totalBytes = 0;
    for (const item of items) {
      if (!item || typeof item !== 'object' || Array.isArray(item)) throw new Error('Invalid bundle item');
      const { filename, bytes } = item;
      if (typeof filename !== 'string'
        || /^[A-Za-z0-9][A-Za-z0-9_.-]{0,119}$/.exec(filename)?.[0] !== filename
        || filename.includes('..')
        || !/\.[A-Za-z0-9]+$/.test(filename)
        || /^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)/i.test(filename)) {
        throw new Error('Invalid bundle filename');
      }
      const key = filename.toLowerCase();
      if (names.has(key)) throw new Error('Duplicate bundle filename');
      names.add(key);
      if (!Buffer.isBuffer(bytes)) throw new Error('Bundle bytes must be a Buffer');
      totalBytes += bytes.length;
      if (totalBytes > 32 * 1024 * 1024) throw new Error('Bundle exceeds 32 MiB');
      entries.push({ filename, bytes, destination: join(directory, filename) });
    }
    if (!entries.at(-1).filename.endsWith('.manifest.json')) throw new Error('The manifest must be the final bundle file');
    await checkDirectory();
    for (const { destination } of entries) {
      checkCancellation();
      try { await io.lstat(destination); }
      catch (error) {
        if (error?.code === 'ENOENT') continue;
        throw error;
      }
      throw new Error('Bundle destination already exists');
    }
    for (const { bytes } of entries) {
      checkCancellation();
      const temporary = join(directory, `.aislide-${randomUUID()}.tmp`);
      const file = await io.open(temporary, 'wx', 0o600);
      temporaries.push(temporary);
      let stageFailed = false;
      try {
        checkCancellation();
        await file.writeFile(bytes);
        checkCancellation();
        await file.datasync();
      } catch (error) {
        stageFailed = true;
        throw error;
      } finally {
        try { await file.close(); }
        catch (error) {
          if (!stageFailed) throw error;
          cleanupErrors.push(`Could not close owned temporary ${basename(temporary)}`);
        }
      }
    }
    await checkDirectory();
    for (const [index, { destination }] of entries.entries()) {
      checkCancellation();
      await io.link(temporaries[index], destination);
      paths.push(destination);
    }
    checkCancellation();
  } catch (error) {
    failed = true;
    primaryError = error;
  } finally {
    for (const temporary of temporaries) {
      try { await io.unlink(temporary); }
      catch (error) {
        cleanupErrors.push(`Could not remove owned temporary ${basename(temporary)}`);
        if (!failed) { failed = true; primaryError = error; }
      }
    }
  }
  if (!failed && signal?.aborted) { failed = true; primaryError = new Error('Operation cancelled'); }
  if (failed) throw new BundlePublicationError(primaryError, paths, filenames.slice(paths.length), cleanupErrors);
  return { paths, multi_file_atomic: false };
}
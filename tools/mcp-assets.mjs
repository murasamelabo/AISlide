import { open, realpath, lstat } from 'node:fs/promises';
import { constants } from 'node:fs';
import { basename, extname, isAbsolute, join, relative, resolve } from 'node:path';
import { createHash, randomUUID } from 'node:crypto';
import { CAPACITY_PROFILES, FONT_LIMITS } from '../packages/client/index.mjs';

const mib = 1048576;
const formats = new Map([
  ['png', ['image/png', mib]], ['jpeg', ['image/jpeg', mib]], ['svg', ['image/svg+xml', 256 * 1024]],
  ['pptx', ['application/vnd.openxmlformats-officedocument.presentationml.presentation', CAPACITY_PROFILES.large.archive_bytes]],
  ['potx', ['application/vnd.openxmlformats-officedocument.presentationml.template', CAPACITY_PROFILES.large.archive_bytes]], ['thmx', ['application/vnd.ms-officetheme', CAPACITY_PROFILES.large.archive_bytes]],
  ['ttf', ['font/ttf', FONT_LIMITS.face_bytes]], ['otf', ['font/otf', FONT_LIMITS.face_bytes]],
  ['csv', ['text/csv', 2 * mib]], ['json', ['application/json', 2 * mib]], ['xlsx', ['application/vnd.openxmlformats-officedocument.spreadsheetml.sheet', 2 * mib]],
  ['markdown', ['text/markdown', 2 * mib]], ['text', ['text/plain', 2 * mib]], ['pdf', ['application/pdf', 2 * mib]],
]);
const extensions = { jpg: 'jpeg', md: 'markdown', txt: 'text' };
const sameFile = (left, right) => left.dev === right.dev && left.ino === right.ino;
const metadata = ({ base64: _base64, ...value }) => ({ ...value });

export class McpAssets {
  #roots;
  #assets = new Map();
  #bytes = 0;
  constructor(roots) { this.#roots = roots; }
  static async create(directories) {
    if (directories.length > 8) throw new Error('At most eight --asset-dir roots');
    const roots = [];
    for (const directory of directories) {
      const path = await realpath(resolve(directory));
      const stat = await lstat(path, { bigint: true });
      if (!stat.isDirectory() || stat.isSymbolicLink()) throw new Error('Asset root must be a local directory');
      roots.push({ path, stat });
    }
    return new McpAssets(roots);
  }
  list() { return { root_count: this.#roots.length, byte_length: this.#bytes, assets: [...this.#assets.values()].map(metadata) }; }
  get(id) {
    const asset = this.#assets.get(id);
    if (!asset) throw new Error('Unknown asset handle; use list_assets');
    return asset;
  }
  close(id) {
    const asset = this.get(id);
    this.#assets.delete(id);
    this.#bytes -= asset.byte_length;
    return { closed: id };
  }
  retain(bytes, format, name, dimensions = {}) { return this.retainMany([{ bytes, format, name, ...dimensions }])[0]; }
  retainMany(inputs) {
    const staged = new Map(this.#assets);
    let totalBytes = this.#bytes;
    const result = inputs.map(({ bytes, format, name, width, height }) => {
      const definition = formats.get(format);
      if (!definition || bytes.length === 0 || bytes.length > definition[1]) throw new Error('Unsupported or oversized asset');
      const hasDimensions = width !== undefined || height !== undefined;
      if (hasDimensions && ![width, height].every(value => Number.isInteger(value) && value > 0 && value <= 4096)) throw new Error('Invalid raster dimensions');
      const dimensions = hasDimensions ? { width, height } : {};
      const sha256 = createHash('sha256').update(bytes).digest('hex');
      const existing = [...staged.values()].find(asset => asset.sha256 === sha256 && asset.format === format);
      if (existing) {
        const retained = Object.freeze({ ...existing, ...dimensions });
        staged.set(retained.asset_id, retained);
        return metadata(retained);
      }
      if (staged.size >= 32 || totalBytes + bytes.length > 64 * mib) throw new Error('Asset store limit reached (32 assets / 64 MiB); close unused assets');
      const asset = Object.freeze({ asset_id: randomUUID(), name, format, mime_type: definition[0], byte_length: bytes.length, sha256, base64: bytes.toString('base64'), ...dimensions });
      staged.set(asset.asset_id, asset);
      totalBytes += bytes.length;
      return metadata(asset);
    });
    this.#assets = staged;
    this.#bytes = totalBytes;
    return result;
  }
  async #inspect(root, segments) {
    const currentRoot = await lstat(root.path, { bigint: true });
    if (!currentRoot.isDirectory() || currentRoot.isSymbolicLink() || !sameFile(root.stat, currentRoot) || await realpath(root.path) !== root.path) throw new Error('Asset root changed');
    let path = root.path;
    let stat;
    for (let index = 0; index < segments.length; index += 1) {
      path = join(path, segments[index]);
      stat = await lstat(path, { bigint: true });
      if (stat.isSymbolicLink() || (index === segments.length - 1 ? !stat.isFile() : !stat.isDirectory())) throw new Error('Asset links and nonregular files are not allowed');
    }
    const resolved = await realpath(path);
    const within = relative(root.path, resolved);
    if (isAbsolute(within) || within === '..' || within.startsWith('../') || within.startsWith('..\\')) throw new Error('Asset path escapes its approved root');
    return { path, stat };
  }
  async registerFile({ path, root: rootIndex = 0 }, signal, inspectRaster) {
    const root = this.#roots[rootIndex];
    if (!root) throw new Error('Reading assets requires an approved --asset-dir root at server startup');
    const segments = path.split(/[\\/]/);
    if (isAbsolute(path) || /[:\x00-\x1f]/.test(path) || segments.some(segment => !segment || segment === '.' || segment === '..' || /[. ]$/.test(segment) || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(segment))) throw new Error('Asset path must be a regular relative filename within its approved root');
    const extension = extname(path).slice(1).toLowerCase();
    const format = Object.hasOwn(extensions, extension) ? extensions[extension] : extension;
    const definition = formats.get(format);
    if (!definition) throw new Error('Unsupported asset extension');
    signal?.throwIfAborted();
    const inspected = await this.#inspect(root, segments);
    if (inspected.stat.size <= 0n || inspected.stat.size > BigInt(definition[1])) throw new Error('Asset exceeds its type size limit or is empty');
    const file = await open(inspected.path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
    try {
      const before = await file.stat({ bigint: true });
      if (!before.isFile() || !sameFile(before, inspected.stat) || before.size !== inspected.stat.size) throw new Error('Asset changed before opening');
      const buffer = Buffer.alloc(Number(before.size) + 1);
      let length = 0;
      while (length < buffer.length) {
        signal?.throwIfAborted();
        const { bytesRead } = await file.read(buffer, length, buffer.length - length, length);
        if (bytesRead === 0) break;
        length += bytesRead;
      }
      const after = await file.stat({ bigint: true });
      const current = await this.#inspect(root, segments);
      if (length !== Number(before.size) || !sameFile(before, after) || !sameFile(before, current.stat) || before.size !== after.size || before.mtimeNs !== after.mtimeNs || before.ctimeNs !== after.ctimeNs || after.size !== current.stat.size || after.mtimeNs !== current.stat.mtimeNs || after.ctimeNs !== current.stat.ctimeNs) throw new Error('Asset changed while reading');
      signal?.throwIfAborted();
      const bytes = buffer.subarray(0, length);
      let dimensions;
      if (format === 'png' || format === 'jpeg') {
        const sha256 = createHash('sha256').update(bytes).digest('hex');
        const existing = [...this.#assets.values()].find(asset => asset.sha256 === sha256 && asset.format === format && asset.width !== undefined && asset.height !== undefined);
        if (existing) return metadata(existing);
        if (typeof inspectRaster !== 'function') throw new Error('Raster inspection is required before file registration');
        const info = await inspectRaster({ base64: bytes.toString('base64'), mime_type: definition[0] }, signal);
        signal?.throwIfAborted();
        if (!info || info.mime_type !== definition[0] || info.sha256 !== sha256 || info.byte_length !== length || ![info.width, info.height].every(value => Number.isInteger(value) && value > 0 && value <= 4096)) throw new Error('Raster inspection dimensions or identity do not match the registered bytes');
        dimensions = { width: info.width, height: info.height };
      }
      return this.retain(bytes, format, basename(path), dimensions);
    } finally { await file.close(); }
  }
}
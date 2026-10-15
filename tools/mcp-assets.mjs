import { open, realpath, lstat } from 'node:fs/promises';
import { constants } from 'node:fs';
import { basename, extname, isAbsolute, join, relative, resolve } from 'node:path';
import { createHash, randomUUID } from 'node:crypto';
import { CAPACITY_PROFILES, FONT_LIMITS } from '../packages/client/index.mjs';

const mib = 1048576;
const assetLimit = 32;
const rawByteLimit = 64 * mib;
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
const sameVersion = (left, right) => sameFile(left, right) && left.size === right.size && left.mtimeNs === right.mtimeNs && left.ctimeNs === right.ctimeNs;
const metadata = ({ base64, ...value }) => ({ ...value, ...(value.width && value.height ? { raster_cost: { encoded_byte_length: base64.length, rgba_byte_length: value.width * value.height * 4, document_total_included: false } } : {}) });

export class McpAssets {
  #roots;
  #assets = new Map();
  #bytes = 0;
  #version = 0;
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
  usage() {
    return { scope: 'process_asset_registry', asset_count: this.#assets.size, asset_limit: assetLimit, remaining_assets: assetLimit - this.#assets.size,
      raw_byte_length: this.#bytes, raw_byte_limit: rawByteLimit, remaining_raw_bytes: rawByteLimit - this.#bytes, document_budgets_included: false };
  }
  list() { return { root_count: this.#roots.length, byte_length: this.#bytes, assets: [...this.#assets.values()].map(metadata), usage: this.usage() }; }
  get(id) {
    const asset = this.#assets.get(id);
    if (!asset) throw new Error('Unknown asset handle; use list_assets');
    return asset;
  }
  close(id) {
    const asset = this.get(id);
    this.#assets.delete(id);
    this.#bytes -= asset.byte_length;
    this.#version += 1;
    return { closed: id, usage: this.usage() };
  }
  retain(bytes, format, name, dimensions = {}) { return this.retainMany([{ bytes, format, name, ...dimensions }])[0]; }
  retainMany(inputs) {
    const state = { assets: new Map(this.#assets), bytes: this.#bytes };
    const result = this.#retain(inputs, state);
    this.#commit(state);
    return result;
  }
  #commit(state) {
    this.#assets = state.assets;
    this.#bytes = state.bytes;
    this.#version += 1;
  }
  #retain(inputs, state) {
    return inputs.map(({ bytes, format, name, width, height }) => {
      const definition = formats.get(format);
      if (!definition || bytes.length === 0 || bytes.length > definition[1]) throw new Error('Unsupported or oversized asset');
      const hasDimensions = width !== undefined || height !== undefined;
      if (hasDimensions && ![width, height].every(value => Number.isInteger(value) && value > 0 && value <= 4096)) throw new Error('Invalid raster dimensions');
      const dimensions = hasDimensions ? { width, height } : {};
      const sha256 = createHash('sha256').update(bytes).digest('hex');
      const existing = [...state.assets.values()].find(asset => asset.sha256 === sha256 && asset.format === format);
      if (existing) {
        const retained = Object.freeze({ ...existing, ...dimensions });
        state.assets.set(retained.asset_id, retained);
        return metadata(retained);
      }
      if (state.assets.size >= assetLimit || state.bytes + bytes.length > rawByteLimit) throw new Error('Asset store limit reached (32 assets / 64 MiB); close unused assets');
      const asset = Object.freeze({ asset_id: randomUUID(), name, format, mime_type: definition[0], byte_length: bytes.length, sha256, base64: bytes.toString('base64'), ...dimensions });
      state.assets.set(asset.asset_id, asset);
      state.bytes += bytes.length;
      return metadata(asset);
    });
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
  async registerFile(input, signal, inspectRaster) {
    const result = await this.registerFiles([input], signal, inspectRaster);
    return { ...result.assets[0], usage: result.usage };
  }
  async prepareRasters(inputs, signal, prepareRaster) {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > assetLimit) throw new Error('Prepare 1-32 registered rasters per batch');
    const version = this.#version;
    const requests = inputs.map(input => {
      if (!input || typeof input !== 'object' || Array.isArray(input) || Object.keys(input).some(key => key !== 'asset_id' && key !== 'params') || typeof input.asset_id !== 'string' || !input.params || typeof input.params !== 'object' || Array.isArray(input.params)) throw new Error('Invalid raster preparation input');
      const asset = this.get(input.asset_id);
      if (!['png', 'jpeg'].includes(asset.format)) throw new Error('Preparation requires a registered PNG/JPEG');
      return { asset, params: structuredClone(input.params) };
    });
    const prepared = [];
    for (const { asset, params } of requests) {
      signal?.throwIfAborted();
      const image = await prepareRaster({ base64: asset.base64, mime_type: asset.mime_type, params }, signal);
      signal?.throwIfAborted();
      if (!image || !['image/png', 'image/jpeg'].includes(image.mime_type) || typeof image.base64 !== 'string' || image.base64.length > 1398104 || ![image.width, image.height].every(value => Number.isInteger(value) && value > 0 && value <= 4096)) throw new Error('Invalid prepared raster');
      const bytes = Buffer.from(image.base64, 'base64');
      if (bytes.toString('base64') !== image.base64) throw new Error('Invalid prepared raster encoding');
      const format = image.mime_type === 'image/png' ? 'png' : 'jpeg';
      prepared.push({ bytes, format, name: `${asset.name}.prepared.${format}`, width: image.width, height: image.height });
    }
    signal?.throwIfAborted();
    if (this.#version !== version) throw new Error('Asset registry changed during preparation; inspect list_assets before retrying');
    const retained = this.retainMany(prepared);
    return { assets: retained.map((asset, index) => ({ ...asset, source_asset_id: requests[index].asset.asset_id })), usage: this.usage() };
  }
  async registerFiles(inputs, signal, inspectRaster) {
    if (!Array.isArray(inputs) || inputs.length < 1 || inputs.length > assetLimit) throw new Error('Register 1-32 assets per batch');
    const requests = inputs.map(input => {
      if (!input || typeof input !== 'object' || Array.isArray(input) || Object.keys(input).some(key => key !== 'path' && key !== 'root') || typeof input.path !== 'string' || input.path.length < 1 || input.path.length > 512 || (input.root !== undefined && (!Number.isInteger(input.root) || input.root < 0 || input.root > 7))) throw new Error('Invalid asset registration input');
      return { path: input.path, root: input.root };
    });
    const version = this.#version;
    const state = { assets: new Map(this.#assets), bytes: this.#bytes };
    const result = [];
    const checks = [];
    for (const input of requests) {
      signal?.throwIfAborted();
      const { check, ...asset } = await this.#readFile(input, signal, inspectRaster, state.assets);
      signal?.throwIfAborted();
      const retained = this.#retain([asset], state);
      result.push(...retained);
      checks.push({ ...check, sha256: retained[0].sha256 });
    }
    for (const { root, segments, stat, sha256 } of checks) {
      signal?.throwIfAborted();
      const current = await this.#inspect(root, segments);
      if (!sameVersion(stat, current.stat)) {
        if (!sameFile(stat, current.stat) || stat.size !== current.stat.size || stat.mtimeNs !== current.stat.mtimeNs) throw new Error('Asset changed before batch registration');
        await this.#verifySnapshot(root, segments, current, sha256, signal);
      }
    }
    signal?.throwIfAborted();
    if (this.#version !== version) throw new Error('Asset registry changed during registration; inspect list_assets before retrying');
    this.#commit(state);
    return { assets: result, usage: this.usage() };
  }
  async #verifySnapshot(root, segments, inspected, sha256, signal) {
    const file = await open(inspected.path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
    try {
      const before = await file.stat({ bigint: true });
      if (!before.isFile() || !sameVersion(before, inspected.stat)) throw new Error('Asset changed before snapshot verification');
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
      if (length !== Number(before.size) || !sameVersion(before, after) || !sameVersion(after, current.stat) || createHash('sha256').update(buffer.subarray(0, length)).digest('hex') !== sha256) throw new Error('Asset changed during snapshot verification');
      signal?.throwIfAborted();
    } finally { await file.close(); }
  }
  async #readFile({ path, root: rootIndex = 0 }, signal, inspectRaster, staged) {
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
      if (length !== Number(before.size) || !sameVersion(before, after) || !sameVersion(after, current.stat)) throw new Error('Asset changed while reading');
      signal?.throwIfAborted();
      const bytes = buffer.subarray(0, length);
      let dimensions;
      if (format === 'png' || format === 'jpeg') {
        const sha256 = createHash('sha256').update(bytes).digest('hex');
        const existing = [...staged.values()].find(asset => asset.sha256 === sha256 && asset.format === format && asset.width !== undefined && asset.height !== undefined);
        if (existing) dimensions = { width: existing.width, height: existing.height };
        else {
          if (typeof inspectRaster !== 'function') throw new Error('Raster inspection is required before file registration');
          const info = await inspectRaster({ base64: bytes.toString('base64'), mime_type: definition[0] }, signal);
          signal?.throwIfAborted();
          if (!info || info.mime_type !== definition[0] || info.sha256 !== sha256 || info.byte_length !== length || ![info.width, info.height].every(value => Number.isInteger(value) && value > 0 && value <= 4096)) throw new Error('Raster inspection dimensions or identity do not match the registered bytes');
          dimensions = { width: info.width, height: info.height };
        }
      }
      return { bytes, format, name: basename(path), ...dimensions, check: { root, segments, stat: before } };
    } finally { await file.close(); }
  }
}
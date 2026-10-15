import { createHash, randomUUID } from 'node:crypto';
import { AsyncLocalStorage } from 'node:async_hooks';
import { execFileSync } from 'node:child_process';
import { constants } from 'node:fs';
import { lstat, mkdir, mkdtemp, open, realpath, rmdir } from 'node:fs/promises';
import { homedir } from 'node:os';
import { dirname, isAbsolute, join, normalize, parse, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import { unzipSync } from 'fflate';
import sharp from 'sharp';
import { publishNewFile } from './atomic-output.mjs';

export const catalogPath = fileURLToPath(new URL('../crates/aislide-core/src/architecture-icons.json', import.meta.url));
const release = '2026-09-20';
const imageLimit = 1024 * 1024;
const catalogLimit = 4 * 1024 * 1024;
const expectedCounts = { azure: 645, aws: 808, gcp: 45 };
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const pngSignature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const providers = [
	{
		id: 'azure', name: 'Microsoft Azure and Entra',
		terms_url: 'https://learn.microsoft.com/en-us/azure/architecture/icons/',
		notice: 'Microsoft artwork, not an AISlide software license. Use only for architecture diagrams, training or documentation, with full service labels. Preserve design, colors and proportions; no cropping, flipping, rotation or implied endorsement. Entra terms: https://learn.microsoft.com/en-us/entra/architecture/architecture-icons . No marketing use. AISlide supplies metadata and a local opt-in downloader, not a redistributed icon library.',
		archives: [
			{ id: 'azure-v24.zip', url: 'https://arch-center.azureedge.net/icons/Azure_Public_Service_Icons_V24.zip', sha256: '921594ccd1bf3d9c0a1bd7b6d924e050551a59342f2b353bb74bdcf761c35141', bytes: 1033184 },
			{ id: 'entra-202310.zip', url: 'https://download.microsoft.com/download/3/1/a/31a56038-856a-4489-88e4-ee5a1c4352be/Microsoft%20Entra%20architecture%20icons%20-%20Oct%202023.zip?wt.mc_id=microsoftentraicons_downloadmicrosoftentraicons_content_cnl_csasci', sha256: '4e07536706a2d092e6524e5417e2c861333fdbdb41c36f78d44dfa07ccc5eedc', bytes: 7705773 },
		],
	},
	{
		id: 'aws', name: 'Amazon Web Services',
		terms_url: 'https://aws.amazon.com/architecture/icons/',
		notice: 'AWS artwork for permitted architecture diagrams and presentations, not MIT or an AISlide software license. Preserve appearance and identify the service; no implied endorsement. Trademark conditions: https://aws.amazon.com/trademark-guidelines/ . Bundled-library redistribution rights are not established. Local consent does not grant redistribution or sublicensing rights.',
		archives: [
			{ id: 'aws-07312026.zip', url: 'https://d1.awsstatic.com/onedam/marketing-channels/website/public/shared/architecture-icon-release/Icon-package_07312026.5846e92413caa21490223536cc97f1269e44fa92.zip', sha256: 'd2d166c453526471749d520e0db022c459abef759d2946cf2dd1d1c992dc6526', bytes: 13988918 },
		],
	},
	{
		id: 'gcp', name: 'Google Cloud',
		terms_url: 'https://cloud.google.com/icons',
		notice: 'Google artwork for permitted diagrams and technical documentation, not an open-source or AISlide software license. Preserve design and service identification; no implied endorsement. Product-icon guidance: https://partnermarketinghub.withgoogle.com/brands/google/branding-guidelines/how-to-show-googles-brand/ ; trademark rules: https://partnermarketinghub.withgoogle.com/brands/google/trademarks-and-terms/trademark-guidelines-for-proper-usage/ . Additional permission may be required; bundled redistribution is not established. Consent grants no additional rights. The official Vertex AI source is not relabeled Gemini.',
		archives: [
			{ id: 'gcp-core.zip', url: 'https://services.google.com/fh/files/misc/core-products-icons.zip', sha256: '6531a10f58bc599c24d9a455d81dd757c1a03c3c43da9cddf639b859c1c1eece', bytes: 318678 },
			{ id: 'gcp-category.zip', url: 'https://services.google.com/fh/files/misc/category-icons.zip', sha256: 'e5bc3abd3527dc2500e9bff7f15870783e2c764129c49b7cd4c1b4e105345002', bytes: 795968 },
		],
	},
];
const archives = providers.flatMap(provider => provider.archives);
const localDrives = new AsyncLocalStorage();

function requireConsent(options) {
	if (options.acceptVendorTerms !== true) {
		throw new Error('Review vendor terms and pass --accept-vendor-terms. Acceptance does not grant redistribution rights.');
	}
}

function localPath(value) {
	if (typeof value !== 'string' || !isAbsolute(value) || /^[\\/]{2}/.test(value)
		|| /[\x00-\x1f]/.test(value) || /(^|[\\/])\.\.?([\\/]|$)/.test(value)
		|| value.replace(/^[A-Za-z]:/, '').includes(':') || normalize(value) === parse(value).root) {
		throw new Error('Expected an absolute local directory, not a relative, URL, UNC, device or traversal path');
	}
	return normalize(value);
}

function assertLocalDrive(path) {
	if (process.platform !== 'win32') return;
	const invalid = () => new Error('Expected a known local backing drive; remote, unknown or unavailable drives are not allowed');
	if (!/^[A-Za-z]:[\\/]/.test(path)) throw invalid();
	const drive = `${path[0].toUpperCase()}:\\`;
	const checked = localDrives.getStore();
	if (checked?.has(drive)) return;
	let result;
	try {
		const systemRoot = localPath(process.env.SystemRoot ?? 'C:\\Windows');
		if (!/^[A-Za-z]:[\\/]/.test(systemRoot)) throw invalid();
		result = execFileSync(join(systemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe'),
			['-NoLogo', '-NoProfile', '-NonInteractive', '-Command', `[int][System.IO.DriveInfo]::new('${drive}').DriveType`],
			{ encoding: 'utf8', timeout: 10000, maxBuffer: 1024, windowsHide: true, shell: false, stdio: ['ignore', 'pipe', 'pipe'] });
	} catch {
		throw invalid();
	}
	if (!/^[2356]$/.test(result.trim())) throw invalid();
	checked?.add(drive);
}

export function defaultPackDirectory({ env = process.env, platform = process.platform, homeDirectory = homedir() } = {}) {
	if (env.AISLIDE_ICON_PACK_ROOT !== undefined) return localPath(env.AISLIDE_ICON_PACK_ROOT);
	const base = platform === 'win32'
		? env.LOCALAPPDATA
		: env.XDG_DATA_HOME || join(localPath(homeDirectory), '.local', 'share');
	return join(localPath(base), 'AISlide', 'icon-packs', release);
}

async function safeDirectory(directory, create = false) {
	const path = localPath(directory);
	assertLocalDrive(path);
	let info;
	try { info = await lstat(path); } catch (error) {
		if (error.code !== 'ENOENT') throw error;
		const parent = dirname(path);
		if (parent === parse(path).root) throw new Error('Missing local filesystem root');
		await safeDirectory(parent, create);
		if (!create) return false;
		try { await mkdir(path, { mode: 0o700 }); } catch (creationError) {
			if (creationError.code !== 'EEXIST') throw creationError;
		}
		info = await lstat(path);
	}
	if (info.isSymbolicLink() || !info.isDirectory()) throw new Error('Symlink, junction or non-directory in icon pack path');
	const actual = normalize(await realpath(path));
	const equal = process.platform === 'win32' ? actual.toLowerCase() === path.toLowerCase() : actual === path;
	if (!equal) throw new Error('Redirected icon pack directory or ancestor');
	return true;
}

async function readBounded(path, limit, optional = false) {
	if (!await safeDirectory(dirname(path))) {
		if (optional) return undefined;
		throw new Error(`Missing directory for ${path}`);
	}
	let info;
	try { info = await lstat(path); } catch (error) {
		if (optional && error.code === 'ENOENT') return undefined;
		throw error;
	}
	if (info.isSymbolicLink() || !info.isFile() || info.nlink !== 1 || info.size > limit) {
		throw new Error('Unsafe or oversized icon pack file; existing content will not be overwritten');
	}
	const file = await open(path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
	try {
		const opened = await file.stat();
		if (opened.dev !== info.dev || opened.ino !== info.ino || opened.size !== info.size) throw new Error('File changed during open');
		const bytes = Buffer.alloc(info.size + 1);
		let length = 0;
		while (length < bytes.length) {
			const result = await file.read(bytes, length, bytes.length - length, null);
			if (result.bytesRead === 0) break;
			length += result.bytesRead;
		}
		const after = await file.stat();
		if (length !== info.size || after.size !== info.size || after.mtimeMs !== info.mtimeMs) throw new Error('File changed during read');
		return bytes.subarray(0, length);
	} finally { await file.close(); }
}

function entryPath(value) {
	if (typeof value !== 'string' || value.length > 1024 || /^[\\/]/.test(value)
		|| /[\\:\x00-\x1f]/.test(value) || value.split('/').some(part => part === '.' || part === '..')) {
		throw new Error('Unsafe archive entry path');
	}
}

function exactKeys(value, keys) {
	return value && typeof value === 'object' && isDeepStrictEqual(Object.keys(value).sort(), [...keys].sort());
}

function label(value) {
	return typeof value === 'string' && value.trim().length > 0 && value.length <= 512 && !/[\x00-\x1f]/.test(value);
}

function validateCatalog(catalog, prepared = true) {
	if (!exactKeys(catalog, ['version', 'release', 'providers', 'icons']) || catalog.version !== 1 || catalog.release !== release
		|| !isDeepStrictEqual(catalog.providers, providers) || !Array.isArray(catalog.icons) || catalog.icons.length !== 1498) {
		throw new Error('Invalid cloud catalog version, official archive pins or icon count');
	}
	const ids = new Set();
	const counts = { azure: 0, aws: 0, gcp: 0 };
	for (const icon of catalog.icons) {
		const fields = ['id', 'provider', 'name', 'kind', 'categories', 'aliases', 'source'];
		if (prepared) fields.push('png_sha256', 'png_bytes', 'width', 'height');
		if (!exactKeys(icon, fields) || !Object.hasOwn(counts, icon.provider) || typeof icon.id !== 'string'
			|| !/^[a-z0-9]+\/[a-z0-9/-]+$/.test(icon.id) || !icon.id.startsWith(`${icon.provider}/`)
			|| icon.id.length > 256 || ids.has(icon.id) || !label(icon.name) || !label(icon.kind)
			|| ![icon.categories, icon.aliases].every(values => Array.isArray(values) && values.length > 0 && values.length <= 64 && values.every(label))) {
			throw new Error('Invalid or duplicate cloud icon identity/labels');
		}
		const source = icon.source;
		if (!exactKeys(source, ['archive_id', 'entry', 'sha256']) || !/^[a-f0-9]{64}$/.test(source.sha256)
			|| !providers.find(provider => provider.id === icon.provider).archives.some(archive => archive.id === source.archive_id)) {
			throw new Error('Invalid cloud icon source');
		}
		entryPath(source.entry);
		if (icon.provider === 'azure' ? !source.entry.endsWith('.svg') : !source.entry.endsWith('.png')) throw new Error('Unexpected cloud icon source format');
		if (prepared && (!/^[a-f0-9]{64}$/.test(icon.png_sha256) || !Number.isInteger(icon.png_bytes) || icon.png_bytes < 8 || icon.png_bytes > imageLimit
			|| ![icon.width, icon.height].every(value => Number.isInteger(value) && value > 0 && value <= 512))) {
			throw new Error('Invalid prepared PNG hash, dimensions or size');
		}
		ids.add(icon.id);
		counts[icon.provider] += 1;
	}
	if (!isDeepStrictEqual(counts, expectedCounts)) throw new Error('Cloud catalog provider coverage mismatch');
	return catalog;
}

export function verifyCatalog(bytes) {
	if (!Buffer.isBuffer(bytes) || bytes.length > catalogLimit || bytes.subarray(0, 3).equals(Buffer.from([239, 187, 191]))) {
		throw new Error('Cloud catalog must be bounded UTF-8 JSON without BOM');
	}
	return validateCatalog(JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)));
}

function pinnedArchive(archive) {
	const pinned = archives.find(value => value.id === archive?.id);
	if (!pinned || !isDeepStrictEqual(pinned, archive)) throw new Error('Unknown or modified official archive pin');
	return pinned;
}

export async function downloadArchive(archive, fetcher = globalThis.fetch) {
	pinnedArchive(archive);
	const response = await fetcher(archive.url, { redirect: 'error', credentials: 'omit', signal: AbortSignal.timeout(120000) });
	if (response.status !== 200 || response.redirected || (response.url && response.url !== archive.url) || !response.body) {
		await response.body?.cancel();
		throw new Error('Official archive response rejected; redirects are not permitted');
	}
	const length = response.headers.get('content-length');
	if (length !== null && Number(length) !== archive.bytes) {
		await response.body.cancel();
		throw new Error('Official archive size mismatch');
	}
	const reader = response.body.getReader();
	const chunks = [];
	let received = 0;
	try {
		for (;;) {
			const result = await reader.read();
			if (result.done) break;
			received += result.value.byteLength;
			if (received > archive.bytes) throw new Error('Official archive exceeds pinned size');
			chunks.push(Buffer.from(result.value));
		}
	} finally {
		await reader.cancel();
		reader.releaseLock();
	}
	const bytes = Buffer.concat(chunks);
	if (bytes.length !== archive.bytes || hash(bytes) !== archive.sha256) throw new Error('Official archive SHA-256/size mismatch');
	return bytes;
}

export function readArchiveEntries(archive, bytes, icons) {
	pinnedArchive(archive);
	if (bytes.length !== archive.bytes || hash(bytes) !== archive.sha256) throw new Error('Official archive SHA-256/size mismatch before ZIP decoding');
	const wanted = new Map();
	for (const icon of icons) {
		if (icon.source.archive_id !== archive.id) continue;
		entryPath(icon.source.entry);
		if (!/\.(svg|png)$/.test(icon.source.entry)) throw new Error('Unexpected archive image format');
		const previous = wanted.get(icon.source.entry);
		if (previous && previous !== icon.source.sha256) throw new Error('Conflicting archive entry hashes');
		wanted.set(icon.source.entry, icon.source.sha256);
	}
	const seen = new Set();
	let selectedBytes = 0;
	const entries = unzipSync(bytes, {
		filter: entry => {
			entryPath(entry.name);
			if (seen.has(entry.name) || seen.size >= 20000) throw new Error('Duplicate or excessive ZIP entries');
			seen.add(entry.name);
			if (!wanted.has(entry.name)) return false;
			selectedBytes += entry.originalSize;
			if (entry.originalSize > imageLimit || selectedBytes > 32 * imageLimit) throw new Error('ZIP selected image size limit exceeded');
			return true;
		},
	});
	if (Object.keys(entries).length !== wanted.size) throw new Error('Missing selected official image entry');
	for (const [name, expected] of wanted) {
		if (hash(entries[name]) !== expected) throw new Error('Official image source SHA-256 mismatch');
	}
	return entries;
}

function guardSvg(bytes) {
	const source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
	const references = [...source.matchAll(/url\(\s*['"]?([^)'"\s]+)/gi)].map(match => match[1]);
	references.push(...[...source.matchAll(/\bhref\s*=\s*(["'])(.*?)\1/gis)].map(match => match[2]));
	if (!/<svg[\s>]/.test(source) || /<!DOCTYPE|<!ENTITY|<\?(?!xml\s)|\bon[a-z]+\s*=|@|\\|&#/i.test(source)
		|| /<(?:[\w.-]+:)?(?:script|foreignObject|image|use|text|filter|animate\w*|set|a)\b/i.test(source)
		|| references.some(reference => !/^#[\w.:-]+$/.test(reference))) {
		throw new Error('Active or external SVG content rejected');
	}
}

export async function inspectPng(bytes) {
	if (bytes.length > imageLimit || !bytes.subarray(0, 8).equals(pngSignature)) throw new Error('Invalid PNG magic or byte limit');
	const image = sharp(bytes, { limitInputPixels: 4096 * 4096, failOn: 'warning' });
	const metadata = await image.metadata();
	if (metadata.format !== 'png' || (metadata.pages ?? 1) !== 1 || metadata.width > 512 || metadata.height > 512) {
		throw new Error('PNG dimensions/pages exceed icon limits');
	}
	const { data, info } = await image.ensureAlpha().raw().toBuffer({ resolveWithObject: true });
	let paintedPixels = 0;
	const bounds = { left: info.width, top: info.height, right: -1, bottom: -1 };
	for (let row = 0; row < info.height; row += 1) {
		for (let column = 0; column < info.width; column += 1) {
			if (data[(row * info.width + column) * info.channels + info.channels - 1] === 0) continue;
			paintedPixels += 1;
			bounds.left = Math.min(bounds.left, column);
			bounds.top = Math.min(bounds.top, row);
			bounds.right = Math.max(bounds.right, column);
			bounds.bottom = Math.max(bounds.bottom, row);
		}
	}
	if (paintedPixels === 0) throw new Error('Blank transparent PNG rejected');
	return { width: info.width, height: info.height, paintedPixels, bounds };
}

export async function prepareIconPng(icon, input) {
	const bytes = Buffer.from(input);
	if (bytes.length > imageLimit || hash(bytes) !== icon.source.sha256) throw new Error('Official image source SHA-256/size mismatch');
	entryPath(icon.source.entry);
	let png;
	if (icon.source.entry.endsWith('.png')) {
		if (!bytes.subarray(0, 8).equals(pngSignature)) throw new Error('Invalid PNG magic');
		const image = sharp(bytes, { limitInputPixels: 4096 * 4096, failOn: 'warning' });
		const source = await image.metadata();
		if (source.format !== 'png' || (source.pages ?? 1) !== 1 || source.width > 4096 || source.height > 4096) throw new Error('Invalid PNG dimensions/pages');
		if (source.width > 512 || source.height > 512) {
			if (sharp.versions.sharp !== '0.35.4') throw new Error('Trusted PNG resizing requires Sharp 0.35.4');
			png = await image.resize({ width: 256, height: 256, fit: 'inside', withoutEnlargement: true })
				.png({ compressionLevel: 9, adaptiveFiltering: false, palette: false }).toBuffer();
		} else {
			png = bytes;
		}
	} else if (icon.provider === 'azure' && ['azure-v24.zip', 'entra-202310.zip'].includes(icon.source.archive_id) && icon.source.entry.endsWith('.svg')) {
		if (sharp.versions.sharp !== '0.35.4') throw new Error('Trusted SVG conversion requires Sharp 0.35.4');
		guardSvg(bytes);
		const image = sharp(bytes, { density: 72, limitInputPixels: 4096 * 4096, failOn: 'warning' });
		const source = await image.metadata();
		if (source.format !== 'svg' || source.width > 4096 || source.height > 4096) throw new Error('Invalid SVG dimensions');
		png = await image.resize({ width: 256, height: 256, fit: 'inside' }).png({ compressionLevel: 9, adaptiveFiltering: false, palette: false }).toBuffer();
	} else {
		throw new Error('Unexpected official image format');
	}
	const measured = await inspectPng(png);
	const prepared = { png_sha256: hash(png), png_bytes: png.length, width: measured.width, height: measured.height };
	if (icon.png_sha256 !== undefined && !isDeepStrictEqual(prepared, {
		png_sha256: icon.png_sha256, png_bytes: icon.png_bytes, width: icon.width, height: icon.height,
	})) throw new Error(`Prepared PNG pin mismatch for ${icon.id}; use the qualified Sharp 0.35.4 stack, do not rewrite catalog pins`);
	return { bytes: png, ...prepared };
}

async function prepareAll(catalog, { cacheDirectory, fetch: fetcher = globalThis.fetch } = {}) {
	const cache = cacheDirectory === undefined ? undefined : localPath(cacheDirectory);
	const prepared = new Map();
	const converted = new Map();
	for (const archive of archives) {
		const cached = cache === undefined ? undefined : await readBounded(join(cache, archive.id), archive.bytes, true);
		const archiveBytes = cached ?? await downloadArchive(archive, fetcher);
		const icons = catalog.icons.filter(icon => icon.source.archive_id === archive.id);
		const entries = readArchiveEntries(archive, archiveBytes, icons);
		for (const icon of icons) {
			const key = `${icon.provider}:${icon.source.sha256}`;
			let value = converted.get(key);
			if (!value) {
				try { value = await prepareIconPng(icon, entries[icon.source.entry]); } catch (error) {
					throw new Error(`${icon.id}: ${error.message}`, { cause: error });
				}
				converted.set(key, value);
			}
			if (icon.png_sha256 !== undefined && (icon.png_sha256 !== value.png_sha256 || icon.png_bytes !== value.png_bytes
				|| icon.width !== value.width || icon.height !== value.height)) throw new Error('Prepared PNG catalog pin mismatch');
			prepared.set(icon.id, value);
		}
	}
	return prepared;
}

function assetPath(directory, icon) {
	return join(directory, icon.provider, `${icon.png_sha256}.png`);
}

async function checkExisting(path, expected, limit) {
	const current = await readBounded(path, limit, true);
	if (current && !current.equals(expected)) throw new Error('Existing icon pack content differs; refusing to overwrite');
	return current !== undefined;
}

export async function installIconPacks(options = {}) {
	return localDrives.run(new Set(), () => installLocalIconPacks(options));
}

async function installLocalIconPacks(options) {
	requireConsent(options);
	const directory = localPath(options.packDirectory ?? defaultPackDirectory());
	assertLocalDrive(directory);
	if (options.cacheDirectory !== undefined) assertLocalDrive(localPath(options.cacheDirectory));
	assertLocalDrive(catalogPath);
	const catalogBytes = await readBounded(catalogPath, catalogLimit);
	const catalog = verifyCatalog(catalogBytes);
	const catalogSha256 = hash(catalogBytes);
	const consent = Buffer.from(`${JSON.stringify({ version: 1, catalog_sha256: catalogSha256, accepted_vendor_terms: true }, null, 2)}\n`);
	const consentPath = join(directory, 'consent.json');
	const ready = await checkExisting(consentPath, consent, 1024);
	const existing = new Map();
	if (await safeDirectory(directory)) {
		for (const icon of catalog.icons) {
			const path = assetPath(directory, icon);
			if (existing.has(path)) continue;
			const bytes = await readBounded(path, imageLimit, true);
			if (!bytes) continue;
			if (bytes.length !== icon.png_bytes || hash(bytes) !== icon.png_sha256) throw new Error('Existing PNG hash/size mismatch; refusing to overwrite');
			const dimensions = await inspectPng(bytes);
			if (dimensions.width !== icon.width || dimensions.height !== icon.height) throw new Error('Existing PNG dimension mismatch');
			existing.set(path, bytes);
		}
	}
	const uniquePaths = new Set(catalog.icons.map(icon => assetPath(directory, icon)));
	if (ready && existing.size === uniquePaths.size) {
		return { directory, catalog_sha256: catalogSha256, icons: catalog.icons.length, files: uniquePaths.size, written: 0, ready: true };
	}
	const prepared = await prepareAll(catalog, options);
	await safeDirectory(directory, true);
	for (const provider of providers) await safeDirectory(join(directory, provider.id), true);
	const stage = await mkdtemp(join(directory, '.prepare-'));
	const stageIdentity = await lstat(stage);
	let written = 0;
	try {
		for (const icon of catalog.icons) {
			const path = assetPath(directory, icon);
			if (existing.has(path)) continue;
			const png = prepared.get(icon.id).bytes;
			await safeDirectory(dirname(path));
			await safeDirectory(stage);
			try { await publishNewFile(join(stage, randomUUID()), path, png); written += 1; } catch (error) {
				if (error.code !== 'EEXIST') throw error;
				await checkExisting(path, png, imageLimit);
			}
			existing.set(path, png);
		}
		for (const icon of catalog.icons) {
			const bytes = await readBounded(assetPath(directory, icon), imageLimit);
			if (bytes.length !== icon.png_bytes || hash(bytes) !== icon.png_sha256) throw new Error('Final pack verification failed; consent not published');
		}
		await safeDirectory(directory);
		await safeDirectory(stage);
		try { await publishNewFile(join(stage, randomUUID()), consentPath, consent); } catch (error) {
			if (error.code !== 'EEXIST') throw error;
			await checkExisting(consentPath, consent, 1024);
		}
	} finally {
		const current = await lstat(stage);
		if (!current.isSymbolicLink() && current.dev === stageIdentity.dev && current.ino === stageIdentity.ino) await rmdir(stage);
	}
	return { directory, catalog_sha256: catalogSha256, icons: catalog.icons.length, files: uniquePaths.size, written, ready: true };
}

export async function buildCatalog(options = {}) {
	return localDrives.run(new Set(), () => buildLocalCatalog(options));
}

async function buildLocalCatalog(options) {
	requireConsent(options);
	const research = localPath(options.researchDirectory);
	assertLocalDrive(research);
	if (options.cacheDirectory !== undefined) assertLocalDrive(localPath(options.cacheDirectory));
	assertLocalDrive(catalogPath);
	const readJson = async name => JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(await readBounded(join(research, name), 64 * imageLimit)));
	const raw = await readJson('catalog.json');
	const coverage = await readJson('coverage.json');
	const provenance = await readJson('research-manifest.json');
	for (const archive of archives) {
		const source = provenance.sources.find(value => value.name === archive.id);
		if (!source || source.source !== archive.url || source.sha256 !== archive.sha256 || source.bytes !== archive.bytes || source.complete !== true) {
			throw new Error('Canonical research does not match official archive pins');
		}
	}
	if (!Array.isArray(raw.items) || raw.items.length !== 1498 || !Array.isArray(coverage.entries)) throw new Error('Invalid canonical research coverage');
	const icons = raw.items.map(item => {
		const source = item.preferredPng ?? item.preferredSvg;
		const mappings = coverage.entries.filter(entry => entry.id === item.id);
		const preferredLabel = mappings.find(entry => !entry.officialCategoryGlyph)?.label;
		const original = item.displayName || item.sourceName;
		const fullName = /^(?:Azure |Microsoft |Amazon |AWS |Google Cloud )/.test(original)
			? original : `${{ azure: 'Azure', aws: 'AWS', gcp: 'Google Cloud' }[item.provider]} ${original}`;
		return {
			id: item.id, provider: item.provider, name: preferredLabel ?? fullName, kind: item.kind,
			categories: item.categories,
			aliases: [...new Set([item.sourceName, item.displayName, ...(item.englishSearchKeywords ?? []),
				...mappings.flatMap(entry => [entry.label, ...(entry.englishSearchKeywords ?? [])])].filter(Boolean))],
			source: { archive_id: source.archive, entry: source.path, sha256: source.sha256 },
		};
	}).sort((first, second) => first.id < second.id ? -1 : first.id > second.id ? 1 : 0);
	const catalog = validateCatalog({ version: 1, release, providers, icons }, false);
	const prepared = await prepareAll(catalog, { ...options, cacheDirectory: options.cacheDirectory ?? research });
	for (const icon of icons) {
		const { bytes: unusedBytes, ...metadata } = prepared.get(icon.id);
		void unusedBytes;
		Object.assign(icon, metadata);
	}
	const bytes = Buffer.from(`${JSON.stringify(catalog, null, 2)}\n`);
	verifyCatalog(bytes);
	if (!await checkExisting(catalogPath, bytes, catalogLimit)) {
		await publishNewFile(join(dirname(catalogPath), `.architecture-icons-${randomUUID()}.tmp`), catalogPath, bytes);
	}
	return { catalogPath, catalog_sha256: hash(bytes), icons: icons.length, providers: expectedCounts,
		source_png: icons.filter(icon => icon.source.entry.endsWith('.png')).length,
		unchanged_png: icons.filter(icon => icon.png_sha256 === icon.source.sha256).length,
		resized_png: icons.filter(icon => icon.source.entry.endsWith('.png') && icon.png_sha256 !== icon.source.sha256).length,
		rasterized_svg: icons.filter(icon => icon.source.entry.endsWith('.svg')).length,
		prepared_bytes: [...new Map(icons.map(icon => [`${icon.provider}:${icon.png_sha256}`, icon.png_bytes])).values()].reduce((total, size) => total + size, 0) };
}

function termsText() {
	return providers.map(provider => `${provider.name}\n${provider.terms_url}\n${provider.notice}`).join('\n\n');
}

async function main(args) {
	const options = {};
	const seen = new Set();
	let help = false;
	for (const argument of args) {
		const name = argument.split('=')[0];
		if (seen.has(name)) throw new Error(`Duplicate setup option: ${name}`);
		seen.add(name);
		if (argument === '--help') help = true;
		else if (argument === '--accept-vendor-terms') options.acceptVendorTerms = true;
		else if (argument.startsWith('--cache-dir=')) options.cacheDirectory = localPath(argument.slice('--cache-dir='.length));
		else if (argument.startsWith('--build-catalog=')) options.researchDirectory = localPath(argument.slice('--build-catalog='.length));
		else throw new Error(`Unknown setup option: ${name}`);
	}
	console.log(termsText());
	console.log('\nUsage: node tools/cloud-icons-setup.mjs --accept-vendor-terms [--cache-dir=<absolute-local-directory>]');
	console.log('Maintainer only: --build-catalog=<absolute-canonical-research-directory>. Metadata only; never vendor artwork in source.');
	if (help) return;
	const result = options.researchDirectory ? await buildCatalog(options) : await installIconPacks(options);
	console.log(JSON.stringify(result, null, 2));
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	main(process.argv.slice(2)).catch(error => { console.error(error.message); process.exitCode = 1; });
}
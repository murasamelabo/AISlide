import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { randomUUID, createHash } from 'node:crypto';
import { mkdir, realpath } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { requestCore, MAX_REQUEST_BYTES } from './core-client.mjs';
import { publishNewFile } from './atomic-output.mjs';
import { publishProject } from './atomic-project.mjs';
import { AislideClient } from '../packages/client/index.mjs';

const server = new McpServer({ name: 'aislide', version: '0.1.0' });
const args = process.argv.slice(2);
if (args.length !== 0 && (args.length !== 2 || args[0] !== '--output-dir')) {
	throw new Error('Usage: node tools/mcp.mjs [--output-dir path]');
}
let outputDirectory;
if (args[1]) {
	await mkdir(resolve(args[1]), { recursive: true });
	outputDirectory = await realpath(resolve(args[1]));
}
const decks = new Map();
const sources = new Map();
const client = new AislideClient(requestCore);
let activeMutation = false;
const handle = z.string().uuid();
const shortText = z.string().max(120);
const chartSchema = z.object({ kind: z.enum(['column', 'bar', 'line']), categories: z.array(z.string().max(80)).min(1).max(32), series: z.array(z.object({ name: z.string().max(80), values: z.array(z.number().min(-1e15).max(1e15)).min(1).max(32), color: z.string().regex(/^[0-9a-fA-F]{6}$/) }).strict()).min(1).max(6) }).strict();
const section = z.object({
	title: z.string().max(100),
	layout: z.enum(['cover', 'metrics', 'table', 'columns', 'statement', 'chart', 'process']),
	body: z.array(z.string().max(240)).max(4).default([]),
	metrics: z.array(z.object({ label: z.string().max(48), value: z.string().max(20) }).strict()).max(4).default([]),
	rows: z.array(z.array(z.string().max(200)).min(1).max(8)).max(12).default([]),
	chart: chartSchema.optional(),
}).strict();
const reportSchema = z.object({ title: shortText, subtitle: z.string().max(200), period: z.string().max(80), source: z.string().max(1200), sections: z.array(section).min(1).max(32) }).strict();
const filenameSchema = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,79}\.pptx$/).refine((name) => !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])\./i.test(name), 'Reserved Windows filename');

function getDeck(id) {
	const state = decks.get(id);
	if (!state) throw new Error('Unknown deck handle; create or compile a deck first');
	return state;
}

function register(name, description, inputSchema, readOnly, action) {
	server.registerTool(name, {
		description,
		inputSchema: z.object(inputSchema).strict(),
		annotations: { readOnlyHint: readOnly, destructiveHint: false, openWorldHint: name === 'generate_report' },
	}, async (input, extra) => {
		if (!readOnly && activeMutation) return { isError: true, content: [{ type: 'text', text: 'Another mutation is in progress' }] };
		if (!readOnly) activeMutation = true;
		try {
			const result = await action(input, extra.signal);
			return { content: [{ type: 'text', text: JSON.stringify(result) }], structuredContent: result };
		} catch (error) {
			return { isError: true, content: [{ type: 'text', text: error instanceof Error ? error.message : 'Operation failed' }] };
		} finally { if (!readOnly) activeMutation = false; }
	});
}

register('sample_report', 'Return a twelve-slide structured example. All numbers are synthetic, and no model or network request is used.', {}, true, async (_input, signal) => requestCore({ op: 'sample' }, { signal }));
register('provider_status', 'Read operator-configured model availability and destination without API keys. This does not contact the provider or validate credentials.', {}, true, async (_input, signal) => requestCore({ op: 'provider_status' }, { signal }));
register('generate_report', 'Ask the operator-configured OpenAI-compatible model for a draft, validate its ReportInput and compile it with the Rust core. No file is saved. Content and citations remain unverified. Requires operator remote opt-in and allow_remote=true before any non-loopback request. May take up to 300 seconds; cancellation stops the provider request. Never accepts endpoints or credentials as tool arguments.', {
	prompt: z.string().min(1).max(8000),
	source_text: z.string().max(24_000).default(''),
	slide_count: z.number().int().min(1).max(32),
	allow_remote: z.boolean().default(false),
    max_repairs: z.number().int().min(0).max(1).default(0),
	outline: z.array(z.object({ title: z.string().min(1).max(100), layout: z.enum(['cover', 'metrics', 'table', 'columns', 'statement', 'chart', 'process']) }).strict()).max(32).default([]),
}, false, async (input, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const result = await requestCore({ op: 'generate', input }, { signal });
	if (signal.aborted) throw new Error('Operation cancelled');
	const id = randomUUID();
	decks.set(id, await client.createDocument({ id, deck: result.compiled.deck, report: result.report }, { signal }));
	return { deck_id: id, slides: result.compiled.deck.slides.length, revision: 0, issues: result.compiled.issues, provenance: result.provenance };
});
register('compile_report', 'Compile structured content into native text, rectangles, tables, charts and process groups. Returns a session document handle. Deterministic layout, not a model call.', { report: reportSchema }, false, async ({ report }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const result = await requestCore({ op: 'compile', report }, { signal });
	const id = randomUUID();
	decks.set(id, await client.createDocument({ id, deck: result.deck, report }, { signal }));
	return { deck_id: id, slides: result.deck.slides.length, revision: 0, issues: result.issues };
});
register('get_deck', 'Read the scene with stable slide and element IDs. Coordinates are in pixels on a 1280 x 720 page.', { deck_id: handle }, true, async ({ deck_id }) => getDeck(deck_id).document.deck);
register('get_document', 'Read the revisioned document, source hashes and field bindings. Hashes detect accidental changes, not source authenticity.', { deck_id: handle }, true, async ({ deck_id }) => getDeck(deck_id).document);
register('import_pptx', 'Open a caller-provided PPTX for approximate native preview and strictly limited non-destructive edits. Preserves original package bytes, including opaque parts. Unsupported content is not executed or fetched. Original byte identity is preserved on no-op export.', { base64: z.string().max(2_796_204) }, false, async ({ base64 }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const id = randomUUID(); const imported = await client.importPresentation(id, base64, { signal }); decks.set(id, imported.session);
	return { deck_id: id, revision: 0, slides: imported.session.document.deck.slides.length, warnings: imported.warnings, objects: imported.objects };
});
register('measure_layout', 'Measure text and table cells with installed fonts and cosmic-text. Reports overflow, missing glyphs and fallback fonts. This is not Office parity and does not measure charts or images.', { deck_id: handle }, true, async ({ deck_id }, signal) => requestCore({ op: 'measure_layout', deck: getDeck(deck_id).document.deck }, { signal }));
register('update_text', 'Set a text element without re-layout. The geometry stays fixed; validate visual fit separately. Optional expected_revision protects against stale edits.', { deck_id: handle, slide_id: shortText, element_id: shortText, text: z.string().max(4000), expected_revision: z.number().int().min(0).optional() }, false, async ({ deck_id, slide_id, element_id, text, expected_revision }, signal) => {
	const state = getDeck(deck_id);
	const revision = state.revision;
	if (expected_revision !== undefined && revision !== expected_revision) throw new Error('Revision conflict');
	const deck = state.document.deck;
	const slideIndex = deck.slides.findIndex((slide) => slide.id === slide_id);
	const elementIndex = deck.slides[slideIndex]?.elements.findIndex((item) => item.id === element_id);
	if (elementIndex === undefined || deck.slides[slideIndex].elements[elementIndex]?.type !== 'text') throw new Error('Target must be an existing top-level text element');
	await state.transact([{ op: 'replace', path: `/deck/slides/${slideIndex}/elements/${elementIndex}/text`, value: text }], { signal, expectedRevision: revision });
	return { deck_id, revision: state.revision, warnings: ['Text fit is not checked by structural validation'] };
});
register('undo', 'Undo the previous complete transaction using core-verified inverse operations.', { deck_id: handle }, false, async ({ deck_id }, signal) => {
	const state = getDeck(deck_id);
	await state.undo({ signal });
	return { deck_id, revision: state.revision };
});
register('redo', 'Reapply the last undone transaction; later edits invalidate redo history.', { deck_id: handle }, false, async ({ deck_id }, signal) => { const state = getDeck(deck_id); await state.redo({ signal }); return { deck_id, revision: state.revision }; });
register('validate_deck', 'Validate IDs, object types, text safety and page geometry. This does not prove Office visual parity or text fit.', { deck_id: handle }, true, async ({ deck_id }, signal) => requestCore({ op: 'validate', deck: getDeck(deck_id).document.deck }, { signal }));
register('export_pptx', 'Save a native PPTX inside the operator-approved --output-dir. Never overwrites an existing file. A plain .pptx filename is required.', { deck_id: handle, filename: filenameSchema }, false, async ({ deck_id, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const { base64 } = await getDeck(deck_id).exportProject({ signal });
	if (signal.aborted) throw new Error('Operation cancelled');
	const bytes = Buffer.from(base64, 'base64');
	const destination = join(outputDirectory, filename);
	const temporary = join(outputDirectory, `.aislide-${randomUUID()}.tmp`);
	await publishNewFile(temporary, destination, bytes, signal);
	return { path: destination, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') };
});
register('close_deck', 'Release the in-memory deck and its undo history. Saved files remain unchanged.', { deck_id: handle }, false, async ({ deck_id }) => {
	getDeck(deck_id);
	decks.delete(deck_id);
	return { closed: deck_id };
});

register('ingest_source', 'Read a caller-provided CSV, JSON, XLSX, UTF-8 text/Markdown, PDF or PNG/JPEG evidence file. Keeps hashes and cell/page locators. Optional image OCR uses installed Windows languages locally; results require review. Never fetches URLs, evaluates formulas or executes content.', {
	name: z.string().min(1).max(200), format: z.enum(['csv', 'json', 'xlsx', 'markdown', 'text', 'pdf', 'png', 'jpeg']), base64: z.string().max(2_796_204), ocr: z.boolean().default(false), ocr_language: z.string().max(35).optional(),
	attribution: z.object({ citation: z.string().max(1000), url: z.string().max(2048), license: z.string().max(500), derived_from_sha256: z.string().regex(/^[0-9a-fA-F]{64}$/).optional(), transformation: z.string().max(2000).default('') }).strict().optional(),
}, false, async (input, signal) => {
	if (sources.size >= 8) throw new Error('At most eight sources per session');
	const source = await client.ingest(input, { signal });
	const source_id = randomUUID(); sources.set(source_id, source);
	return { source_id, source };
});
register('close_source', 'Release an ingested source handle. Sources already attached to documents remain available in those documents.', { source_id: handle }, false, async ({ source_id }) => {
	if (!sources.delete(source_id)) throw new Error('Unknown source handle');
	return { closed: source_id };
});
const mappingSchema = z.object({ title: shortText, period: z.string().max(80), table_index: z.number().int().min(0).max(7), category_column: z.number().int().min(0).max(31), value_columns: z.array(z.number().int().min(0).max(31)).min(1).max(6), row_start: z.number().int().min(0).max(999), row_count: z.number().int().min(1).max(32), chart_kind: z.enum(['column', 'bar', 'line']) }).strict();
register('compile_data_report', 'Create a twelve-slide report from explicitly selected source cells. Numeric parsing is strict; no missing-value imputation or model request. Includes native charts, tables, a process diagram and source bindings.', { source_id: handle, mapping: mappingSchema }, false, async ({ source_id, mapping }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const source = sources.get(source_id); if (!source) throw new Error('Unknown source handle');
	const result = await client.dataReport(source, mapping, { signal });
	const id = randomUUID();
	decks.set(id, await client.createDocument({ id, deck: result.compiled.deck, report: result.report, sources: [source], bindings: result.bindings }, { signal }));
	return { deck_id: id, revision: 0, slides: result.compiled.deck.slides.length, source_sha256: source.sha256, bindings: result.bindings.length, issues: result.compiled.issues };
});
const patchSchema = z.array(z.object({ op: z.enum(['add', 'remove', 'replace', 'move', 'copy', 'test']), path: z.string().max(2048), from: z.string().max(2048).optional(), value: z.unknown().optional() }).strict()).min(1).max(128);
register('apply_transaction', 'Apply an atomic JSON Patch to document content (deck, sources, bindings, report). Revision is required. Invalid geometry, paths, references or preconditions roll back the whole batch. Edits to bound values mark citations stale.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), operations: patchSchema }, false, async ({ deck_id, expected_revision, operations }, signal) => {
	const state = getDeck(deck_id);
	const document = await state.transact(operations, { signal, expectedRevision: expected_revision });
	return { deck_id, revision: document.revision, hash: document.hash, stale_bindings: document.bindings.filter((binding) => binding.stale).length };
});
async function appendElement(deck_id, slide_id, element, signal) {
	const state = getDeck(deck_id); const slideIndex = state.document.deck.slides.findIndex((slide) => slide.id === slide_id);
	if (slideIndex < 0) throw new Error('Unknown slide ID');
	await state.transact([{ op: 'add', path: `/deck/slides/${slideIndex}/elements/-`, value: element }], { signal });
	return { deck_id, revision: state.revision, element_id: element.id };
}
register('add_diagram', 'Create and append a native group of labeled shapes and connected arrows through the shared core.', { deck_id: handle, slide_id: shortText, id: z.string().min(1).max(50), steps: z.array(z.string().max(80)).min(2).max(6) }, false, async ({ deck_id, slide_id, id, steps }, signal) => appendElement(deck_id, slide_id, await requestCore({ op: 'create_diagram', id, steps }, { signal }), signal));
register('add_picture', 'Decode and append a PNG/JPEG image, at most 1 MiB and 4096px. Images are embedded, never linked or executed.', { deck_id: handle, slide_id: shortText, id: z.string().min(1).max(80), base64: z.string().max(1_398_104), mime_type: z.enum(['image/png', 'image/jpeg']), alt: z.string().max(500) }, false, async ({ deck_id, slide_id, ...input }, signal) => appendElement(deck_id, slide_id, await requestCore({ op: 'create_picture', ...input }, { signal }), signal));
register('export_project', 'Save a new PPTX and hash-bound .aislide.json checkpoint in the approved output directory. Never overwrites files. Stale source citations and new-deck measured overflow block export. Publication is per-file: a concurrent collision or process crash may leave a reported incomplete pair; public paths are never deleted during recovery.', { deck_id: handle, filename: filenameSchema }, false, async ({ deck_id, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const result = await getDeck(deck_id).exportProject({ signal });
	return publishProject(outputDirectory, filename, result, signal);
});
register('open_project', 'Open only an exact PPTX/checkpoint pair after verifying both hashes and reproducing its scene. Does not read filesystem paths or fetch external content.', { base64: z.string().max(4 * 1024 * 1024), checkpoint: z.object({ format: z.literal('aislide.project'), version: z.literal(1), pptx_sha256: z.string().length(64), document: z.record(z.string(), z.unknown()) }).strict() }, false, async (input, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const state = await client.openProject(input, { signal }); const id = randomUUID(); decks.set(id, state);
	return { deck_id: id, revision: state.revision, slides: state.document.deck.slides.length };
});

server.registerResource('report-example', 'aislide://report/example', { mimeType: 'application/json' }, async () => ({ contents: [{ uri: 'aislide://report/example', mimeType: 'application/json', text: JSON.stringify(await requestCore({ op: 'sample' })) }] }));

await server.connect(new StdioServerTransport(process.stdin, process.stdout, { maxBufferSize: MAX_REQUEST_BYTES }));
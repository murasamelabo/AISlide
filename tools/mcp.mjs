import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { randomUUID, createHash } from 'node:crypto';
import { mkdir, realpath } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { requestCore, MAX_REQUEST_BYTES } from './core-client.mjs';
import { publishNewFile } from './atomic-output.mjs';

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
const handle = z.string().uuid();
const shortText = z.string().max(120);
const section = z.object({
	title: z.string().max(100),
	layout: z.enum(['cover', 'metrics', 'table', 'columns', 'statement']),
	body: z.array(z.string().max(240)).max(4).default([]),
	metrics: z.array(z.object({ label: z.string().max(48), value: z.string().max(20) }).strict()).max(4).default([]),
	rows: z.array(z.array(z.string().max(200)).min(1).max(8)).max(12).default([]),
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
		try {
			const result = await action(input, extra.signal);
			return { content: [{ type: 'text', text: JSON.stringify(result) }], structuredContent: result };
		} catch (error) {
			return { isError: true, content: [{ type: 'text', text: error instanceof Error ? error.message : 'Operation failed' }] };
		}
	});
}

register('sample_report', 'Return a twelve-slide structured example. All numbers are synthetic, and no model or network request is used.', {}, true, async (_input, signal) => requestCore({ op: 'sample' }, { signal }));
register('provider_status', 'Read operator-configured model availability and destination without API keys. This does not contact the provider or validate credentials.', {}, true, async (_input, signal) => requestCore({ op: 'provider_status' }, { signal }));
register('generate_report', 'Ask the operator-configured OpenAI-compatible model for a draft, validate its ReportInput and compile it with the Rust core. No file is saved. Content and citations remain unverified. Requires operator remote opt-in and allow_remote=true before any non-loopback request. May take up to 300 seconds; cancellation stops the provider request. Never accepts endpoints or credentials as tool arguments.', {
	prompt: z.string().min(1).max(8000),
	source_text: z.string().max(24_000).default(''),
	slide_count: z.number().int().min(1).max(32),
	allow_remote: z.boolean().default(false),
}, false, async (input, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const result = await requestCore({ op: 'generate', input }, { signal });
	if (signal.aborted) throw new Error('Operation cancelled');
	const id = randomUUID();
	decks.set(id, { deck: result.compiled.deck, revision: 0, history: [] });
	return { deck_id: id, slides: result.compiled.deck.slides.length, revision: 0, issues: result.compiled.issues, provenance: result.provenance };
});
register('compile_report', 'Compile structured content into editable text, rectangles and native tables. Returns a session deck handle. This is deterministic layout, not an AI model call; charts and images are not yet supported.', { report: reportSchema }, false, async ({ report }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const result = await requestCore({ op: 'compile', report }, { signal });
	const id = randomUUID();
	decks.set(id, { deck: result.deck, revision: 0, history: [] });
	return { deck_id: id, slides: result.deck.slides.length, revision: 0, issues: result.issues };
});
register('get_deck', 'Read the scene with stable slide and element IDs. Coordinates are in pixels on a 1280 x 720 page.', { deck_id: handle }, true, async ({ deck_id }) => getDeck(deck_id).deck);
register('update_text', 'Set a text element without re-layout. The geometry stays fixed; validate visual fit separately. Optional expected_revision protects against stale edits.', { deck_id: handle, slide_id: shortText, element_id: shortText, text: z.string().max(4000), expected_revision: z.number().int().min(0).optional() }, false, async ({ deck_id, slide_id, element_id, text, expected_revision }, signal) => {
	const state = getDeck(deck_id);
	const revision = state.revision;
	if (expected_revision !== undefined && revision !== expected_revision) throw new Error('Revision conflict');
	const next = structuredClone(state.deck);
	const element = next.slides.find((slide) => slide.id === slide_id)?.elements.find((item) => item.id === element_id);
	if (element?.type !== 'text') throw new Error('Target must be an existing text element');
	element.text = text;
	await requestCore({ op: 'validate', deck: next }, { signal });
	if (getDeck(deck_id).revision !== revision) throw new Error('Revision conflict');
	state.history = [...state.history.slice(-9), state.deck];
	state.deck = next;
	state.revision += 1;
	return { deck_id, revision: state.revision, warnings: ['Text fit is not checked by structural validation'] };
});
register('undo', 'Undo the previous in-session text change.', { deck_id: handle }, false, async ({ deck_id }) => {
	const state = getDeck(deck_id);
	const previous = state.history.pop();
	if (!previous) throw new Error('Nothing to undo');
	state.deck = previous;
	state.revision += 1;
	return { deck_id, revision: state.revision };
});
register('validate_deck', 'Validate IDs, object types, text safety and page geometry. This does not prove Office visual parity or text fit.', { deck_id: handle }, true, async ({ deck_id }, signal) => requestCore({ op: 'validate', deck: getDeck(deck_id).deck }, { signal }));
register('export_pptx', 'Save a native PPTX inside the operator-approved --output-dir. Never overwrites an existing file. A plain .pptx filename is required.', { deck_id: handle, filename: filenameSchema }, false, async ({ deck_id, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const { base64 } = await requestCore({ op: 'export', deck: getDeck(deck_id).deck }, { signal });
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

server.registerResource('report-example', 'aislide://report/example', { mimeType: 'application/json' }, async () => ({ contents: [{ uri: 'aislide://report/example', mimeType: 'application/json', text: JSON.stringify(await requestCore({ op: 'sample' })) }] }));

await server.connect(new StdioServerTransport(process.stdin, process.stdout, { maxBufferSize: MAX_REQUEST_BYTES }));
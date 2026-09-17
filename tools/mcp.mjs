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
const chartKinds = z.enum(['column', 'bar', 'line', 'pie', 'doughnut', 'area', 'scatter', 'stacked_column', 'stacked_bar']);
const colorSchema = z.string().regex(/^(?:[0-9a-fA-F]{6}|@(dk1|lt1|dk2|lt2|accent[1-6]|hlink|folHlink))$/);
const chartSchema = z.object({ kind: chartKinds, categories: z.array(z.string().max(80)).min(1).max(32), series: z.array(z.object({ name: z.string().max(80), values: z.array(z.number().min(-1e15).max(1e15)).min(1).max(32), color: colorSchema }).strict()).min(1).max(6) }).strict();
const themeSchema = z.object({ name: z.string().max(80), colors: z.record(z.string().max(16), z.string().regex(/^[0-9a-fA-F]{6}$/)), fonts: z.object({ major: z.string().min(1).max(100), minor: z.string().min(1).max(100), east_asian: z.string().min(1).max(100), complex_script: z.string().min(1).max(100) }).strict() }).strict();
const designElements = z.array(z.record(z.string(), z.unknown())).max(256);
const designSchema = z.object({ theme: themeSchema, masters: z.array(z.object({ id: z.string().min(1).max(80), name: z.string().max(100), background: colorSchema, elements: designElements }).strict()).min(1).max(8), layouts: z.array(z.object({ id: z.string().min(1).max(80), name: z.string().max(100), master_id: z.string().min(1).max(80), background: colorSchema.nullable().optional(), elements: designElements }).strict()).min(1).max(32) }).strict();
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
const slideId = z.string().min(1).max(80);
const slideOperations = z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('insert'), id: slideId, after: slideId.nullable().optional(), title: z.string().max(120), layout_id: slideId.nullable().optional() }).strict(),
	z.object({ op: z.literal('duplicate'), slide_id: slideId, id: slideId }).strict(),
	z.object({ op: z.literal('remove'), slide_id: slideId }).strict(),
	z.object({ op: z.literal('move'), slide_id: slideId, index: z.number().int().min(0).max(31) }).strict(),
	z.object({ op: z.literal('rename'), slide_id: slideId, title: z.string().max(120) }).strict(),
])).min(1).max(128);
const assetInput = { id: slideId, base64: z.string().max(1398104), mime_type: z.enum(['image/svg+xml', 'image/png', 'image/jpeg']), alt: z.string().max(500), size: z.number().min(8).max(640) };
const elementOperations = z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('duplicate'), id: slideId, new_id: z.string().min(1).max(40) }).strict(),
	z.object({ op: z.literal('remove'), id: slideId }).strict(),
	z.object({ op: z.literal('order'), id: slideId, index: z.number().int().min(0).max(255) }).strict(),
])).min(1).max(128);

const graphId = z.string().regex(/^[A-Za-z0-9_-]{1,24}$/);
const graphPort = z.enum(['auto', 'top', 'left', 'bottom', 'right']);
const graphIcon = z.object({ base64: z.string().min(1).max(1398104), mime_type: z.enum(['image/png', 'image/jpeg']), alt: z.string().max(500).optional() }).strict();
const graphNode = z.object({ id: graphId, label: z.string().max(160), kind: z.enum(['rectangle', 'rounded_rectangle', 'ellipse', 'diamond', 'cylinder', 'cloud']).optional(), x: z.number().min(0).max(1152), y: z.number().min(88).max(512), width: z.number().min(64).max(1152).optional(), height: z.number().min(40).max(424).optional(), fill: colorSchema.optional(), stroke: colorSchema.optional(), color: colorSchema.optional(), font_size: z.number().min(12).max(40).optional(), group: graphId.nullable().optional(), icon: graphIcon.nullable().optional() }).strict();
const graphEdge = z.object({ id: graphId, source: graphId, target: graphId, source_port: graphPort.optional(), target_port: graphPort.optional(), label: z.string().max(64).optional(), route: z.enum(['straight', 'elbow']).optional(), color: colorSchema.optional(), arrow: z.boolean().optional(), start_arrow: z.boolean().optional(), dashed: z.boolean().optional() }).strict();
const graphGroup = z.object({ id: graphId, label: z.string().max(64), x: z.number().min(0).max(1152), y: z.number().min(88).max(512), width: z.number().min(64).max(1152), height: z.number().min(40).max(424), fill: colorSchema.optional(), stroke: colorSchema.optional() }).strict();
const graphSpec = z.object({ version: z.literal(1), title: z.string().max(80), subtitle: z.string().max(120).optional(), nodes: z.array(graphNode).min(1).max(48), edges: z.array(graphEdge).max(64).optional(), groups: z.array(graphGroup).max(8).optional() }).strict();
const graphSelection = z.array(graphId).min(1).max(120);
const graphOperations = z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('put_node'), node: graphNode }).strict(),
	z.object({ op: z.literal('put_edge'), edge: graphEdge }).strict(),
	z.object({ op: z.literal('put_group'), group: graphGroup }).strict(),
	z.object({ op: z.literal('move'), ids: graphSelection, dx: z.number().min(-1152).max(1152), dy: z.number().min(-512).max(512) }).strict(),
	z.object({ op: z.literal('remove'), ids: graphSelection }).strict(),
	z.object({ op: z.literal('align'), ids: graphSelection, alignment: z.enum(['left', 'center', 'right', 'top', 'middle', 'bottom']) }).strict(),
	z.object({ op: z.literal('layout'), columns: z.number().int().min(1).max(8) }).strict(),
])).min(1).max(128);

const partValue = z.number().min(-1e15).max(1e15);
const partLabel = z.string().max(48);
const partItem = z.object({ label: partLabel, detail: z.string().max(120).optional(), value: partValue.nullable().optional() }).strict();
const partData = z.discriminatedUnion('kind', [
	z.object({ kind: z.literal('chart'), categories: z.array(z.string().max(32)).min(1).max(12), series: z.array(z.object({ name: z.string().max(32), values: z.array(partValue).min(1).max(12) }).strict()).min(1).max(4), x_axis: z.string().max(60).optional(), y_axis: z.string().max(60).optional() }).strict(),
	z.object({ kind: z.literal('items'), items: z.array(partItem).min(2).max(12), center: partLabel.optional() }).strict(),
	z.object({ kind: z.literal('tree'), nodes: z.array(z.object({ id: z.string().min(1).max(32), label: z.string().max(40), parent: z.string().max(32).nullable().optional() }).strict()).min(2).max(12) }).strict(),
	z.object({ kind: z.literal('network'), nodes: z.array(partItem).min(2).max(6), edges: z.array(z.object({ from: z.number().int().min(0).max(5), to: z.number().int().min(0).max(5), label: z.string().max(24).optional() }).strict()).min(1).max(12) }).strict(),
	z.object({ kind: z.literal('matrix'), rows: z.array(partLabel).min(2).max(4), columns: z.array(partLabel).min(2).max(4), cells: z.array(z.array(partLabel).min(2).max(4)).min(2).max(4) }).strict(),
	z.object({ kind: z.literal('groups'), groups: z.array(z.object({ label: z.string().max(32), items: z.array(z.string().max(40)).min(1).max(5) }).strict()).min(2).max(6) }).strict(),
	z.object({ kind: z.literal('timeline'), periods: z.array(z.string().max(16)).min(2).max(12), tasks: z.array(z.object({ label: z.string().max(32), start: z.number().int().min(0).max(11), end: z.number().int().min(1).max(12), progress: z.number().min(0).max(1).optional() }).strict()).min(1).max(8) }).strict(),
	z.object({ kind: z.literal('waterfall'), steps: z.array(z.object({ label: z.string().max(32), value: partValue, total: z.boolean().optional() }).strict()).min(2).max(10), unit: z.string().max(24).optional() }).strict(),
	z.object({ kind: z.literal('map'), points: z.array(z.object({ label: z.string().max(32), longitude: z.number().min(-180).max(180), latitude: z.number().min(-85).max(85), value: partValue.nullable().optional() }).strict()).min(1).max(8) }).strict(),
	z.object({ kind: z.literal('diagram'), graph: graphSpec }).strict(),
]);
const partSpec = z.object({ version: z.literal(1), preset: z.string().min(1).max(100), title: z.string().max(80), subtitle: z.string().max(120).optional(), data: partData }).strict();
const authoringProfile = z.enum(['consulting-decision', 'technical-explainer', 'event-talk', 'status-report']);
const evidenceId = z.string().min(1).max(40);
const guidedInput = z.object({
	version: z.literal(1), profile_id: authoringProfile, title: z.string().min(1).max(120), audience: z.string().min(1).max(240), purpose: z.string().min(1).max(600), governing_message: z.string().min(1).max(600), language: z.enum(['en', 'ja']), brand_color: z.string().regex(/^[0-9a-fA-F]{6}$/).nullable().optional(),
	evidence: z.array(z.object({ id: evidenceId, kind: z.enum(['source', 'assumption', 'unknown']), reference: z.string().min(1).max(600), statement: z.string().min(1).max(1200) }).strict()).max(64),
	issues: z.array(z.object({ id: z.string().min(1).max(32), question: z.string().min(1).max(100), requested_decision: z.string().min(1).max(120), criterion: z.string().min(1).max(120), owner: z.string().min(1).max(48), due: z.string().min(1).max(48), evidence_ids: z.array(evidenceId).min(1).max(8), analysis_slide_ids: z.array(slideId).min(1).max(12) }).strict()).max(6).optional(),
	slides: z.array(z.object({ id: slideId, section: z.string().min(1).max(80), headline: z.string().min(1).max(240), sentence_form: z.enum(['causal', 'conditional', 'contrast', 'causal-focus', 'evaluation', 'proposal', 'explanation', 'comparison', 'outcome']), pattern_id: z.string().min(1).max(80), question: z.string().min(1).max(240), parent_message: z.string().min(1).max(80), transition: z.string().min(1).max(80), parallel_basis: z.string().min(1).max(80), part: partSpec.nullable().optional(), support: z.array(z.object({ clause: z.string().min(1).max(240), body_paths: z.array(z.string().min(1).max(512)).min(1).max(16), evidence_ids: z.array(evidenceId).min(1).max(16) }).strict()).max(8), numbers: z.array(z.object({ path: z.string().min(1).max(512), value: z.union([partValue, z.string().max(120), z.boolean(), z.null()]), evidence_id: evidenceId }).strict()).max(256).optional() }).strict()).min(1).max(32),
}).strict();

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
register('create_presentation', 'Create a blank one-slide presentation with a default editable design. No synthetic report, file writes or network access.', { title: z.string().min(1).max(120) }, false, async ({ title }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const id = randomUUID(); const state = await client.createPresentation(id, title, { signal }); decks.set(id, state);
	return { deck_id: id, revision: state.revision, slides: 1 };
});
register('edit_slides', 'Insert, duplicate, remove, move or rename slides in one undoable revision. Preserve native parts and metadata; reject unsafe sections/custom shows and removing the final slide.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), operations: slideOperations }, false, async ({ deck_id, expected_revision, operations }, signal) => {
	const state = getDeck(deck_id); await state.editSlides(operations, { expectedRevision: expected_revision, signal });
	return { deck_id, revision: state.revision, slides: state.document.deck.slides.length };
});
register('create_asset', 'Validate PNG/JPEG or rasterize an inert SVG icon into a transparent PNG picture. SVG scripts, text, images, external references and unsupported effects are rejected. No raw SVG is executed or embedded.', assetInput, true, async (input, signal) => client.createAsset(input, { signal }));
register('edit_elements', 'Duplicate, remove or reorder a top-level element in one undoable revision. Preserve part metadata and bindings; reject lossy native copies and remove connectors that lose their target.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), slide_id: slideId, operations: elementOperations }, false, async ({ deck_id, expected_revision, slide_id, operations }, signal) => {
	const state = getDeck(deck_id); await state.editElements(slide_id, operations, { expectedRevision: expected_revision, signal });
	return { deck_id, revision: state.revision, slide_id };
});
register('add_asset', 'Insert an SVG/PNG/JPEG asset in one undoable revision. SVG becomes a validated PNG; no external resource fetching. Existing slide content is not rearranged.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), slide_id: slideId, ...assetInput }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
	const state = getDeck(deck_id); await state.addAsset(slide_id, input, { expectedRevision: expected_revision, signal });
	return { deck_id, revision: state.revision, element_id: input.id };
});
register('graph_catalog', 'List architecture diagram shapes, ports, routing styles, typed schemas, limits and editable examples. Does not create a document or contact a network.', {}, true, async (_input, signal) => client.graphCatalog({ signal }));
register('create_graph_icon', 'Prepare a bounded node icon from SVG, PNG or JPEG. SVG becomes transparent PNG; large rasters are fitted to a 256px longest side. Returns image data for GraphNode.icon, with no document or filesystem mutation. External resources and active SVG are rejected.', { base64: assetInput.base64, mime_type: assetInput.mime_type, alt: z.string().max(500).optional() }, true, async (input, signal) => client.createGraphIcon(input, { signal }));
register('create_graph', 'Render a typed node/edge/group graph into native editable PowerPoint objects without inserting it. Coordinates use the 1152x512 graph canvas below its 88px title band. No HTML, executable content or draw.io XML is accepted.', { id: z.string().min(1).max(40), spec: graphSpec, theme: themeSchema.optional() }, true, async (input, signal) => client.createGraph(input, { signal }));
register('transform_graph', 'Apply bounded graph operations to a candidate specification. Nodes, edges, group movement, alignment and grid layout are computed and validated in Rust. No document mutation.', { spec: graphSpec, operations: graphOperations }, true, async ({ spec, operations }, signal) => client.transformGraph(spec, operations, { signal }));
register('get_graph', 'Read one managed graph specification and stale status without returning unrelated document source data.', { deck_id: handle, slide_id: z.string().min(1).max(80), id: z.string().min(1).max(40) }, true, async ({ deck_id, slide_id, id }) => {
	const state = getDeck(deck_id); const part = state.document.parts?.find((entry) => entry.slide_id === slide_id && entry.element_id === id && entry.spec.data.kind === 'diagram');
	if (!part) throw new Error('Managed graph not found');
	return { deck_id, revision: state.revision, slide_id, element_id: id, spec: part.spec.data.graph, stale: part.stale };
});
for (const name of ['add_graph', 'update_graph', 'apply_graph']) {
	register(name, name === 'add_graph' ? 'Insert an architecture graph as native shapes and attached connectors in one undoable revision. Does not rearrange existing slide objects.' : 'Update a managed architecture graph in one undoable revision. Preserves root placement and rejects stale metadata rather than overwriting external/manual changes.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), slide_id: z.string().min(1).max(80), id: z.string().min(1).max(40), ...(name === 'apply_graph' ? { operations: graphOperations } : { spec: graphSpec }) }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
		const state = getDeck(deck_id); const options = { signal, expectedRevision: expected_revision };
		if (name === 'add_graph') await state.addGraph(slide_id, input, options);
		else if (name === 'update_graph') await state.updateGraph(slide_id, input, options);
		else await state.applyGraph(slide_id, input, options);
		return { deck_id, revision: state.revision, element_id: input.id };
	});
}
register('object_catalog', 'List native preset shapes, supported chart types and table limits. Insertion examples contain clearly named synthetic chart values.', {}, true, async (_input, signal) => client.objectCatalog({ signal }));
register('part_catalog', 'List 108 original metadata-driven presets across 36 chart and diagram categories, with synthetic examples and the core input schema. No model calls, downloads or document mutation.', {}, true, async (_input, signal) => client.partCatalog({ signal }));
register('best_practice_profiles', 'List four evidence-led authoring profiles: consulting decisions, technical explanations, event talks and reports. English guides are retrieved separately; no file or model access.', {}, true, async (_input, signal) => client.bestPracticeProfiles({ signal }));
register('best_practice_guide', 'Retrieve the English five-stage workflow, profile-specific guidance and strict creation schema. The consulting catalog retains 48 patterns with honest native-template, composition-required or guidance-only status. Read this before planning; guidance does not verify truth.', { profile_id: authoringProfile }, true, async ({ profile_id }, signal) => client.bestPracticeGuide(profile_id, { signal }));
register('validate_guided_presentation', 'Dry-run a structured outline, clause-to-body evidence, numeric source declarations and native layout. ready means compilable, not semantically proven or Office-qualified. Returns unmet checks and human-review requirements; no deck handle or file is created.', { input: guidedInput }, true, async ({ input }, signal) => client.validateGuidedPresentation(input, { signal }));
register('create_guided_presentation', 'Create a NEW evidence-led presentation from supplied claims and native parts after the same strict validation. Consulting multi-page decks require 3-6 issues, C02 summary and C03 close. Unknown numbers must remain xx in qualitative content. Does not invent content, call a model, overwrite a deck or save a file; use export_pptx separately. Review semantics and actual rendering.', { input: guidedInput }, false, async ({ input }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const id = randomUUID(); const result = await client.createGuidedPresentation(id, input, { signal });
	if (signal.aborted) throw new Error('Operation cancelled');
	decks.set(id, result.session);
	return { deck_id: id, revision: result.session.revision, slides: result.session.document.deck.slides.length, profile_id: result.profile_id, validation: result.validation, model_inference: false };
});
register('create_part', 'Preview a metadata-driven part as native editable elements without inserting it. Validates bounds, numeric meaning and references in Rust. This is deterministic design, not AI generation.', { id: z.string().min(1).max(40), spec: partSpec, theme: themeSchema.optional() }, true, async (input, signal) => client.createPart(input, { signal }));
for (const name of ['add_part', 'update_part']) {
	register(name, name === 'add_part' ? 'Insert a native metadata-driven part in one undoable revision. Presets and metadata schema come from part_catalog. Placement does not rearrange existing content.' : 'Update a part from metadata while retaining its placement, in one undoable revision. Rejects stale metadata after native edits; never restores a cached scene over external changes.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), slide_id: z.string().min(1).max(80), id: z.string().min(1).max(40), spec: partSpec }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
		const state = getDeck(deck_id);
		await (name === 'add_part' ? state.addPart(slide_id, input, { signal, expectedRevision: expected_revision }) : state.updatePart(slide_id, input, { signal, expectedRevision: expected_revision }));
		return { deck_id, revision: state.revision, element_id: input.id };
	});
}
register('design_defaults', 'Return a theme and editable native master/layout templates. Does not mutate a document.', {}, true, async (_input, signal) => client.designDefaults({ signal }));
register('design_presets', 'List seven original native master presets with theme fonts, color roles, margins, gutters and layout regions. These are deterministic designs, not AI generation or copied third-party templates.', {}, true, async (_input, signal) => ({ presets: await client.designPresets({ signal }) }));
register('apply_design_preset', 'Apply a core-owned design preset with revision checking and Undo. Retains original masters and slide content; adds or replaces the dedicated preset master and layouts. Native packages that cannot add this structure are rejected. Does not automatically rearrange freeform objects or fetch fonts.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), preset_id: z.enum(['public', 'minimal', 'stylish', 'pop', 'dynamic', 'trust', 'luxury']) }, false, async ({ deck_id, expected_revision, preset_id }, signal) => {
	const state = getDeck(deck_id); await state.applyDesignPreset(preset_id, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('update_design', 'Replace the authored master/layout design, propagate linked placeholder geometry and formatting, and retain content. Unsupported changes to imported original packages are rejected. Revision and undo use the shared transaction engine.', { deck_id: handle, expected_revision: z.number().int().min(0), design: designSchema }, false, async ({ deck_id, expected_revision, design }, signal) => {
	const state = getDeck(deck_id); await state.updateDesign(design, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('apply_theme', 'Apply twelve native theme color slots and heading/body/script fonts. Colors matching the previous theme become live references; unrelated custom colors are retained. Never installs or fetches fonts.', { deck_id: handle, expected_revision: z.number().int().min(0), theme: themeSchema }, false, async ({ deck_id, expected_revision, theme }, signal) => {
	const state = getDeck(deck_id); await state.applyTheme(theme, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision };
});
register('assign_layout', 'Apply or reset a native slide layout while preserving existing matching text. Placeholder geometry and format follow later design edits until explicitly detached.', { deck_id: handle, expected_revision: z.number().int().min(0), slide_id: shortText, layout_id: z.string().min(1).max(80) }, false, async ({ deck_id, expected_revision, slide_id, layout_id }, signal) => {
	const state = getDeck(deck_id); await state.assignLayout(slide_id, layout_id, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision };
});
register('add_object', 'Insert a core-created text box, preset shape, table, chart, line or arrow. Charts contain labeled synthetic examples, not factual data. All objects remain native and editable in PowerPoint.', { deck_id: handle, expected_revision: z.number().int().min(0), slide_id: shortText, id: z.string().min(1).max(80), kind: z.enum(['text', 'shape', 'table', 'chart', 'line', 'arrow']), preset: z.string().max(80).optional(), rows: z.number().int().min(1).max(12).optional(), columns: z.number().int().min(1).max(8).optional() }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
	const state = getDeck(deck_id); await state.addObject(slide_id, input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, element_id: input.id };
});
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
register('open_pptx', 'Open one standard unencrypted Open XML PPTX without a checkpoint. Current native XML is authoritative; embedded provenance is optional. Unknown parts are preserved and unsupported edits fail. Encrypted/protected OLE containers and legacy .ppt are rejected without removing protection.', { base64: z.string().max(3_900_000) }, false, async ({ base64 }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const id = randomUUID(); const opened = await client.openPresentation(id, base64, { signal }); decks.set(id, opened.session);
	return { deck_id: id, revision: 0, slides: opened.session.document.deck.slides.length, format: opened.format, warnings: opened.warnings, objects: opened.objects };
});
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
	 if (elementIndex === undefined || !['text', 'shape'].includes(deck.slides[slideIndex].elements[elementIndex]?.type)) throw new Error('Target must be an existing top-level text or shape element');
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
register('export_pptx', 'Save one native PPTX including optional sources/bindings in internal Custom XML, without an external checkpoint. Uses the operator-approved --output-dir and never overwrites an existing file. A plain .pptx filename is required.', { deck_id: handle, filename: filenameSchema }, false, async ({ deck_id, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	 const { base64 } = await getDeck(deck_id).exportPresentation({ signal });
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
const mappingSchema = z.object({ title: shortText, period: z.string().max(80), table_index: z.number().int().min(0).max(7), category_column: z.number().int().min(0).max(31), value_columns: z.array(z.number().int().min(0).max(31)).min(1).max(6), row_start: z.number().int().min(0).max(999), row_count: z.number().int().min(1).max(32), chart_kind: chartKinds }).strict();
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
register('export_project', 'Legacy compatibility only: save a new PPTX and hash-bound .aislide.json checkpoint. Prefer export_pptx for single-file operation. Never overwrites files; pair publication is not crash-atomic.', { deck_id: handle, filename: filenameSchema }, false, async ({ deck_id, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const result = await getDeck(deck_id).exportProject({ signal });
	return publishProject(outputDirectory, filename, result, signal);
});
register('open_project', 'Legacy compatibility only: open an exact PPTX/checkpoint pair. Prefer open_pptx for a standalone presentation. Does not read paths, fetch content or ignore integrity failures.', { base64: z.string().max(4 * 1024 * 1024), checkpoint: z.object({ format: z.literal('aislide.project'), version: z.literal(1), pptx_sha256: z.string().length(64), document: z.record(z.string(), z.unknown()) }).strict() }, false, async (input, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const state = await client.openProject(input, { signal }); const id = randomUUID(); decks.set(id, state);
	return { deck_id: id, revision: state.revision, slides: state.document.deck.slides.length };
});

server.registerResource('report-example', 'aislide://report/example', { mimeType: 'application/json' }, async () => ({ contents: [{ uri: 'aislide://report/example', mimeType: 'application/json', text: JSON.stringify(await requestCore({ op: 'sample' })) }] }));

await server.connect(new StdioServerTransport(process.stdin, process.stdout, { maxBufferSize: MAX_REQUEST_BYTES }));
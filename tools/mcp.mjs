import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { randomUUID, createHash } from 'node:crypto';
import { mkdir, realpath, lstat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { requestCore, MAX_REQUEST_BYTES } from './core-client.mjs';
import { CAPACITY_PROFILES, FONT_LIMITS } from '../packages/client/index.mjs';
import { publishNewFile, publishNewBundle, BundlePublicationError } from './atomic-output.mjs';
import { publishProject } from './atomic-project.mjs';
import { AislideClient } from '../packages/client/index.mjs';

const serverInfo = { name: 'aislide', version: '0.1.0' };
const server = new McpServer(serverInfo);
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
const revisionCandidates = new Map();
const candidateLifetimeMs = 10 * 60 * 1000;
const client = new AislideClient(requestCore);
const imageResult = Symbol('imageResult');
let activeMutation = false;
const handle = z.string().uuid();
const shortText = z.string().max(120);
const fontBase64 = z.string().max(Math.ceil(FONT_LIMITS.face_bytes / 3) * 4);
const capacityProfileSchema = z.enum(['legacy', 'standard', 'large']);
const archiveBase64 = z.string().max(Math.ceil(CAPACITY_PROFILES.large.archive_bytes / 3) * 4);
const chartKinds = z.enum(['column', 'bar', 'line', 'pie', 'doughnut', 'area', 'scatter', 'stacked_column', 'stacked_bar', 'percent_stacked_column', 'percent_stacked_bar', 'combo', 'bubble', 'radar', 'radar_filled', 'column3d', 'bar3d', 'pie3d', 'funnel', 'waterfall', 'histogram', 'box_whisker', 'treemap', 'sunburst']);
const colorSchema = z.string().regex(/^(?:[0-9a-fA-F]{6}|@(dk1|lt1|dk2|lt2|accent[1-6]|hlink|folHlink))$/);
const numeric = z.number().min(-1e15).max(1e15);
const optional = (schema) => schema.nullable().optional();
const axisSchema = z.object({ min: optional(numeric), max: optional(numeric), major_unit: optional(numeric.positive()), minor_unit: optional(numeric.positive()), log_base: optional(z.number().min(2).max(1000)), reverse: z.boolean().optional(), number_format: optional(z.string().max(100)) }).strict();
const labelsSchema = z.object({ show_value: z.boolean().optional(), show_category_name: z.boolean().optional(), show_series_name: z.boolean().optional(), show_percent: z.boolean().optional(), position: optional(z.enum(['center', 'inside_end', 'outside_end', 'best_fit'])), number_format: optional(z.string().max(100)) }).strict();
const histogramOptionsSchema = z.object({ samples: z.array(numeric).min(1).max(4096), binning: z.discriminatedUnion('rule', [z.object({ rule: z.literal('count'), count: z.number().int().min(1).max(128) }).strict(), z.object({ rule: z.literal('width'), width: numeric.positive() }).strict()]), interval_closed: z.enum(['left', 'right']), underflow: optional(numeric), overflow: optional(numeric) }).strict();
const boxWhiskerOptionsSchema = z.object({ samples: z.array(z.array(numeric).min(4).max(4096)).min(1).max(32).refine((groups) => groups.reduce((total, group) => total + group.length, 0) <= 4096, 'At most 4096 total samples'), quartile_method: z.enum(['inclusive', 'exclusive']), mean_line: z.boolean(), mean_marker: z.boolean(), nonoutliers: z.boolean(), outliers: z.boolean() }).strict();
const hierarchyOptionsSchema = z.object({ paths: z.array(z.array(z.string().min(1).max(80)).min(2).max(4)).min(1).max(32), parent_labels: optional(z.enum(['none', 'banner', 'overlapping'])) }).strict();
const chartOptionsSchema = z.object({ primary_axis: axisSchema.optional(), secondary_axis: axisSchema.optional(), category_axis: axisSchema.optional(), legend: optional(z.enum(['bottom', 'top', 'left', 'right', 'top_right', 'hidden'])), data_labels: optional(labelsSchema), waterfall_totals: optional(z.array(z.number().int().min(0).max(31)).max(32)), histogram: optional(histogramOptionsSchema), box_whisker: optional(boxWhiskerOptionsSchema), hierarchy: optional(hierarchyOptionsSchema) }).strict();
const trendlineSchema = z.object({ kind: z.enum(['linear', 'exponential', 'logarithmic', 'polynomial', 'power', 'moving_average']), order: optional(z.number().int().min(2).max(6)), period: optional(z.number().int().min(2).max(255)), intercept: optional(numeric), forward: optional(numeric.nonnegative()), backward: optional(numeric.nonnegative()), display_equation: z.boolean().optional(), display_r_squared: z.boolean().optional() }).strict();
const errorBarsSchema = z.object({ kind: z.enum(['fixed_value', 'percentage', 'standard_deviation', 'standard_error', 'custom']), direction: z.enum(['x', 'y']).optional(), bar_type: z.enum(['both', 'plus', 'minus']).optional(), value: optional(numeric.nonnegative()), plus: optional(z.array(numeric.nonnegative()).max(32)), minus: optional(z.array(numeric.nonnegative()).max(32)) }).strict();
const chartSeriesSchema = z.object({ name: z.string().max(80), values: z.array(numeric).max(32), color: colorSchema, kind: optional(chartKinds), axis: optional(z.enum(['primary', 'secondary'])), bubble_sizes: optional(z.array(numeric.positive()).min(1).max(32)), trendline: optional(trendlineSchema), error_bars: optional(errorBarsSchema) }).strict();
const chartSchema = z.object({ kind: chartKinds, categories: z.array(z.string().max(80)).max(32), series: z.array(chartSeriesSchema).min(1).max(6), options: chartOptionsSchema.optional() }).strict();
const themeSchema = z.object({ name: z.string().max(80), colors: z.record(z.string().max(16), z.string().regex(/^[0-9a-fA-F]{6}$/)), fonts: z.object({ major: z.string().min(1).max(100), minor: z.string().min(1).max(100), east_asian: z.string().min(1).max(100), complex_script: z.string().min(1).max(100) }).strict() }).strict();
const runStyleSchema = z.object({ bold: optional(z.boolean()), italic: optional(z.boolean()), underline: optional(z.boolean()), font_size: optional(z.number().min(1).max(400)), color: optional(colorSchema), font_family: optional(z.string().min(1).max(100)), baseline: optional(z.number().int().min(-100000).max(100000)), highlight: optional(z.union([colorSchema, z.literal('none')])), language: optional(z.string().min(1).max(64).regex(/^[A-Za-z0-9-]+$/)) }).strict();
const spacingSchema = z.discriminatedUnion('kind', [z.object({ kind: z.literal('percent'), value: z.number().int().min(0).max(1000000) }).strict(), z.object({ kind: z.literal('points'), value: z.number().int().min(0).max(158400) }).strict()]);
const fieldUuid = z.string().min(36).max(38).regex(/^(?:[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}|\{[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}\})$/);
const fieldSchema = z.object({ id: fieldUuid, kind: z.string().min(1).max(128).refine(value => !/\p{Cc}/u.test(value), 'Field kinds cannot contain control characters') }).strict();
const paragraphSchema = z.object({
	runs: z.array(z.object({ text: z.string().max(4000), style: runStyleSchema.optional(), field: optional(fieldSchema) }).strict()).max(4096),
	alignment: optional(z.enum(['left', 'center', 'right', 'justify'])), bullet: optional(z.enum(['none', 'bullet', 'numbered'])), bullet_character: optional(z.string().min(1).max(2)), numbering: optional(z.enum(['arabicPeriod', 'arabicParenR', 'arabicParenBoth', 'arabicPlain', 'alphaLcPeriod', 'alphaUcPeriod', 'alphaLcParenR', 'alphaUcParenR', 'romanLcPeriod', 'romanUcPeriod'])), number_start: optional(z.number().int().min(1).max(32767)), level: optional(z.number().int().min(0).max(8)), margin_left: optional(z.number().int().min(0).max(51206400)), indent: optional(z.number().int().min(-51206400).max(51206400)), line_spacing: optional(spacingSchema), space_before: optional(spacingSchema), space_after: optional(spacingSchema), tabs: z.array(z.object({ position: z.number().int().min(0).max(51206400), alignment: z.enum(['left', 'center', 'right', 'decimal']).optional() }).strict()).max(32).optional(),
}).strict();
const paragraphsSchema = z.array(paragraphSchema).max(256);
const notesParagraphsSchema = z.array(paragraphSchema.extend({ runs: z.array(z.object({ text: z.string().max(16000).refine(value => Array.from(value).length <= 8000, 'Notes exceed 8000 Unicode scalars'), style: runStyleSchema.optional(), field: optional(fieldSchema) }).strict()).max(4096) })).max(256);
const proofLanguageSchema = z.string().min(2).max(64).regex(/^[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*$/);
const proofDictionarySchema = z.object({ language: proofLanguageSchema, words: z.array(z.string().min(1).max(128)).max(10000), synonyms: z.record(z.string().min(1).max(128), z.array(z.string().min(1).max(128)).max(16)).optional(), translations: z.record(proofLanguageSchema, z.record(z.string().min(1).max(128), z.string().min(1).max(128))).optional() }).strict();
const textFormatSchema = z.object({ italic: z.boolean().optional(), underline: z.boolean().optional(), alignment: z.enum(['left', 'center', 'right', 'justify']).optional(), vertical: z.enum(['top', 'middle', 'bottom']).optional(), bullet: z.enum(['none', 'bullet', 'numbered']).optional(), font_family: optional(z.string().min(1).max(100)), hyperlink: optional(z.string().max(2048)), placeholder: optional(z.object({ kind: z.enum(['title', 'body', 'subtitle', 'footer', 'date', 'slide_number']), index: z.number().int().min(0).max(4294967295) }).strict()), inherit_layout: z.boolean().optional(), paragraphs: paragraphsSchema.optional() }).strict();
const dimensionsSchema = z.object({ unit: z.enum(['relative', 'absolute']), values: z.array(z.number().min(0.000001).max(1000000)).min(1).max(12) }).strict();
const rowIndex = z.number().int().min(0).max(11);
const columnIndex = z.number().int().min(0).max(7);
const mergeSchema = z.object({ row: rowIndex, column: columnIndex, row_span: z.number().int().min(1).max(12), col_span: z.number().int().min(1).max(8) }).strict();
const padding = z.number().min(0).max(4096);
const cellStyleSchema = z.object({ fill: optional(z.union([colorSchema, z.literal('none')])), outline: optional(z.object({ color: colorSchema, width: z.number().min(0).max(20) }).strict()), padding: optional(z.object({ left: padding, right: padding, top: padding, bottom: padding }).strict()), vertical: optional(z.enum(['top', 'middle', 'bottom'])), text_style: optional(runStyleSchema), text_format: optional(textFormatSchema) }).strict();
const tableFormatSchema = z.object({ column_widths: optional(dimensionsSchema), row_heights: optional(dimensionsSchema), merges: z.array(mergeSchema).max(96).optional(), cells: z.array(z.object({ row: rowIndex, column: columnIndex, style: cellStyleSchema }).strict()).max(96).optional() }).strict();
const alpha = z.number().min(0).max(1);
const stopsSchema = z.array(z.object({ offset: alpha, color: colorSchema, opacity: alpha }).strict()).min(2).max(16);
const pointSchema = z.tuple([alpha, alpha]);
const pathSchema = z.object({ commands: z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('move'), point: pointSchema }).strict(),
	z.object({ op: z.literal('line'), point: pointSchema }).strict(),
	z.object({ op: z.literal('quadratic'), control: pointSchema, point: pointSchema }).strict(),
	z.object({ op: z.literal('cubic'), control1: pointSchema, control2: pointSchema, point: pointSchema }).strict(),
	z.object({ op: z.literal('close') }).strict(),
])).min(3).max(256) }).strict();
const visualSchema = z.object({ rotation: optional(z.number().min(-360).max(360)), flip_h: z.boolean().optional(), flip_v: z.boolean().optional(), hidden: z.boolean().optional(), locked: z.boolean().optional(), opacity: optional(alpha), gradient: optional(z.discriminatedUnion('kind', [z.object({ kind: z.literal('linear'), angle: z.number().min(0).max(360), stops: stopsSchema }).strict(), z.object({ kind: z.literal('radial'), center: z.tuple([alpha, alpha]), stops: stopsSchema }).strict()])), shadow: optional(z.object({ color: colorSchema, opacity: alpha, blur: z.number().min(0).max(100), distance: z.number().min(0).max(200), angle: z.number().min(0).max(360) }).strict()), glow: optional(z.object({ color: colorSchema, opacity: alpha, radius: z.number().min(0).max(100) }).strict()), soft_edge: optional(z.number().min(0).max(100)), reflection: optional(z.object({ blur: z.number().min(0).max(100), distance: z.number().min(0).max(200), start_opacity: alpha, end_opacity: alpha, end_position: alpha }).strict()), text_warp: optional(z.enum(['arch_up', 'arch_down', 'wave1', 'wave2', 'inflate', 'deflate', 'slant_up', 'slant_down'])), adjustments: z.array(z.object({ name: z.literal('adj'), value: z.number().int().min(0).max(100000) }).strict()).max(1).optional(), picture_mask: optional(z.enum(['ellipse', 'round_rect', 'diamond', 'hexagon'])) }).strict();
const currentVisualSchema = visualSchema.extend({ path: optional(pathSchema), connection_sites: z.array(z.object({ x: alpha, y: alpha, angle: z.number().min(0).max(360) }).strict()).max(128).optional() }).strict();
const paintEffectsSchema = visualSchema.pick({ shadow: true, glow: true, soft_edge: true, reflection: true }).strict();
const surfaceStyleSchema = z.object({ fill: z.union([colorSchema,z.literal('none')]), stroke: colorSchema.nullable(), stroke_width: z.number().min(0).max(20).nullable(), opacity: alpha.nullable(), gradient: visualSchema.shape.gradient }).strict();
const paintedCellSchema = cellStyleSchema.extend({ text_style: optional(runStyleSchema.omit({ highlight: true, language: true })), text_format: optional(textFormatSchema.extend({ paragraphs: z.array(paragraphSchema.extend({ runs: z.array(z.never()).length(0) })).max(1).optional(), hyperlink: z.null().optional(), placeholder: z.null().optional(), inherit_layout: z.literal(false).optional() })) }).strict();
const formatSnapshotSchema = z.union([
	z.object({ run: runStyleSchema.omit({ highlight: true, language: true }), paragraph: paragraphSchema.extend({ runs: z.array(z.never()).length(0) }), vertical: z.enum(['top', 'middle', 'bottom']), surface: optional(surfaceStyleSchema), effects: optional(paintEffectsSchema), text_warp: visualSchema.shape.text_warp }).strict(),
	z.object({ kind: z.literal('filled'), surface: surfaceStyleSchema, effects: paintEffectsSchema }).strict(),
	z.object({ kind: z.literal('picture'), opacity: alpha.nullable(), effects: paintEffectsSchema }).strict(),
	z.object({ kind: z.literal('table'), font_size: z.number().min(8).max(120), rows: z.number().int().min(1).max(12), columns: z.number().int().min(1).max(8), cells: z.array(z.object({ row: rowIndex,column: columnIndex,style: paintedCellSchema }).strict()).max(96) }).strict(),
	z.object({ kind: z.literal('chart'), colors: z.array(colorSchema).min(1).max(6), legend: chartOptionsSchema.shape.legend, data_labels: optional(labelsSchema), axis_formats: z.tuple([z.string().max(100).nullable(),z.string().max(100).nullable(),z.string().max(100).nullable()]) }).strict(),
]);
const svgSchema = z.string().min(1).max(349528).regex(/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/);
const boundsSchema = { id: z.string().min(1).max(80), x: z.number().min(0).max(4096), y: z.number().min(0).max(4096), width: z.number().positive().max(4096), height: z.number().positive().max(4096) };
const textFields = { text: z.string().max(4000), font_size: z.number().min(8).max(120), color: colorSchema, bold: z.boolean(), format: textFormatSchema.optional() };
const connectionSchema = z.object({ element_id: z.string().min(1).max(80), site: z.number().int().min(0).max(4294967295) }).strict();
const cropSchema = z.object({ left: alpha.optional(), right: alpha.optional(), top: alpha.optional(), bottom: alpha.optional() }).strict();
const connectorRoutingSchema = z.union([
	z.object({ points: z.array(pointSchema).min(2).max(4), start_arrow: z.boolean().optional(), dashed: z.boolean().optional(), custom: z.literal(false).optional() }).strict(),
	z.object({ points: z.array(pointSchema).min(2).max(18), start_arrow: z.boolean().optional(), dashed: z.boolean().optional(), custom: z.literal(true) }).strict(),
]);
const frameSchema = z.object(boundsSchema).omit({ id: true }).strict();
const connectorSettingsSchema = z.object({ color: colorSchema, stroke_width: z.number().min(0).max(20), arrow: z.boolean(), flip_v: z.boolean().optional(), start: optional(connectionSchema), end: optional(connectionSchema), routing: optional(connectorRoutingSchema) }).strict();
const elementSchema = z.lazy(() => z.discriminatedUnion('type', [
	z.object({ type: z.literal('text'), ...boundsSchema, ...textFields, visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('rect'), ...boundsSchema, fill: colorSchema, visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('polygon'), ...boundsSchema, points: z.array(pointSchema).min(3).max(4096), fill: z.union([colorSchema, z.literal('none')]), stroke: colorSchema, stroke_width: z.number().min(0).max(20), visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('shape'), ...boundsSchema, ...textFields, preset: z.string().min(1).max(80), fill: z.union([colorSchema, z.literal('none')]), stroke: colorSchema, stroke_width: z.number().min(0).max(20), rotation: z.number().min(-360).max(360).optional(), visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('table'), ...boundsSchema, rows: z.array(z.array(z.string().max(200)).min(1).max(8)).min(1).max(12), font_size: z.number().min(8).max(120), format: tableFormatSchema.optional() }).strict(),
	z.object({ type: z.literal('chart'), ...boundsSchema, ...chartSchema.shape }).strict(),
	z.object({ type: z.literal('picture'), ...boundsSchema, base64: z.string().max(1398104), mime_type: z.enum(['image/png', 'image/jpeg']), alt: z.string().max(500), crop: cropSchema.optional(), visual: optional(currentVisualSchema), svg: optional(svgSchema) }).strict(),
	z.object({ type: z.literal('connector'), ...boundsSchema, ...connectorSettingsSchema.shape, visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('group'), ...boundsSchema, view_width: z.number().positive().max(4096), view_height: z.number().positive().max(4096), children: z.array(elementSchema).min(1).max(256), visual: optional(currentVisualSchema) }).strict(),
]));
const designElements = z.array(elementSchema).max(256);
const designSchema = z.object({ theme: themeSchema, masters: z.array(z.object({ id: z.string().min(1).max(80), name: z.string().max(100), background: colorSchema, elements: designElements, theme: optional(themeSchema) }).strict()).min(1).max(8), layouts: z.array(z.object({ id: z.string().min(1).max(80), name: z.string().max(100), master_id: z.string().min(1).max(80), background: colorSchema.nullable().optional(), elements: designElements }).strict()).min(1).max(32) }).strict();
const auxiliaryMasterSchema = z.object({ name: z.string().max(100), background: colorSchema, theme: themeSchema, elements: designElements }).strict();
const auxiliaryDesignSchema = z.object({ width: z.number().int().min(320).max(4096), height: z.number().int().min(320).max(4096), notes_master: optional(auxiliaryMasterSchema), handout_master: optional(auxiliaryMasterSchema) }).strict();
const section = z.object({
	title: z.string().max(100),
	layout: z.enum(['cover', 'metrics', 'table', 'columns', 'statement', 'chart', 'process']),
	body: z.array(z.string().max(240)).max(4).default([]),
	metrics: z.array(z.object({ label: z.string().max(48), value: z.string().max(20) }).strict()).max(4).default([]),
	rows: z.array(z.array(z.string().max(200)).min(1).max(8)).max(12).default([]),
	chart: chartSchema.omit({ options: true }).optional(),
}).strict();
const reportSchema = z.object({ title: shortText, subtitle: z.string().max(200), period: z.string().max(80), source: z.string().max(1200), sections: z.array(section).min(1).max(CAPACITY_PROFILES.large.slides) }).strict();
const filenameSchema = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,79}\.pptx$/).refine((name) => !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])\./i.test(name), 'Reserved Windows filename');
const staticFilename = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,79}\.(?:pdf|png|jpg)$/).refine((name) => !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])\./i.test(name), 'Reserved Windows filename');
const staticOptions = z.object({
	format: z.enum(['pdf', 'png', 'jpeg']), page_indices: z.array(z.number().int().min(0).max(CAPACITY_PROFILES.large.slides - 1)).min(1).max(32).nullable().optional(),
	scale: z.number().min(0.01).max(16).optional(), transparent: z.boolean().optional(), jpeg_quality: z.number().int().min(1).max(100).optional(),
	jpeg_matte: z.tuple([z.number().int().min(0).max(255), z.number().int().min(0).max(255), z.number().int().min(0).max(255)]).optional(),
	max_output_bytes: z.number().int().min(1).max(33554432).optional(), deny_warnings: z.boolean().optional(),
}).strict();
const recoveryJson = z.string().min(2).max(48 * 1048576).refine(value => Buffer.byteLength(value) <= 48 * 1048576, 'Recovery snapshot exceeds 48 MiB');
const previewOptions = z.object({
	page_indices: z.array(z.number().int().min(0).max(CAPACITY_PROFILES.large.slides - 1)).min(1).max(8).nullable().optional(),
	max_dimension: z.number().int().min(160).max(1600).optional(), layout: z.enum(['pages', 'contact_sheet']).optional(),
	max_output_bytes: z.number().int().min(1).max(2 * 1048576).optional(),
	format: z.enum(['png', 'jpeg']).optional(), overflow: z.enum(['shrink', 'error']).optional(),
}).strict();
const deliveryName = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,59}$/).refine(name => !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i.test(name), 'Reserved Windows filename');
const deliveryOptions = z.object({
	page_indices: previewOptions.shape.page_indices, pdf: z.boolean().optional(), preview: z.enum(['none', 'pages', 'contact_sheet']).optional(),
	notes: z.boolean().optional(), source_report: z.boolean().optional(), preflight: z.boolean().optional(),
	max_dimension: previewOptions.shape.max_dimension, min_font_size: z.number().min(8).max(48).optional(), max_output_bytes: z.number().int().min(1).max(32 * 1048576).optional(),
}).strict();
const slideId = z.string().min(1).max(80);
const commentSchema = z.object({ id: slideId, author: z.string().min(1).max(256), initials: z.string().max(64), timestamp: z.string().min(19).max(35), text: z.string().max(8000), x: z.number().int().min(-2147483648).max(2147483647).optional(), y: z.number().int().min(-2147483648).max(2147483647).optional(), parent_id: optional(slideId), resolved: z.boolean().optional(), native_author_id: optional(z.number().int().min(0).max(4294967295)), native_index: optional(z.number().int().min(0).max(4294967295)) }).strict();
const localCommentSchema = commentSchema.omit({ parent_id: true, native_author_id: true, native_index: true }).strict();
const accessibilitySchema = z.object({ title: optional(z.string().max(500)), description: optional(z.string().max(4000)), decorative: optional(z.boolean()) }).strict();
const readingOrderSchema = z.array(slideId).max(256);
const modernGuid = z.string().regex(/^\{[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\}$/);
const modernStatus = z.enum(['active', 'resolved', 'closed']);
const tableHeaders = z.enum(['unknown', 'none', 'first_row', 'first_column', 'both']);
const modernAuthor = z.object({ id: modernGuid, name: z.string().min(1).max(256), user_id: z.string().min(1).max(256), provider_id: z.string().min(1).max(256), initials: optional(z.string().max(64)) }).strict();
const modernAnchor = z.discriminatedUnion('kind', [z.object({ kind: z.literal('unknown') }).strict(), z.object({ kind: z.literal('preserved') }).strict(), z.object({ kind: z.literal('slide'), slide_id: z.number().int().nonnegative(), slide_creation_id: z.number().int().nonnegative() }).strict(), z.object({ kind: z.literal('shape'), slide_id: z.number().int().nonnegative(), slide_creation_id: z.number().int().nonnegative(), shape_id: z.number().int().nonnegative(), shape_creation_id: modernGuid }).strict(), z.object({ kind: z.literal('text_range'), slide_id: z.number().int().nonnegative(), shape_id: z.number().int().nonnegative(), start: z.number().int().nonnegative(), end: z.number().int().nonnegative() }).strict()]);
const modernReply = z.object({ id: modernGuid, author: modernAuthor, created: z.string().min(19).max(35), status: modernStatus, body: paragraphsSchema }).strict();
const modernThread = modernReply.extend({ anchor: modernAnchor, replies: z.array(modernReply).max(1024) }).strict();
const modernDraft = z.object({ author_name: z.string().min(1).max(256), initials: optional(z.string().max(64)), created: z.string().min(19).max(35), body: paragraphsSchema }).strict();
const modernOperation = z.discriminatedUnion('type', [
	z.object({ type: z.literal('create'), draft: modernDraft, anchor: z.object({ kind: z.literal('unknown') }).strict() }).strict(),
	z.object({ type: z.literal('reply'), thread_id: modernGuid, draft: modernDraft }).strict(),
	z.object({ type: z.literal('set_status'), comment_id: modernGuid, status: modernStatus }).strict(),
	z.object({ type: z.literal('update_body'), comment_id: modernGuid, body: paragraphsSchema }).strict(),
	z.object({ type: z.literal('remove'), comment_id: modernGuid }).strict(),
]);
const reviewSchema = z.object({ comments: z.array(commentSchema).max(512).optional(), modern_threads: optional(z.array(modernThread).max(512)), table_headers: z.record(slideId, tableHeaders).optional(), accessibility: z.record(slideId, accessibilitySchema).optional(), reading_order: readingOrderSchema.optional() }).strict();
const slideSchema = z.object({ id: slideId, title: z.string().max(120), background: colorSchema, elements: designElements, notes: z.string().max(16000).refine(value => Array.from(value).length <= 8000), notes_paragraphs: notesParagraphsSchema.optional(), layout_id: optional(slideId), inherit_background: z.boolean().optional(), hide_master_graphics: z.boolean().optional(), native_source_id: optional(slideId), review: optional(reviewSchema) }).strict();
const embeddedFontSchema = z.object({ family: z.string().min(1).max(128), style: z.enum(['regular', 'bold', 'italic', 'bold_italic']), base64: fontBase64, license_acknowledged: z.boolean() }).strict();
const deckSchema = z.object({ version: z.literal(1), title: shortText, width: z.number().int().min(320).max(4096), height: z.number().int().min(320).max(4096), slides: z.array(slideSchema).min(1).max(CAPACITY_PROFILES.large.slides), design: optional(designSchema), auxiliary_design: optional(auxiliaryDesignSchema), embedded_fonts: z.array(embeddedFontSchema).max(8).optional() }).strict();
const inspectionCategory = z.enum(['sources', 'custom_xml', 'comments', 'notes', 'unused_media', 'off_slide', 'invisible']);
const cleanCategories = z.array(inspectionCategory).min(1).max(7).refine((categories) => new Set(categories).size === categories.length, 'Categories must be unique');
const slideOperations = z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('insert'), id: slideId, after: slideId.nullable().optional(), title: z.string().max(120), layout_id: slideId.nullable().optional() }).strict(),
	z.object({ op: z.literal('duplicate'), slide_id: slideId, id: slideId }).strict(),
	z.object({ op: z.literal('remove'), slide_id: slideId }).strict(),
	z.object({ op: z.literal('move'), slide_id: slideId, index: z.number().int().min(0).max(CAPACITY_PROFILES.large.slides - 1) }).strict(),
	z.object({ op: z.literal('rename'), slide_id: slideId, title: z.string().max(120) }).strict(),
])).min(1).max(128);
const assetInput = { id: slideId, base64: z.string().max(1398104), mime_type: z.enum(['image/svg+xml', 'image/png', 'image/jpeg', 'image/emf', 'image/wmf']), alt: z.string().max(500), size: z.number().min(8).max(640) };
const elementOperations = z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('duplicate'), id: slideId, new_id: z.string().min(1).max(40) }).strict(),
	z.object({ op: z.literal('remove'), id: slideId }).strict(),
	z.object({ op: z.literal('order'), id: slideId, index: z.number().int().min(0).max(255) }).strict(),
])).min(1).max(128);

const searchSchema = z.object({ query: z.string().min(1).max(8000), case_sensitive: z.boolean().optional(), whole_word: z.boolean().optional(), include_notes: z.boolean().optional(), include_masters: z.boolean().optional(), max_matches: z.number().int().min(1).max(1024).optional() }).strict();
const matchSchema = z.object({ path: z.string().max(2048), start: z.number().int().min(0).max(8000), end: z.number().int().min(0).max(8000), expected: z.string().max(8000), snippet: z.string().max(320) }).strict();
const replaceSchema = z.object({ search: searchSchema, replacement: z.string().max(8000), replace_all: z.boolean().optional(), selected: optional(z.array(matchSchema).max(1024)) }).strict();
const selectedIds = z.array(slideId).min(1).max(256);
const clipboardFormat = z.literal('keep_source_formatting');
const selectionSchema = z.discriminatedUnion('op', [
	z.object({ op: z.literal('translate'), ids: selectedIds, dx: numeric, dy: numeric }).strict(),
	z.object({ op: z.literal('resize'), ids: selectedIds, x: padding, y: padding, width: padding.positive(), height: padding.positive() }).strict(),
	z.object({ op: z.literal('align'), ids: selectedIds, alignment: z.enum(['left', 'center', 'right', 'top', 'middle', 'bottom']), relative_to: z.enum(['selection', 'page']) }).strict(),
	z.object({ op: z.literal('distribute'), ids: selectedIds, axis: z.enum(['horizontal', 'vertical']), relative_to: z.enum(['selection', 'page']) }).strict(),
	z.object({ op: z.literal('group'), ids: selectedIds, group_id: slideId }).strict(),
	z.object({ op: z.literal('ungroup'), ids: selectedIds }).strict(),
	z.object({ op: z.literal('copy'), ids: selectedIds, format: clipboardFormat }).strict(),
	z.object({ op: z.literal('cut'), ids: selectedIds, format: clipboardFormat }).strict(),
	z.object({ op: z.literal('paste'), id_prefix: z.string().min(1).max(60), dx: numeric, dy: numeric }).strict(),
]);
const clipboardSourceSchema = z.object({ id: slideId, hash: z.string().regex(/^[0-9a-fA-F]{64}$/), origin_sha256: optional(z.string().regex(/^[0-9a-fA-F]{64}$/)) }).strict();
const clipboardSchema = z.object({ version: z.literal(1), format: clipboardFormat, source_slide_id: slideId, source_theme: themeSchema, elements: z.array(elementSchema).min(1).max(256), source_document: optional(clipboardSourceSchema) }).strict();
const tableOperationsSchema = z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('update_format'), format: tableFormatSchema }).strict(),
	z.object({ op: z.literal('set_cell_style'), row: rowIndex, column: columnIndex, style: cellStyleSchema }).strict(),
	z.object({ op: z.literal('set_cell_text'), row: rowIndex, column: columnIndex, text: z.string().max(200) }).strict(),
	z.object({ op: z.literal('merge'), region: mergeSchema }).strict(),
	z.object({ op: z.literal('split'), row: rowIndex, column: columnIndex }).strict(),
	z.object({ op: z.literal('insert_row'), index: z.number().int().min(0).max(12), values: z.array(z.string().max(200)).min(1).max(8) }).strict(),
	z.object({ op: z.literal('insert_column'), index: z.number().int().min(0).max(8), values: z.array(z.string().max(200)).min(1).max(12) }).strict(),
	z.object({ op: z.literal('remove_row'), index: rowIndex }).strict(),
	z.object({ op: z.literal('remove_column'), index: columnIndex }).strict(),
])).min(1).max(128);
const rgbSchema = z.tuple([z.number().int().min(0).max(255), z.number().int().min(0).max(255), z.number().int().min(0).max(255)]);
const imageParamsSchema = z.object({ brightness: z.number().min(-1).max(1).optional(), contrast: z.number().min(0).max(4).optional(), saturation: z.number().min(0).max(4).optional(), grayscale: z.boolean().optional(), background_key: optional(z.object({ color: rgbSchema, tolerance: z.number().min(0).max(255) }).strict()), resize_longest_side: optional(z.number().int().min(1).max(4096)), format: z.discriminatedUnion('kind', [z.object({ kind: z.literal('png') }).strict(), z.object({ kind: z.literal('jpeg'), quality: z.number().int().min(1).max(100), matte: optional(rgbSchema) }).strict()]).optional() }).strict();
const preparedImageSchema = z.object({ base64: z.string().max(1398104), mime_type: z.enum(['image/png', 'image/jpeg']), width: z.number().int().min(1).max(4096), height: z.number().int().min(1).max(4096) }).strict();
const mutationInput = { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER) };
const authoringIds = z.array(slideId).min(1).max(128).refine(ids => new Set(ids).size === ids.length, 'IDs must be distinct');
const pictureInput = { id: slideId, base64: z.string().max(1398104), mime_type: z.enum(['image/png', 'image/jpeg']), alt: z.string().max(500), frame: optional(frameSchema), crop: cropSchema.optional() };
const authoringVariants = [
	z.object({ op: z.literal('add_elements'), slide_id: slideId, elements: z.array(elementSchema).min(1).max(128) }).strict(),
	z.object({ op: z.literal('set_frame'), slide_id: slideId, id: slideId, frame: frameSchema }).strict(),
	z.object({ op: z.literal('set_text_style'), slide_id: slideId, ids: authoringIds, style: runStyleSchema.refine(style => Object.values(style).some(value => value != null), 'At least one style property is required') }).strict(),
	z.object({ op: z.literal('set_slide_background'), slide_id: slideId, color: colorSchema }).strict(),
	z.object({ op: z.literal('set_connector'), slide_id: slideId, id: slideId, connector: connectorSettingsSchema, frame: optional(frameSchema) }).strict(),
	z.object({ op: z.literal('set_picture_crop'), slide_id: slideId, id: slideId, crop: cropSchema }).strict(),
	z.object({ op: z.literal('set_hyperlink'), slide_id: slideId, id: slideId, link: z.string().max(2048).nullable() }).strict(),
	z.object({ op: z.literal('set_shape_adjustment'), slide_id: slideId, id: slideId, adjustment: visualSchema.shape.adjustments.unwrap().element }).strict(),
	z.object({ op: z.literal('add_picture'), slide_id: slideId, ...pictureInput }).strict(),
	z.object({ op: z.literal('add_part'), slide_id: slideId, id: z.string().min(1).max(40), spec: z.lazy(() => partSpec) }).strict(),
	z.object({ op: z.literal('update_part'), slide_id: slideId, id: z.string().min(1).max(40), spec: z.lazy(() => partSpec) }).strict(),
	z.object({ op: z.literal('add_graph'), slide_id: slideId, id: z.string().min(1).max(40), spec: z.lazy(() => graphSpec), layout: optional(z.lazy(() => partLayout)) }).strict(),
	z.object({ op: z.literal('update_graph'), slide_id: slideId, id: z.string().min(1).max(40), spec: z.lazy(() => graphSpec) }).strict(),
];
const authoringOperations = z.array(z.discriminatedUnion('op', authoringVariants)).min(1).max(128);
const objectInput = { id: slideId, kind: z.enum(['text', 'shape', 'table', 'chart', 'line', 'arrow']), preset: z.string().max(80).optional(), rows: z.number().int().min(1).max(12).optional(), columns: z.number().int().min(1).max(8).optional() };
const templateKind = z.enum(['potx', 'thmx']);
const templateFilename = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,79}\.(?:potx|thmx)$/).refine((name) => !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])\./i.test(name), 'Reserved Windows filename');

const graphId = z.string().regex(/^[A-Za-z0-9_-]{1,24}$/);
const graphPort = z.enum(['auto', 'top', 'left', 'bottom', 'right']);
const graphIcon = z.object({ base64: z.string().min(1).max(1398104), mime_type: z.enum(['image/png', 'image/jpeg']), alt: z.string().max(500).optional() }).strict();
const graphNode = z.object({ id: graphId, label: z.string().max(160), detail: optional(z.string().max(480).refine(value => value.trim().length > 0 && Array.from(value).length <= 240, 'Detail requires 1-240 Unicode scalars and must not be blank')), detail_font_size: optional(z.number().min(12).max(40)), text_align: optional(z.enum(['left', 'center', 'right'])), heading_bold: z.boolean().optional(), kind: z.enum(['rectangle', 'rounded_rectangle', 'ellipse', 'diamond', 'cylinder', 'cloud']).optional(), presentation: z.enum(['card', 'icon']).optional(), x: z.number().min(0).max(1152), y: z.number().min(0).max(512), width: z.number().min(64).max(1152).optional(), height: z.number().min(40).max(512).optional(), fill: colorSchema.optional(), stroke: colorSchema.optional(), color: colorSchema.optional(), font_size: z.number().min(12).max(40).optional(), group: graphId.nullable().optional(), icon: graphIcon.nullable().optional() }).strict();
const graphLabelPlacement = z.object({ position: alpha, side: z.enum(['above', 'below']).optional(), offset: z.number().min(0).max(128).optional(), on_overlap: z.enum(['warn', 'error']).optional() }).strict();
const graphBadge = z.object({ number: z.number().int().min(1).max(99), position: alpha.optional(), size: z.number().min(16).max(64).optional(), font_size: z.number().min(8).max(32).optional(), fill: colorSchema.optional(), color: colorSchema.optional() }).strict();
const graphEdge = z.object({ id: graphId, source: graphId, target: graphId, source_port: graphPort.optional(), target_port: graphPort.optional(), label: z.string().max(64).optional(), route: z.enum(['straight', 'elbow', 'manual']).optional(), waypoints: z.array(z.tuple([z.number().min(0).max(1152), z.number().min(0).max(512)])).max(16).optional(), color: colorSchema.optional(), arrow: z.boolean().optional(), start_arrow: z.boolean().optional(), dashed: z.boolean().optional(), stroke_width: optional(z.number().min(0.5).max(12)), label_color: optional(colorSchema), label_font_size: optional(z.number().min(8).max(40)), source_offset: optional(z.number().min(-0.5).max(0.5)), target_offset: optional(z.number().min(-0.5).max(0.5)), label_placement: optional(graphLabelPlacement), badge: optional(graphBadge) }).strict();
const graphGroup = z.object({ id: graphId, label: z.string().max(64), x: z.number().min(0).max(1152), y: z.number().min(0).max(512), width: z.number().min(64).max(1152), height: z.number().min(40).max(512), fill: colorSchema.optional(), stroke: colorSchema.optional(), parent: graphId.nullable().optional(), icon: graphIcon.nullable().optional(), padding: optional(z.number().min(0).max(64)), header_height: optional(z.number().min(20).max(128)), header_font_size: optional(z.number().min(8).max(32)) }).strict();
const graphSpec = z.object({ version: z.literal(1), title: z.string().max(80), subtitle: z.string().max(120).optional(), show_title: z.boolean().optional(), nodes: z.array(graphNode).min(1).max(48), edges: z.array(graphEdge).max(64).optional(), groups: z.array(graphGroup).max(16).optional() }).strict();
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
	z.object({ kind: z.literal('matrix'), corner_label: z.string().max(96).refine(value => Array.from(value).length <= 48, 'Corner label exceeds 48 Unicode scalars').meta({ maxLength: 48 }).optional().describe('At most 48 Unicode scalars. Omitted defaults to empty; core omits empty labels from serialization.'), rows: z.array(partLabel).min(2).max(4), columns: z.array(partLabel).min(2).max(4), cells: z.array(z.array(partLabel).min(2).max(4)).min(2).max(4) }).strict(),
	z.object({ kind: z.literal('groups'), groups: z.array(z.object({ label: z.string().max(32), items: z.array(z.string().max(40)).min(1).max(5) }).strict()).min(2).max(6) }).strict(),
	z.object({ kind: z.literal('timeline'), periods: z.array(z.string().max(16)).min(2).max(12), tasks: z.array(z.object({ label: z.string().max(32), start: z.number().int().min(0).max(11), end: z.number().int().min(1).max(12), progress: z.number().min(0).max(1).optional() }).strict()).min(1).max(8) }).strict(),
	z.object({ kind: z.literal('waterfall'), steps: z.array(z.object({ label: z.string().max(32), value: partValue, total: z.boolean().optional() }).strict()).min(2).max(10), unit: z.string().max(24).optional() }).strict(),
	z.object({ kind: z.literal('map'), points: z.array(z.object({ label: z.string().max(32), longitude: z.number().min(-180).max(180), latitude: z.number().min(-85).max(85), value: partValue.nullable().optional() }).strict()).min(1).max(8) }).strict(),
	z.object({ kind: z.literal('diagram'), graph: graphSpec }).strict(),
]);
const partLayout = frameSchema.extend({ show_title: z.boolean().optional() }).strict();
const partSpec = z.object({ version: z.literal(1), preset: z.string().min(1).max(100), title: z.string().max(80), subtitle: z.string().max(120).optional(), data: partData, layout: optional(partLayout) }).strict();
const contentHash = z.string().regex(/^[a-f0-9]{64}$/);
const masterSourceSchema = z.object({ kind: z.enum(['pptx', 'potx']), base64: archiveBase64.min(1) }).strict();
const masterImportSchema = masterSourceSchema.extend({
	source_sha256: contentHash, mode: z.enum(['masters', 'slides']),
	ids: z.array(z.string().min(1).max(80).refine(value => value.trim().length > 0, 'Source IDs must not be blank')).min(1).max(8).refine(ids => new Set(ids).size === ids.length, 'Source IDs must be distinct'),
	prefix: z.string().regex(/^[A-Za-z0-9_-]{1,32}$/), name: z.string().min(1).max(60).refine(value => value.trim().length > 0, 'Master import name is required'),
}).strict();
const revisionIds = z.array(slideId).min(1).max(32);
const revisionEdits = z.array(z.discriminatedUnion('op', [
	z.object({ op: z.literal('translate'), ids: revisionIds, dx: z.number().min(-4096).max(4096), dy: z.number().min(-4096).max(4096) }).strict(),
	z.object({ op: z.literal('align'), ids: revisionIds, alignment: z.enum(['left', 'center', 'right', 'top', 'middle', 'bottom']), relative_to: z.enum(['selection', 'page']) }).strict(),
	z.object({ op: z.literal('set_text_frame'), id: slideId, x: padding, y: padding, width: padding.positive(), height: padding.positive() }).strict(),
	z.object({ op: z.literal('replace_text'), id: slideId, text: z.string().max(4000) }).strict(),
	z.object({ op: z.literal('update_part'), id: slideId, spec: partSpec }).strict(),
	z.object({ op: z.literal('update_graph'), id: slideId, spec: graphSpec }).strict(),
])).min(1).max(16).refine(edits => Buffer.byteLength(JSON.stringify(edits)) <= 128 * 1024, 'Revision edits exceed 128 KiB');
const guidedAuthoring = z.object({ context: optional(z.enum(['reading', 'projection'])), density: optional(z.enum(['comfortable', 'compact'])), spacing: optional(z.enum(['standard', 'relaxed'])), body_font_min: optional(z.number().min(12).max(40)), headline_font_size: optional(z.number().min(28).max(64)), font_family: optional(z.string().trim().min(1).max(100)), headline_style: optional(z.enum(['sentence', 'keyword'])), slide_limit: optional(z.number().int().min(32).max(128)) }).strict();
const authoringProfile = z.enum(['consulting-decision', 'technical-explainer', 'event-talk', 'status-report']);
const evidenceId = z.string().min(1).max(40);
const guidedInput = z.object({
	version: z.literal(1), profile_id: authoringProfile, title: z.string().min(1).max(120), audience: z.string().min(1).max(240), purpose: z.string().min(1).max(600), governing_message: z.string().min(1).max(600), language: z.enum(['en', 'ja']), brand_color: z.string().regex(/^[0-9a-fA-F]{6}$/).nullable().optional(),
	authoring: optional(guidedAuthoring),
	evidence: z.array(z.object({ id: evidenceId, kind: z.enum(['source', 'assumption', 'unknown']), reference: z.string().min(1).max(600), statement: z.string().min(1).max(1200) }).strict()).max(64),
	issues: z.array(z.object({ id: z.string().min(1).max(32), question: z.string().min(1).max(100), requested_decision: z.string().min(1).max(120), criterion: z.string().min(1).max(120), owner: z.string().min(1).max(48), due: z.string().min(1).max(48), evidence_ids: z.array(evidenceId).min(1).max(8), analysis_slide_ids: z.array(slideId).min(1).max(12) }).strict()).max(6).optional(),
	slides: z.array(z.object({ id: slideId, section: z.string().min(1).max(80), headline: z.string().min(1).max(240), sentence_form: z.enum(['causal', 'conditional', 'contrast', 'causal-focus', 'evaluation', 'proposal', 'explanation', 'comparison', 'outcome']), pattern_id: z.string().min(1).max(80), question: z.string().min(1).max(240), parent_message: z.string().min(1).max(80), transition: z.string().min(1).max(80), parallel_basis: z.string().min(1).max(80), part: partSpec.nullable().optional(), speaker_notes: optional(z.string().max(8000).refine(value => Array.from(value).length <= 4000, 'Speaker notes exceed 4000 Unicode scalars')), support: z.array(z.object({ clause: z.string().min(1).max(240), body_paths: z.array(z.string().min(1).max(512)).min(1).max(16), evidence_ids: z.array(evidenceId).min(1).max(16) }).strict()).max(8), numbers: z.array(z.object({ path: z.string().min(1).max(512), value: z.union([partValue, z.string().max(120), z.boolean(), z.null()]), evidence_id: evidenceId }).strict()).max(256).optional() }).strict()).min(1).max(128),
}).strict();

function getDeck(id) {
	const state = decks.get(id);
	if (!state) throw new Error('Unknown deck handle; create or compile a deck first');
	return state;
}

function previewResult(result, includeImages = true) {
	return {
		[imageResult]: true,
		metadata: { ...result, images: result.images.map(({ base64: _base64, ...metadata }) => metadata) },
		images: includeImages ? result.images.map(image => ({ type: 'image', data: image.base64, mimeType: image.mime_type })) : [],
	};
}

function expireCandidates() {
	for (const [id, candidate] of revisionCandidates) {
		const state = decks.get(candidate.deck_id);
		if (performance.now() >= candidate.expires || !state || state.revision !== candidate.expected_revision) revisionCandidates.delete(id);
	}
}

function toolResponse(result) {
	const metadata = result?.[imageResult] ? result.metadata : result;
	const text = JSON.stringify(metadata);
	return { content: [{ type: 'text', text }, ...(result?.[imageResult] ? result.images : [])], ...(Buffer.byteLength(text) <= 65536 ? { structuredContent: metadata } : {}) };
}

function managedProgress(name, readOnly, extra) {
	const managed = !readOnly || ['preview_presentation', 'preview_slide_revision', 'preflight_presentation', 'measure_layout', 'check_accessibility', 'validate_deck', 'preview_master_import'].includes(name);
	const token = extra._meta?.progressToken;
	if (!managed || !(typeof token === 'string' || Number.isSafeInteger(token)) || typeof extra.sendNotification !== 'function') return () => {};
	const started = performance.now();
	let stopped = false;
	let inFlight = false;
	let progress = -1;
	const report = () => {
		if (stopped || extra.signal.aborted || inFlight) return;
		const elapsed = (performance.now() - started) / 1000;
		progress = Math.max(progress + 0.001, elapsed);
		inFlight = true;
		void Promise.resolve().then(() => {
			if (stopped || extra.signal.aborted) return;
			return extra.sendNotification({ method: 'notifications/progress', params: { progressToken: token, progress, message: `${name}: ${Math.floor(elapsed)} seconds elapsed; not a completion percentage.` } });
		}).catch(() => {}).finally(() => { inFlight = false; });
	};
	const timer = setInterval(report, 5000);
	timer.unref?.();
	const stop = () => { stopped = true; clearInterval(timer); extra.signal.removeEventListener('abort', stop); };
	extra.signal.addEventListener('abort', stop, { once: true });
	report();
	return stop;
}

function register(name, description, inputSchema, readOnly, action) {
	const selectsProfile = ['create_presentation', 'compile_report', 'open_pptx', 'import_pptx', 'open_project', 'import_template', 'inspect_master_source', 'create_guided_presentation', 'verify_recovery', 'recover_presentation'].includes(name);
	server.registerTool(name, {
		description,
		inputSchema: z.object({ ...inputSchema, ...(selectsProfile ? { capacity_profile: capacityProfileSchema.optional() } : {}) }).strict(),
		annotations: { readOnlyHint: readOnly, destructiveHint: false, openWorldHint: name === 'generate_report' },
	}, async (input, extra) => {
		if (!readOnly && activeMutation) return { isError: true, content: [{ type: 'text', text: 'Another mutation is in progress' }] };
		if (!readOnly) activeMutation = true;
		let stopProgress;
		try {
			const { capacity_profile, ...parameters } = input;
			const profile = capacity_profile ?? (input.deck_id ? getDeck(input.deck_id).capacityProfile : 'large');
			const budget = CAPACITY_PROFILES[profile].request_bytes;
			if (Buffer.byteLength(JSON.stringify(input), 'utf8') > budget) throw new Error('JSON input exceeds selected capacity profile');
			if (extra.signal.aborted) throw new Error('Operation cancelled');
			stopProgress = managedProgress(name, readOnly, extra);
			const result = await action(parameters, extra.signal, { signal: extra.signal, capacityProfile: profile });
			if (readOnly && extra.signal.aborted) throw new Error('Operation cancelled');
			const response = toolResponse(result);
			if (Buffer.byteLength(JSON.stringify(response), 'utf8') > Math.min(budget, result?.[imageResult] ? 4 * 1048576 : budget)) throw new Error('JSON tool output exceeds selected capacity profile; request a smaller result');
			return response;
		} catch (error) {
			if (error instanceof BundlePublicationError) {
				const status = !error.published_paths.length ? 'not_published' : error.pending_filenames.length ? 'partially_published' : 'published_with_error';
				return { ...toolResponse({ status, code: error.code, message: error.message, ...error.delivery_context,
					published_paths: error.published_paths, pending_filenames: error.pending_filenames, cleanup_errors: error.cleanup_errors,
					retry: 'Inspect published paths and manifest hashes; do not overwrite or blindly retry the same name. Use a new name for another complete delivery.', multi_file_atomic: false }), isError: true };
			}
			return { isError: true, content: [{ type: 'text', text: error instanceof Error ? error.message : 'Operation failed' }] };
		} finally { stopProgress?.(); if (!readOnly) activeMutation = false; }
	});
}

register('sample_report', 'Return a twelve-slide structured example. All numbers are synthetic, and no model or network request is used.', {}, true, async (_input, signal) => requestCore({ op: 'sample' }, { signal }));
register('inspect_font', 'Inspect explicitly selected TTF/OTF bytes, family/style, OS/2 fsType and hash. No installation, file access or license verification. Metadata-only fonts are not usable.', { base64: fontBase64 }, true, async ({ base64 }, signal) => client.inspectFont(base64, { signal }));
register('inspect_pptx_fonts', 'Inspect only the presentation-root embedded font list and referenced internal bytes. Reports hashes and opaque encodings. Never loads fonts, follows external relationships or removes protection.', { base64: z.string().max(MAX_REQUEST_BYTES - 1024) }, true, async ({ base64 }, signal) => client.inspectPptxFonts(base64, { signal }));
register('list_fonts', 'List supported document font metadata without raw font bytes or filesystem paths. Office compatibility is unverified.', { deck_id: handle }, true, async ({ deck_id }, signal) => getDeck(deck_id).listFonts({ signal }));
register('embed_font', 'Attach a selected complete static TTF/OTF after explicit license confirmation. fsType is not proof of a license. Installable/editable rights only, eight faces, 12 MiB per face and 24 MiB combined, additionally constrained by the full document including immutable origin. Full EOT v1; no subsetting, variable fonts or collections. Undo/revision checked; no global installation.', { ...mutationInput, base64: fontBase64, license_acknowledged: z.literal(true) }, false, async ({ deck_id, expected_revision, base64, license_acknowledged }, signal) => {
	const state = getDeck(deck_id); await state.embedFont({ base64, license_acknowledged }, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash, office_verified: false };
});
register('set_font_usage', 'Set document-local font consent by content hash using Undo/revision checks. Confirm actual embedding/editing rights before enabling. No global installation; revoked consent blocks new embedding.', { ...mutationInput, sha256: z.string().regex(/^[a-f0-9]{64}$/), license_acknowledged: z.boolean() }, false, async ({ deck_id, expected_revision, sha256, license_acknowledged }, signal) => {
	const state = getDeck(deck_id); await state.setFontUsage(sha256, license_acknowledged, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('capacity_profiles', 'Read fixed large (default), standard and legacy resource budgets. No custom or unlimited policy is accepted.', {}, true, async (_input, signal) => client.capacityProfiles({ signal }));
register('set_capacity_profile', 'Validate the complete current document and both histories before selecting a fixed capacity profile. Failed or cancelled lowering preserves the current profile and document. No truncation.', { ...mutationInput, profile: capacityProfileSchema }, false, async ({ deck_id, expected_revision, profile }, signal) => {
	const state = getDeck(deck_id); if (state.revision !== expected_revision) throw new Error('Revision conflict');
	await state.setCapacityProfile(profile, { signal }); return { deck_id, capacity_profile: state.capacityProfile, revision: state.revision };
});
register('get_session_recovery', 'Read a typed session recovery envelope including plaintext original/source bytes and bounded Undo/Redo receipts. Does not persist files or access the recovery store.', { deck_id: handle }, true, async ({ deck_id }) => getDeck(deck_id).recoveryEnvelope);
register('recover_session', 'Core-verify a versioned session envelope and both receipt chains, then create a new handle. No arbitrary filesystem access, original mutation or storage consent changes.', { recovery_json: recoveryJson }, false, async ({ recovery_json }, signal) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const state = await client.recoverSession(JSON.parse(recovery_json), { signal });
	const deck_id = randomUUID(); decks.set(deck_id, state);
	return { deck_id, revision: state.revision, capacity_profile: state.capacityProfile, can_undo: state.canUndo, can_redo: state.canRedo, history_boundary: state.historyBoundary };
});
register('authoring_capabilities', 'Read current supported authoring operations, schemas and limits. Native preservation may reject edits; no Office parity or future implementation claims.', {}, true, async (_input, signal) => client.authoringCapabilities({ signal }));
register('format_text_element', 'Return a validated element with a Unicode-scalar range styled by core rich_text. No document state or history is changed.', { element: elementSchema, start: z.number().int().min(0).max(4000), end: z.number().int().min(0).max(4000), style: runStyleSchema }, true, async (input, signal) => client.formatTextElement(input, { signal }));
register('replace_element_text', 'Return a validated element with replaced text and retained supported rich formatting. Does not change any document or flatten dynamic fields.', { element: elementSchema, text: z.string().max(4000) }, true, async (input, signal) => client.replaceElementText(input, { signal }));
register('set_table_cell_text', 'Return a validated table with synchronized plain and rich cell text, retaining cell formatting. Does not change any document.', { element: elementSchema, row: rowIndex, column: columnIndex, text: z.string().max(200) }, true, async (input, signal) => client.setTableCellText(input, { signal }));
register('edit_vector', 'Return a validated polygon with a normalized closed path, retaining all other element fields. Reject locked elements. Does not change any document.', { element: elementSchema, path: pathSchema }, true, async (input, signal) => client.editVector(input, { signal }));
register('replace_deck', 'Replace a strictly typed scene in one revision, retaining sources and native origin. Native preservation, field identity and geometry checks remain authoritative; no arbitrary XML is accepted.', { ...mutationInput, deck: deckSchema }, false, async ({ deck_id, expected_revision, deck }, signal) => {
	const state = getDeck(deck_id); await state.replaceDeck(deck, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('update_notes', 'Update one slide speaker-notes value in a revision-checked transaction. Native unsupported rich notes fail closed; Undo restores the previous value.', { ...mutationInput, slide_id: slideId, notes: z.string().max(8000) }, false, async ({ deck_id, expected_revision, slide_id, notes }, signal) => {
	const state = getDeck(deck_id); await state.updateNotes(slide_id, notes, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('update_rich_notes', 'Atomically update typed notes paragraphs and their plain projection. Up to 8000 Unicode scalars. Native unrepresentable paragraph changes fail closed; unchanged fields, styles and parts remain native. One Undo.', { ...mutationInput, slide_id: slideId, paragraphs: notesParagraphsSchema }, false, async ({ deck_id, expected_revision, slide_id, paragraphs }, signal) => {
	const state = getDeck(deck_id); await state.updateRichNotes(slide_id, paragraphs, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('update_auxiliary_design', 'Update separate notes and handout masters in real notes-page coordinates. Creates native OOXML parts and preserves unknown existing content. Does not add slide masters, change print imposition or certify Office rendering. Native removal and ambiguous masters are rejected.', { ...mutationInput, design: auxiliaryDesignSchema }, false, async ({ deck_id, expected_revision, design }, signal) => {
	const state = getDeck(deck_id); await state.updateAuxiliaryDesign(design, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('refresh_fields', 'Refresh slidenum/datetime1..13 caches using explicit caller date and optional HH:mm:ss time. Only en-US dates are evaluated. Missing time, other locales and unknown types retain their caches; no system-clock inference. Shared master page numbers are not overwritten.', { ...mutationInput, reference_date: z.string().length(10).regex(/^\d{4}-\d{2}-\d{2}$/), reference_time: z.string().regex(/^\d{2}:\d{2}:\d{2}$/).optional(), locale: z.string().min(1).max(64).optional() }, false, async ({ deck_id, expected_revision, reference_date, reference_time, locale }, signal) => {
	const state = getDeck(deck_id); await state.refreshFields(reference_date, { signal, expectedRevision: expected_revision, referenceTime: reference_time, locale }); return { deck_id, revision: state.revision, hash: state.document.hash, warnings: state.fieldWarnings };
});
for (const [name, schema, apply] of [
	['add_comment', { comment: localCommentSchema }, (state, slide, input, options) => state.addComment(slide, input.comment, options)],
	['reply_comment', { parent_id: slideId, comment: localCommentSchema }, (state, slide, input, options) => state.replyComment(slide, input.parent_id, input.comment, options)],
	['resolve_comment', { comment_id: slideId, resolved: z.boolean() }, (state, slide, input, options) => state.resolveComment(slide, input.comment_id, input.resolved, options)],
	['remove_comment', { comment_id: slideId }, (state, slide, input, options) => state.removeComment(slide, input.comment_id, options)],
]) {
	register(name, 'Edit legacy native comments with AISlide-local reply/resolution state in one revision. Author and timestamp are explicitly supplied plaintext, not authenticated identities or service mentions. Resolve false reopens; removing a parent removes its replies. Not modern PowerPoint thread semantics.', { ...mutationInput, slide_id: slideId, ...schema }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
		const state = getDeck(deck_id); await apply(state, slide_id, input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
	});
}
register('modern_comment', 'Edit native modern comment threads with active/resolved/closed state and nonrecursive replies. New identities are local offline GUIDs, not authenticated users. New anchors must explicitly be unknown; existing opaque anchors and rich XML are preserved. Unsupported body edits and slide copies fail closed. One Undo.', { ...mutationInput, slide_id: slideId, operation: modernOperation }, false, async ({ deck_id, expected_revision, slide_id, operation }, signal) => {
	const state = getDeck(deck_id); await state.modernComment(slide_id, operation, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('set_table_headers', 'Declare semantic table headers independently of first-row visual styling. Unknown/none/first_row/first_column/both. Merged associations require manual review; this is not PDF or WCAG certification.', { ...mutationInput, slide_id: slideId, element_id: slideId, policy: tableHeaders }, false, async ({ deck_id, expected_revision, slide_id, element_id, policy }, signal) => {
	const state = getDeck(deck_id); await state.setTableHeaders(slide_id, element_id, policy, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('set_accessibility', 'Set or remove typed accessibility metadata in one revision after the target exists. Separate from add_part/add_graph and apply_operations. Core budget is at least 65 seconds and size-aware up to 180 seconds; opt into MCP progress and use a sufficient finite client deadline. Picture descriptions map to alt text. Decorative is not an accessibility certification.', { ...mutationInput, slide_id: slideId, element_id: slideId, metadata: accessibilitySchema.nullable() }, false, async ({ deck_id, expected_revision, slide_id, element_id, metadata }, signal) => {
	const state = getDeck(deck_id); await state.setAccessibility(slide_id, element_id, metadata, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('set_reading_order', 'Set all top-level IDs exactly once or clear the explicit order. Also changes native z-order and may change overlaps. No screen-reader or group-order certification.', { ...mutationInput, slide_id: slideId, order: readingOrderSchema }, false, async ({ deck_id, expected_revision, slide_id, order }, signal) => {
	const state = getDeck(deck_id); const result = await state.setReadingOrder(slide_id, order, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash, warnings: result.warnings };
});
register('check_accessibility', 'Read bounded core accessibility findings and coverage limitations. Not WCAG certification or a screen-reader test.', { deck_id: handle }, true, async ({ deck_id }, signal) => getDeck(deck_id).checkAccessibility({ signal }));
register('inspect_document', 'Read removable-category counts and separate masked email/phone/personal-property candidates with numeric locations in current_deck and embedded_origin scopes. No values, snippets, paths or author names are returned. Candidates require manual review and cannot be clean-copy categories. Bounded scan, no OCR/external access; history retains content. Not proof of complete detection.', { deck_id: handle }, true, async ({ deck_id }, signal) => getDeck(deck_id).inspectDocument({ signal }));
register('export_clean_copy', 'Export only a NEW PPTX after explicit category selection and confirmed=true. Source session/history are unchanged. Refuses signed, labelled or protected packages; never unprotects. Inspection is incomplete, not a guarantee of secret removal. Requires the approved output directory and a new filename.', { ...mutationInput, categories: cleanCategories, confirmed: z.literal(true), filename: filenameSchema }, false, async ({ deck_id, expected_revision, categories, confirmed, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const state = getDeck(deck_id); if (state.revision !== expected_revision) throw new Error('Revision conflict');
	const new_document_id = randomUUID();
	const result = await state.exportCleanCopy({ new_document_id, categories, confirmed }, { signal });
	if (result.document.id !== new_document_id || result.document.revision !== 0) throw new Error('Core did not return a new clean document');
	if (signal.aborted) throw new Error('Operation cancelled');
	const bytes = Buffer.from(result.base64, 'base64'); const destination = join(outputDirectory, filename);
	await publishNewFile(join(outputDirectory, `.aislide-${randomUUID()}.tmp`), destination, bytes, signal);
	return { new_document_id, path: destination, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), inspection: result.inspection };
});
register('search_text', 'Search current scene text using Unicode-scalar ranges and bounded literal matching. Does not mutate the document. Unsupported rich content follows the current core support contract.', { deck_id: handle, options: searchSchema }, true, async ({ deck_id, options }, signal) => ({ matches: await getDeck(deck_id).searchText(options, { signal }) }));
register('import_proofing_dictionary', 'Parse supplied UTF-8 word-list or JSON dictionary content locally. 512 KiB maximum. User must have rights to the supplied data. No paths, OS install, model, or network lookup.', { language: proofLanguageSchema, format: z.enum(['wordlist', 'json']), content: z.string().max(524288) }, true, async (input, signal) => client.importProofingDictionary(input, { signal }));
register('proof_text', 'Local word-list membership check, up to five one-edit suggestions, synonyms and exact bilingual term lookup. Without a supplied dictionary only a tiny representative en-US sample is available. Not grammar checking, a complete dictionary, or sentence translation.', { text: z.string().max(8000), language: proofLanguageSchema, dictionary: optional(proofDictionarySchema), term: z.string().min(1).max(128).optional(), target_language: proofLanguageSchema.optional() }, true, async (input, signal) => client.proofText(input, { signal }));
register('copy_format', 'Read a typed top-level text/shape, rect/polygon, picture, table or chart style. Text accepts paragraph/run indices; others require zero. Does not copy content, data, links, highlights, language, geometry or font resources.', { deck_id: handle, slide_id: slideId, id: slideId, paragraph_index: z.number().int().min(0).max(255).optional(), run_index: z.number().int().min(0).max(4095).optional() }, true, async ({ deck_id, slide_id, ...input }, signal) => getDeck(deck_id).copyFormat(slide_id, input, { signal }));
register('sample_slide_pixel', 'Read a composited pixel from a core-rendered slide, not the screen. Zero-based integer slide-pixel coordinates. Read-only; returns rendering warnings, not Office parity.', { deck_id: handle, slide_id: slideId, x: z.number().int().min(0).max(4095), y: z.number().int().min(0).max(4095) }, true, async ({ deck_id, slide_id, x, y }, signal) => getDeck(deck_id).sampleSlidePixel(slide_id,x,y,{ signal }));
register('compute_chart_presentation', 'Read-only bounded Cartesian projection: normalized SVD regression, absolute error endpoints, axes/ticks and numeric-format subset. 6 series, 32 categories, 256 trend points, 100000 forecast X units. Does not replace raw native chart/workbook values; not Office parity.', chartSchema.shape, true, async (input, signal) => client.computeChartPresentation(input, { signal }));
register('render_element_preview', 'Generate inert SVG from typed chart/text/shape using shared core. No raw SVG input, document mutation, external fetch or Office parity. Default WordArt adjustments only; installed fonts, not embedded document fonts.', { element: elementSchema.refine(element => ['chart', 'text', 'shape'].includes(element.type)), theme: themeSchema.optional() }, true, async ({ element, theme }, signal) => client.renderElementPreview(element, theme, { signal }));
register('apply_format', 'Apply a typed style to 1-128 compatible top-level objects atomically. Preserves data, content, geometry, links, highlights, language and fields. Tables require equal grid dimensions; charts equal series count. Incompatible, locked, hidden or unknown native targets fail closed. One Undo.', { ...mutationInput, slide_id: slideId, ids: z.array(slideId).min(1).max(128), style: formatSnapshotSchema }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
	const state = getDeck(deck_id); await state.applyFormat(slide_id, input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('set_proofing_language', 'Set an explicit proofing-language tag on all runs of one text box or shape. Does not install a dictionary or translate text. Revision checked and undoable.', { ...mutationInput, slide_id: slideId, id: slideId, language: proofLanguageSchema }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
	const state = getDeck(deck_id); await state.setProofingLanguage(slide_id, input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('replace_text', 'Replace all or explicitly selected literal matches in one revision. Match expected values, document integrity and native preservation are checked; no regex or hidden model calls.', { ...mutationInput, options: replaceSchema }, false, async ({ deck_id, expected_revision, options }, signal) => {
	const state = getDeck(deck_id); await state.replaceText(options, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('replace_font', 'Replace exact explicit font-family names using the current core text contract, with revision checking and Undo. Never installs fonts or replaces theme tokens blindly.', { ...mutationInput, from: z.string().min(1).max(100), to: z.string().min(1).max(100) }, false, async ({ deck_id, expected_revision, from, to }, signal) => {
	const state = getDeck(deck_id); await state.replaceFont(from, to, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
for (const [name, method, schema, description] of [
	['format_text', 'formatText', { start: z.number().int().min(0).max(4000), end: z.number().int().min(0).max(4000), style: runStyleSchema }, 'Apply a RunStyle to a Unicode-scalar range [start,end) in a text box or shape; preserves rich runs outside the selection.'],
	['replace_text_content', 'replaceTextContent', { text: z.string().max(4000) }, 'Replace the text of a text box or shape with rich-format preservation; synchronize plain text and paragraphs.'],
	['update_paragraphs', 'updateParagraphs', { paragraphs: paragraphsSchema }, 'Replace typed rich paragraphs and synchronize plain text in one revision. Empty paragraphs clear the text.'],
	['edit_table', 'editTable', { operations: tableOperationsSchema }, 'Apply typed table format, cell style, merge, split and row/column edits atomically. Reject nonempty merge subordinates and edits crossing merges.'],
	['apply_image_edit', 'applyImageEdit', { image: preparedImageSchema }, 'Apply explicitly prepared PNG/JPEG bytes to a picture, retaining its frame, crop and alt text. Verify actual dimensions; discard any superseded SVG source. No implicit image processing.'],
]) {
	register(name, `${description} Requires revision and passes native preservation validation before SDK acceptance.`, { ...mutationInput, slide_id: slideId, id: slideId, ...schema }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
		const state = getDeck(deck_id); await state[method](slide_id, input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
	});
}
register('combine_shapes', 'Union, intersect, subtract, XOR or fragment 2-32 filled rectangles/polygons. Beziers adaptively flatten within 0.25 slide pixels before clipping; output is polygonal, not exact Bezier. Fragment partitions covered areas into up to 128 nonoverlapping connected objects (result_id, result_id-2, ...). First ID defines style. 4096 vertices, 256 commands per compound path and 262144 edge-pair work budget. Transforms/effects, unsafe native content and empty results reject. One Undo; no Office parity.', { ...mutationInput, slide_id: slideId, ids: z.array(z.string().min(1).max(80)).min(2).max(32), operation: z.enum(['union', 'intersect', 'subtract', 'xor', 'fragment']), result_id: z.string().min(1).max(80) }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
	const state = getDeck(deck_id); const result = await state.combineShapes(slide_id, input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash, effects: result.effects, changes: result.changes };
});
register('edit_selection', 'Transform, align, distribute, group, ungroup, copy, cut or paste a typed structural selection. Copy has no mutation. Returns clipboard/effects; only verifiable current-document copies map metadata. Raw XML is not in bundles; imported topology or raw-copy loss may be rejected.', { ...mutationInput, slide_id: slideId, operation: selectionSchema, clipboard: optional(clipboardSchema) }, false, async ({ deck_id, expected_revision, slide_id, operation, clipboard }, signal) => {
	const state = getDeck(deck_id); const result = await state.editSelection(slide_id, operation, { clipboard, signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash, clipboard: result.clipboard, effects: result.effects, changes: result.changes };
});
register('resize_canvas', 'Resize both page dimensions within 320..4096 using scale or keep. One undoable document revision; out-of-bounds content rejects keep. Existing part metadata can become stale after scaling.', { ...mutationInput, width: z.number().int().min(320).max(4096), height: z.number().int().min(320).max(4096), mode: z.enum(['scale', 'keep']) }, false, async ({ deck_id, expected_revision, ...input }, signal) => {
	const state = getDeck(deck_id); await state.resizeCanvas(input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('edit_image', 'Prepare a locally edited PNG/JPEG object without document or file mutation. Explicit brightness, contrast, saturation, grayscale, RGB background key and resize; JPEG alpha requires an explicit matte. Keying is not segmentation.', { base64: preparedImageSchema.shape.base64, mime_type: preparedImageSchema.shape.mime_type, params: imageParamsSchema }, true, async (input, signal) => client.editImage(input, { signal }));
register('text_assist', 'Generate a review-only full-text proofreading or translation candidate with a configured local loopback model. Strict JSON, 8000 scalar limit, no remote providers, no fallback or automatic application. Model quality is unverified.', { input: z.object({ task: z.enum(['proofread', 'translate']), text: z.string().min(1).max(16000), language: proofLanguageSchema, target_language: proofLanguageSchema.nullable().optional() }).strict() }, true, async ({ input }, signal) => client.textAssist(input, { signal }));
register('apply_text_assist', 'Apply a reviewed candidate atomically after checking revision, source SHA-256 and expected old text. Preserves paragraph formatting via rich replacement; fields and unsafe native content reject. One Undo.', { ...mutationInput, slide_id: slideId, id: slideId, expected_text: z.string().min(1).max(16000), candidate: z.object({ text: z.string().min(1).max(16000), source_sha256: z.string().regex(/^[0-9a-f]{64}$/) }).strict() }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
	const state = getDeck(deck_id); await state.applyTextAssist(slide_id, input, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('segmentation_status', 'Read sanitized local U2NetP model setup status. Verified bytes do not prove inference availability or quality. No download or document mutation.', {}, true, async (_input, signal) => client.segmentationStatus({ signal }));
register('segment_image', 'Generate a review-only PNG with U2NetP general saliency on local CPU. Explicit model setup and dataset terms acknowledgement required. Fixed pinned model, no path/URL inputs or runtime downloads, no color-key fallback. Native inference is non-preemptible; cancelled results are discarded. Use apply_image_edit separately.', { base64: preparedImageSchema.shape.base64, mime_type: preparedImageSchema.shape.mime_type }, true, async (input, signal) => client.segmentImage(input, { signal }));
register('import_template', 'Create a NEW document from explicit POTX or THMX base64. POTX uses the template factory; THMX creates one blank themed slide. No path reads, existing-deck mutation or protection changes.', { kind: templateKind, base64: archiveBase64 }, false, async (input, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const id = randomUUID(); const state = await client.importTemplate(id, input, options);
	if (signal.aborted) throw new Error('Operation cancelled');
	decks.set(id, state); return { deck_id: id, revision: state.revision, slides: state.document.deck.slides.length, kind: input.kind };
});
register('inspect_master_source', 'Inspect supplied PPTX/POTX bytes for source SHA-256, page size and selectable master/slide IDs with importability reasons. No files, source handles, network access or document mutation; not Office visual parity.', masterSourceSchema.shape, true, async (input, _signal, options) => client.inspectMasterSource(input, options));
register('preview_master_import', 'Preview 1-8 distinct inspected source masters or sample slides as editable design content under exact source and base revision/hash guards. Returns candidate hash, Design and preview_slides as JSON only; no cached candidate, files, document or Undo changes. Not Office visual parity.', { ...mutationInput, expected_hash: contentHash, input: masterImportSchema }, true, async ({ deck_id, expected_revision, expected_hash, input }, signal) => {
	return { deck_id, ...await getDeck(deck_id).previewMasterImport(input, { signal, expectedRevision: expected_revision, expectedHash: expected_hash }) };
});
register('import_masters', 'Import selected PPTX/POTX master or sample-slide design content after recomputing and verifying the source SHA-256, base revision/hash and expected candidate hash. Preserves existing slides and origin, records one Undo, and rejects unsafe native content. No files, fetches or candidate storage; not Office visual parity.', { ...mutationInput, expected_hash: contentHash, input: masterImportSchema, expected_candidate_hash: contentHash }, false, async ({ deck_id, expected_revision, expected_hash, input, expected_candidate_hash }, signal) => {
	const state = getDeck(deck_id);
	await state.importMasters(input, expected_candidate_hash, { signal, expectedRevision: expected_revision, expectedHash: expected_hash });
	return { deck_id, revision: state.revision, hash: state.document.hash, office_visual_parity: false };
});
register('export_template', 'Save a new POTX or THMX only inside the operator-approved output directory. Filename extension must exactly match kind. Never overwrites a file or converts protected/signed/macro content.', { deck_id: handle, kind: templateKind, filename: templateFilename }, false, async ({ deck_id, kind, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	if (!filename.endsWith(`.${kind}`)) throw new Error('Template filename extension must match kind');
	const result = await getDeck(deck_id).exportTemplate(kind, { signal });
	if (result.filename !== `template.${kind}`) throw new Error('Core returned an unexpected template format');
	if (signal.aborted) throw new Error('Operation cancelled');
	const bytes = Buffer.from(result.base64, 'base64'); const destination = join(outputDirectory, filename);
	await publishNewFile(join(outputDirectory, `.aislide-${randomUUID()}.tmp`), destination, bytes, signal);
	return { path: destination, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), kind };
});
register('create_presentation', 'Create a blank one-slide presentation with a default editable design. No synthetic report, file writes or network access.', { title: z.string().min(1).max(120) }, false, async ({ title }, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const id = randomUUID(); const state = await client.createPresentation(id, title, options); decks.set(id, state);
	return { deck_id: id, revision: state.revision, slides: 1 };
});
register('verify_recovery', 'Verify caller-supplied recovery JSON with document::verify. Checks shape, content hash, source bindings and native origin; never trusts database hashes, accesses storage or removes protection. Returns the verified document without mutation.', { document_json: recoveryJson }, true, async ({ document_json }, signal, options) => client.verifyRecovery(JSON.parse(document_json), options));
register('recover_presentation', 'Verify supplied recovery JSON and create a NEW session with empty Undo/Redo history. Never replaces an open deck, reads a recovery database, or bypasses protected origin validation.', { document_json: recoveryJson }, false, async ({ document_json }, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const state = await client.recoverPresentation(JSON.parse(document_json), options);
	if (signal.aborted) throw new Error('Operation cancelled');
	const id = randomUUID(); decks.set(id, state);
	return { deck_id: id, revision: state.revision, hash: state.document.hash, slides: state.document.deck.slides.length };
});
register('preview_presentation', 'Read selected pages as actual MCP PNG/JPEG images or one contact sheet, with slide IDs, coordinates, revision/hash and renderer warnings. 1-8 unique zero-based pages; omission selects all only when at most eight slides exist. Maximum edge 1600px, encoded images 2MiB, response 4MiB. Defaults: format=png, overflow=shrink. Only encoded-byte overflow retries at 75% then 56.25% of requested size, minimum 160px; all pages are retained. quality_reduced and actual_max_dimension disclose reduction. overflow=error forbids shrink. No files, network, document or Undo changes. include_images=false omits images but still renders; use get_document for revision/hash. Stale bindings are warned; not Office parity.', { deck_id: handle, options: previewOptions.optional(), include_images: z.boolean().optional() }, true, async ({ deck_id, options, include_images }, signal) => {
	return previewResult({ deck_id, ...await getDeck(deck_id).previewPresentation(options, { signal }) }, include_images);
});
register('preflight_presentation', 'Read bounded visual diagnostics for 1-8 selected pages: renderer clipping/glyph/font warnings, transformed off-slide frames, text overlap, connector-label interference, rounded-container corner and padding heuristics, small text and density. Likely intentional opaque numbered badges are info, not approved. Returns IDs, scopes, bounds, evidence and repair suggestions. No automatic edits, external fetch, factual verification, Office parity or accessibility certification. Backgrounds are not text collisions; likely rounded containers are checked separately. Attached connector endpoints are excluded. Run check_accessibility separately.', { deck_id: handle, options: z.object({ page_indices: previewOptions.shape.page_indices, min_font_size: z.number().min(8).max(48).optional() }).strict().optional() }, true, async ({ deck_id, options }, signal) => {
	return { deck_id, ...await getDeck(deck_id).preflightPresentation(options, { signal }) };
});
register('finalize_presentation', 'Prepare and exclusively publish a delivery from the exact current revision/hash under the operator-approved output directory. Always includes the complete PPTX and a manifest; optional PDF/PNG preview/diagnostics select at most eight pages (zero-based). Defaults: contact sheet and preflight on, PDF/notes/source-report off. notes/source_report explicitly export plaintext metadata; PPTX already retains notes and may contain source data. All generation, byte budgets and destination names are checked before publication; manifest last. No overwrite, source mutation, automatic persistence, source freshness or Office parity guarantee. Partial failures report actual published paths; multi-file output is not crash-atomic. Returns one preview image unless include_images=false.', { ...mutationInput, expected_hash: contentHash, name: deliveryName, options: deliveryOptions.optional(), include_images: z.boolean().optional() }, false, async ({ deck_id, expected_revision, expected_hash, name, options, include_images }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const state = getDeck(deck_id);
	const result = await state.prepareDelivery(options, { expectedRevision: expected_revision, expectedHash: expected_hash, signal });
	if (signal.aborted) throw new Error('Operation cancelled');
	if (state.revision !== expected_revision || state.document.hash !== expected_hash) throw new Error('Delivery base changed; inspect the current deck before exporting');
	const items = result.files.map(file => {
		const bytes = Buffer.from(file.base64, 'base64');
		if (bytes.length !== file.byte_length || createHash('sha256').update(bytes).digest('hex') !== file.sha256) throw new Error('Core delivery artifact hash mismatch');
		return { filename: `${name}${file.suffix}`, bytes };
	});
	const files = result.files.map(({ base64: _base64, suffix: _suffix, byte_length, ...file }, index) => ({ ...file, filename: items[index].filename, path: join(outputDirectory, items[index].filename), bytes: byte_length }));
	const manifestValue = { ...result.manifest, producer: { ...result.manifest.producer, name: serverInfo.name, version: serverInfo.version, core_version: result.manifest.producer.version, transport: 'mcp-stdio' },
		files: files.map(({ path: _path, ...file }) => file) };
	const manifestBytes = Buffer.from(JSON.stringify(manifestValue, null, 2));
	const manifestFilename = `${name}.manifest.json`;
	const manifest = { filename: manifestFilename, path: join(outputDirectory, manifestFilename), bytes: manifestBytes.length, sha256: createHash('sha256').update(manifestBytes).digest('hex') };
	items.push({ filename: manifestFilename, bytes: manifestBytes });
	if (items.reduce((total, item) => total + item.bytes.length, 0) > (options?.max_output_bytes ?? 32 * 1048576)) throw new Error('Delivery including manifest exceeds output budget');
	const thumbnail = result.files.find(file => file.kind === 'preview');
	const checks = { ...result.manifest.checks, preflight_errors: result.manifest.preflight?.findings.filter(finding => finding.severity === 'error').length ?? null,
		preflight_warnings: result.manifest.preflight?.findings.filter(finding => finding.severity === 'warning').length ?? null, render_warning_count: result.manifest.render_warnings.length };
	const response = { [imageResult]: true, metadata: { status: 'complete', deck_id, revision: result.revision, hash: result.hash, files, manifest, checks,
		multi_file_atomic: false, limitations: result.manifest.limitations }, images: thumbnail && include_images !== false ? [{ type: 'image', data: thumbnail.base64, mimeType: thumbnail.mime_type }] : [] };
	if (Buffer.byteLength(JSON.stringify(toolResponse(response))) > Math.min(4 * 1048576, CAPACITY_PROFILES[state.capacityProfile].request_bytes)) throw new Error('Delivery response exceeds budget; lower max_dimension or disable images');
	try { await publishNewBundle(outputDirectory, items, signal); }
	catch (error) {
		if (error instanceof BundlePublicationError) error.delivery_context = { deck_id, revision: result.revision, hash: result.hash, manifest_filename: manifestFilename };
		throw error;
	}
	return response;
});
register('preview_slide_revision', 'Preview 1-16 typed edits on one slide without changing document/history. Returns before/after MCP PNG images, affected IDs, stale metadata impact and an opaque candidate ID valid for 10 minutes in this server process. At most 16 live candidates, edits <=128KiB. translate/align use top-level IDs; set_text_frame/replace_text accept nested IDs in their parent coordinates. Frame edits retain font size. Locked/hidden targets and unsupported native edits reject. No raw patch, XML, URLs or arbitrary paths. Apply explicitly with matching base revision/hash.', { ...mutationInput, expected_hash: contentHash, slide_id: slideId, edits: revisionEdits, max_dimension: z.number().int().min(160).max(1600).optional(), include_images: z.boolean().optional() }, true, async ({ deck_id, expected_revision, expected_hash, slide_id, edits, max_dimension, include_images }, signal) => {
	expireCandidates();
	if (revisionCandidates.size >= 16) throw new Error('At most 16 live revision candidates; apply, close a deck, or wait for expiry');
	const state = getDeck(deck_id);
	const result = await state.previewSlideRevision(slide_id, edits, { expectedRevision: expected_revision, expectedHash: expected_hash, maxDimension: max_dimension, signal });
	if (signal.aborted) throw new Error('Operation cancelled');
	expireCandidates();
	if (revisionCandidates.size >= 16) throw new Error('At most 16 live revision candidates');
	if (state.revision !== expected_revision || state.document.hash !== expected_hash || decks.get(deck_id) !== state) throw new Error('Document changed during preview; preview again');
	const candidate_id = randomUUID();
	const before = previewResult(result.before, include_images);
	const after = previewResult(result.after, include_images);
	const response = { [imageResult]: true, metadata: { ...result, deck_id, candidate_id, expires_in_seconds: candidateLifetimeMs / 1000, before: before.metadata, after: after.metadata }, images: [...before.images, ...after.images] };
	const encoded = { content: [{ type: 'text', text: JSON.stringify(response.metadata) }, ...response.images], structuredContent: response.metadata };
	if (Buffer.byteLength(JSON.stringify(encoded)) > Math.min(4 * 1048576, CAPACITY_PROFILES[state.capacityProfile].request_bytes)) throw new Error('Revision preview exceeds response budget; lower max_dimension');
	revisionCandidates.set(candidate_id, { deck_id, expected_revision, expected_hash, slide_id, edits: structuredClone(edits), candidate_hash: result.candidate_hash, expires: performance.now() + candidateLifetimeMs });
	return response;
});
register('apply_slide_revision', 'Apply one unexpired immutable preview candidate to the same deck and exact base revision/hash. Recomputes the core candidate and verifies its content hash, then records one Undo. No-op creates no history. Missing, consumed, stale or cross-deck candidates fail without modifying the document. Does not export files.', { ...mutationInput, expected_hash: contentHash, candidate_id: handle }, false, async ({ deck_id, expected_revision, expected_hash, candidate_id }, signal) => {
	expireCandidates();
	const candidate = revisionCandidates.get(candidate_id);
	if (!candidate || candidate.deck_id !== deck_id) throw new Error('Unknown, expired or cross-deck revision candidate; preview again');
	if (candidate.expected_revision !== expected_revision || candidate.expected_hash !== expected_hash) throw new Error('Revision candidate base mismatch; preview again');
	const state = getDeck(deck_id);
	await state.applySlideRevision(candidate.slide_id, candidate.edits, { expectedRevision: expected_revision, expectedHash: expected_hash, candidateHash: candidate.candidate_hash, signal });
	revisionCandidates.delete(candidate_id);
	return { deck_id, revision: state.revision, hash: state.document.hash, slide_id: candidate.slide_id, candidate_id, can_undo: state.canUndo };
});
register('export_static', 'Export selected/all pages as one multipage PDF or individual PNG/JPEG files in the approved output directory. Exact extension pdf/png/jpg must match format. Multiple images use name-page-NNN.ext in selection order. All names preflight before exclusive publication; racing partial outputs are retained and reported, never deleted. Shared core only, no Office. PDF retains outlined appearance with a positioned searchable/selectable Unicode overlay and basic reading-order/table/alt-text tags; visible text is not editable. Not PDF/UA, WCAG or Office parity certification. 8192px/32MiB core caps and stricter 4MiB JSON bundle cap apply. Does not print.', { deck_id: handle, filename: staticFilename, options: staticOptions }, false, async ({ deck_id, filename, options }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const extension = options.format === 'jpeg' ? 'jpg' : options.format;
	if (!filename.endsWith(`.${extension}`)) throw new Error('Static filename extension must match format');
	const result = await getDeck(deck_id).exportStatic(options, { signal });
	const files = result.files.map(file => {
		if (!file.filename.endsWith(`.${extension}`)) throw new Error('Core returned an unexpected static format');
		const name = result.files.length === 1 ? filename : `${filename.slice(0, -extension.length - 1)}-page-${String(file.page_indices[0] + 1).padStart(3, '0')}.${extension}`;
		return { ...file, path: join(outputDirectory, name) };
	});
	if (new Set(files.map(file => file.path)).size !== files.length) throw new Error('Duplicate static output names');
	for (const file of files) {
		try { await lstat(file.path); throw new Error(`Output already exists: ${file.path}`); }
		catch (error) { if (error.code !== 'ENOENT') throw error; }
	}
	const published = [];
	try {
		for (const file of files) {
			const bytes = Buffer.from(file.base64, 'base64');
			await publishNewFile(join(outputDirectory, `.aislide-${randomUUID()}.tmp`), file.path, bytes, signal);
			const { base64: _base64, ...metadata } = file;
			published.push({ ...metadata, filename: file.path.split(/[\\/]/).at(-1), sha256: createHash('sha256').update(bytes).digest('hex') });
		}
	} catch (error) { throw new Error(`Static publication failed; public paths were not removed. Published: ${JSON.stringify(published.map(file => file.path))}. ${error.message}`); }
	return { ...result, files: published };
});
register('edit_slides', 'Insert, duplicate, remove, move or rename slides in one undoable revision. Preserve native parts and metadata; reject unsafe sections/custom shows and removing the final slide.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), operations: slideOperations }, false, async ({ deck_id, expected_revision, operations }, signal) => {
	const state = getDeck(deck_id); await state.editSlides(operations, { expectedRevision: expected_revision, signal });
	return { deck_id, revision: state.revision, slides: state.document.deck.slides.length };
});
register('create_asset', 'Validate PNG/JPEG or prepare inert SVG with a PNG fallback. SVG literal text uses local fonts; up to 8 embedded data PNG/JPEG images and 16 million decoded pixels. EMF v1/WMF placeable explicitly convert supported solid-pen/brush rectangle, ellipse, line and polygon records to SVG; unknown records reject. Vector input 256 KiB. No scripts, external targets, OS execution or network fetches.', assetInput, true, async (input, signal) => client.createAsset(input, { signal }));
register('edit_elements', 'Duplicate, remove or reorder a top-level element in one undoable revision. Preserve part metadata and bindings; reject lossy native copies and remove connectors that lose their target.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), slide_id: slideId, operations: elementOperations }, false, async ({ deck_id, expected_revision, slide_id, operations }, signal) => {
	const state = getDeck(deck_id); await state.editElements(slide_id, operations, { expectedRevision: expected_revision, signal });
	return { deck_id, revision: state.revision, slide_id };
});
register('add_asset', 'Insert an SVG/PNG/JPEG asset in one undoable revision. SVG uses a validated PNG fallback and retained inert SVG for native vector export; no external resource fetching. Existing slide content is not rearranged.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), slide_id: slideId, ...assetInput }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
	const state = getDeck(deck_id); await state.addAsset(slide_id, input, { expectedRevision: expected_revision, signal });
	return { deck_id, revision: state.revision, element_id: input.id };
});
register('graph_catalog', 'List architecture diagram shapes, ports, routing styles, typed schemas, limits and editable examples. Does not create a document or contact a network.', {}, true, async (_input, signal) => client.graphCatalog({ signal }));
register('architecture_icons', 'Return the fixed 1498-entry Azure/Entra, AWS and GCP metadata catalog with vendor notices, archive/source provenance and PNG pins. configured checks local vendor consent only, not full pack integrity or redistribution rights. Metadata has no artwork; this read-only API never downloads, installs or follows source URLs.', {}, true, async (_input, signal) => client.architectureIcons({ signal }));
register('architecture_icon_assets', 'Read explicitly selected local catalog PNGs after consent, size, SHA-256 and raster verification. Returns icons with id, original base64, image/png mime_type, full service name alt and actual width/height. Requires 1..60 distinct catalog IDs, at most 4 MiB raw PNG bytes per atomic batch; use fewer IDs if needed. No paths, URLs, downloads or artwork transformations. Strip id/width/height before supplying GraphIcon.', { ids: z.array(z.string().min(1).max(256)).min(1).max(60).refine(ids => new Set(ids).size === ids.length, 'Duplicate architecture icon IDs are not allowed') }, true, async ({ ids }, signal) => client.architectureIconAssets(ids, { signal }));
register('create_graph_icon', 'Prepare a bounded node icon from SVG, PNG or JPEG. SVG becomes transparent PNG; large rasters are fitted to a 256px longest side. Returns image data for GraphNode.icon, with no document or filesystem mutation. External resources and active SVG are rejected.', { base64: assetInput.base64, mime_type: assetInput.mime_type, alt: z.string().max(500).optional() }, true, async (input, signal) => client.createGraphIcon(input, { signal }));
register('create_graph', 'Render a typed graph without inserting it. Prefer apply_operations with add_graph and explicit layout for regenerable metadata. create_graph plus add_elements is an explicit unmanaged choice, never a timeout fallback. Canvas: 1152x512; y starts at 88 unless show_title=false. Core checks bounds and detail font size. No HTML, executable content or draw.io XML.', { id: z.string().min(1).max(40), spec: graphSpec, theme: themeSchema.optional() }, true, async (input, signal) => client.createGraph(input, { signal }));
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
register('create_guided_presentation', 'Create a NEW evidence-led presentation after strict validation. Defaults: sentence headlines, 32 slides. input.authoring.headline_style="keyword" permits keyword headings; slide_limit explicitly raises the limit within 32..128. Evidence and numeric checks still apply. Only consulting-decision multi-page decks require 3-6 issues, C02 summary and C03 close. Unknown numbers remain xx in qualitative content. No model call or file save; review actual rendering and export separately.', { input: guidedInput }, false, async ({ input }, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const id = randomUUID(); const result = await client.createGuidedPresentation(id, input, options);
	if (signal.aborted) throw new Error('Operation cancelled');
	decks.set(id, result.session);
	return { deck_id: id, revision: result.session.revision, slides: result.session.document.deck.slides.length, profile_id: result.profile_id, validation: result.validation, model_inference: false };
});
register('create_part', 'Preview a part without inserting it. Prefer apply_operations with add_part and PartSpec.layout for regenerable metadata. create_part plus add_elements is an explicit unmanaged choice, never a timeout fallback. Core validates bounds, numeric meaning and references. Deterministic design, not AI generation.', { id: z.string().min(1).max(40), spec: partSpec, theme: themeSchema.optional() }, true, async (input, signal) => client.createPart(input, { signal }));
for (const name of ['add_part', 'update_part']) {
	register(name, name === 'add_part' ? 'Insert a native metadata-driven part in one undoable revision. Presets and metadata schema come from part_catalog. Placement does not rearrange existing content.' : 'Update a part from metadata while retaining its placement, in one undoable revision. Rejects stale metadata after native edits; never restores a cached scene over external changes.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), slide_id: z.string().min(1).max(80), id: z.string().min(1).max(40), spec: partSpec }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
		const state = getDeck(deck_id);
		await (name === 'add_part' ? state.addPart(slide_id, input, { signal, expectedRevision: expected_revision }) : state.updatePart(slide_id, input, { signal, expectedRevision: expected_revision }));
		return { deck_id, revision: state.revision, element_id: input.id };
	});
}
register('design_defaults', 'Return a theme and editable native master/layout templates. Does not mutate a document.', {}, true, async (_input, signal) => client.designDefaults({ signal }));
register('design_capabilities', 'Read the implemented master-theme, native-field, Studio and preservation contract. Explicitly distinguishes supported slide fields from pending notes/handout masters and unverified Office recalculation.', {}, true, async (_input, signal) => client.designCapabilities({ signal }));
register('design_presets', 'List seven original native master presets with theme fonts, color roles, margins, gutters and layout regions. These are deterministic designs, not AI generation or copied third-party templates.', {}, true, async (_input, signal) => ({ presets: await client.designPresets({ signal }) }));
register('apply_design_preset', 'Apply a core-owned design preset with revision checking and Undo. Retains original masters and slide content; adds or replaces the dedicated preset master and layouts. Native packages that cannot add this structure are rejected. Does not automatically rearrange freeform objects or fetch fonts.', { deck_id: handle, expected_revision: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER), preset_id: z.enum(['public', 'minimal', 'stylish', 'pop', 'dynamic', 'trust', 'luxury']) }, false, async ({ deck_id, expected_revision, preset_id }, signal) => {
	const state = getDeck(deck_id); await state.applyDesignPreset(preset_id, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('set_master_theme', 'Set one master theme, or null to inherit the default. Retains other owners and native unknown XML; shared theme parts are copied before editing. No font fetching or installation.', { ...mutationInput, master_id: slideId, theme: themeSchema.nullable() }, false, async ({ deck_id, expected_revision, master_id, theme }, signal) => {
	const state = getDeck(deck_id); await state.setMasterTheme(master_id, theme, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('set_design_field', 'Create or update slide-number, datetime1 date, or footer placeholders for one master or layout and its slides. Explicit date, per-instance field UUIDs, one undoable revision. Notes and handout masters are unsupported; Office recalculation is unverified.', { ...mutationInput, field: z.object({ master_id: slideId, layout_id: optional(slideId), kind: z.enum(['slide_number', 'date', 'footer']), reference_date: z.string().regex(/^\d{4}-\d{2}-\d{2}$/), text: z.string().max(4000).optional() }).strict() }, false, async ({ deck_id, expected_revision, field }, signal) => {
	const state = getDeck(deck_id); await state.setDesignField(field, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('update_design', 'Replace the authored master/layout design, propagate linked placeholder geometry and formatting, and retain content. Unsupported changes to imported original packages are rejected. Revision and undo use the shared transaction engine.', { deck_id: handle, expected_revision: z.number().int().min(0), design: designSchema }, false, async ({ deck_id, expected_revision, design }, signal) => {
	const state = getDeck(deck_id); await state.updateDesign(design, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('apply_theme', 'Apply twelve native theme color slots and heading/body/script fonts. Colors matching the previous theme become live references; unrelated custom colors are retained. Set the theme before inserting managed parts/graphs: later changes can mark their metadata stale, and no automatic regeneration is performed. Never installs or fetches fonts.', { deck_id: handle, expected_revision: z.number().int().min(0), theme: themeSchema }, false, async ({ deck_id, expected_revision, theme }, signal) => {
	const state = getDeck(deck_id); await state.applyTheme(theme, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision };
});
register('assign_layout', 'Apply or reset a native slide layout while preserving existing matching text. Placeholder geometry and format follow later design edits until explicitly detached.', { deck_id: handle, expected_revision: z.number().int().min(0), slide_id: shortText, layout_id: z.string().min(1).max(80), preserve_freeform: z.boolean().optional() }, false, async ({ deck_id, expected_revision, slide_id, layout_id, preserve_freeform }, signal) => {
	const state = getDeck(deck_id); await state.assignLayout(slide_id, layout_id, { signal, expectedRevision: expected_revision, preserveFreeform: preserve_freeform }); return { deck_id, revision: state.revision };
});
register('create_object', 'Create a typed default object without inserting it. Charts contain synthetic examples. Supply final content and geometry through add_elements or apply_operations; core validates complete elements there.', objectInput, true, async (input, signal) => client.createObject(input, { signal }));
register('apply_operations', 'Apply 1-128 typed operations atomically with one Undo and revision/hash checks. Prefer add_part/add_graph with explicit layouts for regenerable metadata; add_elements is for unmanaged final elements. update_graph preserves existing PartLayout. Child edits followed by managed updates reject stale render hashes. Core retains locks, hidden-group, source and native guards, 128 total metadata entries and other capacity limits. Use smaller bounded chunks for progress/cancellation; never automatically fall back after timeout. set_text_style overlays; set_connector replaces settings. No raw Patch paths.', { ...mutationInput, expected_hash: contentHash, operations: authoringOperations }, false, async ({ deck_id, expected_revision, expected_hash, operations }, signal) => {
	const state = getDeck(deck_id);
	await state.applyOperations(operations, { signal, expectedRevision: expected_revision, expectedHash: expected_hash });
	return { deck_id, revision: state.revision, hash: state.document.hash, can_undo: state.canUndo };
});
const authoringDescriptions = {
	add_elements: 'Append 1-128 complete typed native elements in one Undo. Includes rich text, geometry, visual styles, tables, charts and groups. Core validates all content; no external fetches.',
	set_frame: 'Set one element frame, geometry only. Groups normalize child geometry and absolute table tracks scale; fonts and strokes remain unchanged. One Undo.',
	set_text_style: 'Overlay supplied RunStyle fields on 1-128 distinct text/shape targets and their defaults. Retains unspecified paragraph/run attributes. Unlike apply_format, this is not format painting. One Undo.',
	set_slide_background: 'Set one slide background color and detach background inheritance in one Undo.',
	set_connector: 'Replace all connector settings, optionally its frame, in one Undo. Omitted start/end/routing clear existing values; omitted flip_v becomes false. Visual properties remain unchanged.',
	set_picture_crop: 'Replace the crop of an existing picture in one Undo. Omitted crop edges become zero; core validates the visible region.',
	set_hyperlink: 'Set or clear a text/shape hyperlink in one Undo. Core validates allowed link targets without fetching them.',
	set_shape_adjustment: 'Set the existing adj guide of a supported roundRect, chevron or triangle in one Undo. Core rejects unsupported presets and values.',
	add_picture: 'Decode and append embedded PNG/JPEG bytes with optional final frame/crop in one core transaction and Undo. No linked images or execution. Optional revision retains legacy calls; supply it for stale-edit protection.',
};
for (const variant of authoringVariants) {
	const name = variant.shape.op.value;
	if (['add_part', 'update_part', 'add_graph', 'update_graph'].includes(name)) continue;
	register(name, authoringDescriptions[name], { ...mutationInput, ...(name === 'add_picture' ? { expected_revision: mutationInput.expected_revision.optional() } : {}), expected_hash: contentHash.optional(), ...variant.omit({ op: true }).shape }, false, async ({ deck_id, expected_revision, expected_hash, ...input }, signal) => {
		const state = getDeck(deck_id);
		await state.applyOperations([{ ...input, op: name }], { signal, expectedRevision: expected_revision, expectedHash: expected_hash });
		return { deck_id, revision: state.revision, hash: state.document.hash, ...(input.id === undefined ? {} : { element_id: input.id }) };
	});
}
register('set_frames', 'Set 1-128 frames on one slide in one Undo. Geometry only; fonts and strokes remain unchanged. Core validates the whole batch.', { ...mutationInput, expected_hash: contentHash.optional(), slide_id: slideId, frames: z.array(z.object({ id: slideId, frame: frameSchema }).strict()).min(1).max(128) }, false, async ({ deck_id, expected_revision, expected_hash, slide_id, frames }, signal) => {
	const state = getDeck(deck_id);
	await state.setFrames(slide_id, frames, { signal, expectedRevision: expected_revision, expectedHash: expected_hash });
	return { deck_id, revision: state.revision, hash: state.document.hash };
});
register('import_slides', 'Import 1-128 selected slides from an existing authored source deck handle into a same-canvas target, in one Undo. Core deduplicates design and preserves supported metadata, fonts and evidence bindings. Native sources and unsupported native target edits fail closed. Source notes/evidence may contain sensitive data. No paths or arbitrary source JSON.', { ...mutationInput, expected_hash: contentHash, source_deck_id: handle, source_slide_ids: authoringIds, prefix: z.string().regex(/^[A-Za-z0-9_-]{1,24}$/), after: optional(slideId) }, false, async ({ deck_id, expected_revision, expected_hash, source_deck_id, ...input }, signal) => {
	const state = getDeck(deck_id);
	const source = getDeck(source_deck_id).document;
	await state.importSlides(source, input, { signal, expectedRevision: expected_revision, expectedHash: expected_hash });
	return { deck_id, revision: state.revision, hash: state.document.hash, slides: state.document.deck.slides.length, can_undo: state.canUndo };
});
register('add_object', 'Insert a core-created default text box, shape, table, chart, line or arrow. Prefer add_elements for complete final content and geometry in one call. Charts contain synthetic examples, not factual data.', { deck_id: handle, expected_revision: z.number().int().min(0), slide_id: shortText, ...objectInput }, false, async ({ deck_id, expected_revision, slide_id, ...input }, signal) => {
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
register('compile_report', 'Compile structured content into native text, rectangles, tables, charts and process groups using fixed structured layouts. A shortcut, not the best path for exact recreation: use complete add_elements/apply_operations for freeform final geometry. Returns a session document handle. Deterministic layout, not a model call.', { report: reportSchema }, false, async ({ report }, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const result = await client.request({ op: 'compile', report }, options);
	const id = randomUUID();
	decks.set(id, await client.createDocument({ id, deck: result.deck, report }, options));
	return { deck_id: id, slides: result.deck.slides.length, revision: 0, issues: result.issues };
});
register('get_deck', 'Read the scene with stable slide and element IDs. Coordinates are pixels within deck.width and deck.height, each 320..4096.', { deck_id: handle }, true, async ({ deck_id }) => getDeck(deck_id).document.deck);
register('get_document', 'Read the revisioned document, source hashes and field bindings. Hashes detect accidental changes, not source authenticity.', { deck_id: handle }, true, async ({ deck_id }) => getDeck(deck_id).document);
register('open_pptx', 'Open one standard unencrypted Open XML PPTX without a checkpoint. Current native XML is authoritative; embedded provenance is optional. Unknown parts are preserved and unsupported edits fail. Encrypted/protected OLE containers and legacy .ppt are rejected without removing protection.', { base64: archiveBase64 }, false, async ({ base64 }, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const id = randomUUID(); const opened = await client.openPresentation(id, base64, options); decks.set(id, opened.session);
	return { deck_id: id, revision: 0, slides: opened.session.document.deck.slides.length, format: opened.format, warnings: opened.warnings, objects: opened.objects };
});
register('import_pptx', 'Open a caller-provided PPTX for approximate native preview and strictly limited non-destructive edits. Preserves original package bytes, including opaque parts. Unsupported content is not executed or fetched. Original byte identity is preserved on no-op export.', { base64: archiveBase64 }, false, async ({ base64 }, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const id = randomUUID(); const imported = await client.importPresentation(id, base64, options); decks.set(id, imported.session);
	return { deck_id: id, revision: 0, slides: imported.session.document.deck.slides.length, warnings: imported.warnings, objects: imported.objects };
});
register('measure_layout', 'Measure text and table cells with installed fonts and cosmic-text. Reports overflow, missing glyphs and fallback fonts. This is not Office parity and does not measure charts or images.', { deck_id: handle }, true, async ({ deck_id }, signal) => requestCore({ op: 'measure_layout', capacity_profile: getDeck(deck_id).capacityProfile, deck: getDeck(deck_id).document.deck }, { signal }));
register('update_text', 'Set a text element without re-layout. The geometry stays fixed; validate visual fit separately. Optional expected_revision protects against stale edits.', { deck_id: handle, slide_id: shortText, element_id: shortText, text: z.string().max(4000), expected_revision: z.number().int().min(0).optional() }, false, async ({ deck_id, slide_id, element_id, text, expected_revision }, signal) => {
	const state = getDeck(deck_id);
	const revision = state.revision;
	if (expected_revision !== undefined && revision !== expected_revision) throw new Error('Revision conflict');
	const deck = state.document.deck;
	const slideIndex = deck.slides.findIndex((slide) => slide.id === slide_id);
	const elementIndex = deck.slides[slideIndex]?.elements.findIndex((item) => item.id === element_id);
	 if (elementIndex === undefined || !['text', 'shape'].includes(deck.slides[slideIndex].elements[elementIndex]?.type)) throw new Error('Target must be an existing top-level text or shape element');
	await state.replaceTextContent(slide_id, { id: element_id, text }, { signal, expectedRevision: revision });
	return { deck_id, revision: state.revision, warnings: ['Text fit is not checked by structural validation'] };
});
register('undo', 'Undo the previous complete transaction using core-verified inverse operations.', { deck_id: handle }, false, async ({ deck_id }, signal) => {
	const state = getDeck(deck_id);
	await state.undo({ signal });
	return { deck_id, revision: state.revision };
});
register('redo', 'Reapply the last undone transaction; later edits invalidate redo history.', { deck_id: handle }, false, async ({ deck_id }, signal) => { const state = getDeck(deck_id); await state.redo({ signal }); return { deck_id, revision: state.revision }; });
register('validate_deck', 'Validate IDs, object types, text safety and page geometry. This does not prove Office visual parity or text fit.', { deck_id: handle }, true, async ({ deck_id }, signal) => requestCore({ op: 'validate', capacity_profile: getDeck(deck_id).capacityProfile, deck: getDeck(deck_id).document.deck }, { signal }));
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
	expireCandidates();
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
register('export_project', 'Legacy compatibility only: save a new PPTX and hash-bound .aislide.json checkpoint. Prefer export_pptx for single-file operation. Never overwrites files; pair publication is not crash-atomic.', { deck_id: handle, filename: filenameSchema }, false, async ({ deck_id, filename }, signal) => {
	if (!outputDirectory) throw new Error('Saving requires --output-dir at server startup');
	const result = await getDeck(deck_id).exportProject({ signal });
	return publishProject(outputDirectory, filename, result, signal);
});
register('open_project', 'Legacy compatibility only: open an exact PPTX/checkpoint pair. Prefer open_pptx for a standalone presentation. Does not read paths, fetch content or ignore integrity failures.', { base64: archiveBase64, checkpoint: z.object({ format: z.literal('aislide.project'), version: z.literal(1), pptx_sha256: z.string().length(64), document: z.record(z.string(), z.unknown()) }).strict() }, false, async (input, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const state = await client.openProject(input, options); const id = randomUUID(); decks.set(id, state);
	return { deck_id: id, revision: state.revision, slides: state.document.deck.slides.length };
});

server.registerResource('report-example', 'aislide://report/example', { mimeType: 'application/json' }, async () => ({ contents: [{ uri: 'aislide://report/example', mimeType: 'application/json', text: JSON.stringify(await requestCore({ op: 'sample' })) }] }));

const authoringWorkflow = [
	'Create an editable presentation using the AISlide MCP tools in this connection.',
	'Read authoring_capabilities, including typed_authoring, and choose the authoring path for the task. Establish audience, purpose, evidence and assumptions; do not invent facts or fetch imported relationships.',
	'For freeform, exact recreation or high-volume authoring, use create_presentation and apply_operations with complete add_elements at their final frames. Each atomic batch accepts 1..128 typed operations; successful changed batches create one Undo entry, while no-ops create none. Supply expected_revision and expected_hash from get_document. set_frame/set_frames change geometry only. set_text_style is a partial style update; apply_format copies a complete compatible style. Do not force a preview_slide_revision candidate for every frame change. Finish with preview_presentation and preflight_presentation.',
	'When parts or graphs need later regeneration, prefer apply_operations with add_part/add_graph and explicit layouts: the batch retains both native elements and managed metadata in one transaction. Mixed batches supersede separate per-part calls without removing the existing individual tools. Use create_part/create_graph plus add_elements ONLY as an explicit unmanaged choice; never automatically fall back to ordinary groups after a timeout or rejection. Editing a managed child then update_part/update_graph in the same batch rejects the stale render hash atomically.',
	'Set theme before add_part/add_graph. Applying a theme afterward can change rendered child styles and mark managed metadata stale; there is no automatic theme-driven regeneration. Use set_accessibility after the target exists in a separate revision, not inside add_part/add_graph or apply_operations. Accessibility updates use at least 65 seconds with a size-aware finite budget up to 180 seconds, and support opt-in MCP progress; the client deadline must allow the operation to finish. Do not blindly retry mutations. All three venn variants support PartSpec.layout.show_title=false; this does not remove clipping guards from other presets.',
	'Keep batches bounded: 1..128 operations, at most 128 metadata entries in the document, plus all selected capacity limits. Reduce chunk size for progress and cancellation responsiveness; these do not make limits unlimited. Each committed chunk has its own Undo. After timeout or cancellation, inspect get_document/get_session_recovery before retrying; do not blindly repeat a mutation.',
	'For evidence-led guided decks, read best_practice_guide for the chosen profile. Guided defaults remain sentence headlines and a 32-slide limit. Set input.authoring.headline_style="keyword" to opt into keyword-headline validation. For a 39-slide deck, use input.authoring.slide_limit=39 or a higher ceiling (explicit range 32..128). Evidence support and numeric declarations still apply. Consulting issues are required only for the consulting-decision profile.',
	'Use optional input.authoring for reading/projection context, density, spacing, body_font_min, headline_font_size and font_family; existing brand_color controls the palette. Add speaker_notes to each input slide when supplied.',
	'For the guided path, run validate_guided_presentation, resolve unmet checks, then create_guided_presentation. ready means compilable, not factual truth or Office parity. compile_report is a fixed structured-layout shortcut, not the best path for exact recreation.',
	'For positioned native parts, use PartSpec.layout with show_title=false and an explicit body box (x, y, width, height). For graphs, GraphSpec.show_title=false removes the fixed title band; node.detail, text_align and heading_bold separate heading and detail presentation. Native part fitting enforces a 12px text floor and can reject a box that is too small. Keep layout when updating parts or graphs across APIs. Only batch add_graph accepts a top-level layout (PartLayout or null); batch update_graph has no layout field and preserves the existing PartLayout. Matrix and contrast data accept corner_label, at most 48 Unicode scalars, default empty; core omits empty labels from serialization.',
	'For cross-document reuse, import_slides takes an authored source_deck_id handle and selected source_slide_ids, reusing matching masters. Source and target must use the same canvas. It does not support arbitrary native cross-package slide import; opening native bytes does not turn them into an authored source or permit origin rewriting.',
	'Use preview_presentation for actual PNG/JPEG pages or a contact_sheet; choose at most eight unique zero-based page_indices per call. Defaults are format=png and overflow=shrink; encoded-byte overflow can lower dimensions in at most three attempts while retaining all pages. Inspect quality_reduced, actual_max_dimension and PREVIEW_DOWNSCALED. Use overflow=error to forbid shrinking or format=jpeg for photo-heavy pages. include_images=false returns metadata only but still renders; get_document reads revision/hash without rendering.',
	'Run preflight_presentation for the same pages and a suitable min_font_size. Findings include geometry and readability heuristics, not guaranteed defects. Run check_accessibility separately when needed.',
	'Keep card text at least 8 slide pixels from the container edges and clear of rounded corners. Inset or shorten an accent bar so it does not protrude beyond a rounded outline; use a rectangular card when a flush full-height accent is intentional. Reserve separate regions for connector labels, numbered badges and node descriptions instead of shrinking every font. CONTAINER_CORNER_OVERFLOW and CONTAINER_PADDING infer containers and require preview review. CONNECTOR_BADGE_OVERLAP is info for a compact opaque numbered ellipse over a center-crossing line, not a visual approval; retain real label-interference warnings. For an existing native slide, propose guarded edits and inspect before/after previews instead of silently moving objects or suppressing all overlap findings.',
	'When a correction needs before/after review before mutation, use preview_slide_revision with deck_id, expected_revision, expected_hash, slide_id and typed edits. Inspect before/after images, affected_ids, stale_part_ids and source_bindings_stale. No changes have been applied yet.',
	'Apply only the agreed candidate using apply_slide_revision with the candidate_id and exact base revision/hash. Candidates expire after ten minutes, deck closure or any revision change; at most sixteen are retained. Undo reverses one applied batch. Native preservation may reject unsupported edits.',
	'Review again, then finalize_presentation with deck_id, exact expected_revision/expected_hash and a new plain name under the operator-approved output directory. It always exports the complete PPTX and manifest. Optional PDF/previews/preflight cover selected pages only (at most eight); notes and source_report require explicit opt-in because they may contain sensitive plaintext. The PPTX itself retains notes and may include source data. Individual export_pptx/export_static remain available.',
	'Finalization stages all files and publishes the manifest last without overwriting existing files. Return actual paths, sizes, SHA-256 hashes and check scopes. Partial failures retain published files; do not blindly retry the same name. Complete means output generation, not a visual or factual approval; Office parity remains unverified.',
	'No automatic persistence, restart recovery, source freshness verification or background model generation is provided by this workflow.',
].join('\n\n');
server.registerResource('authoring-workflow', 'aislide://authoring/workflow', { mimeType: 'text/plain', description: 'Choose typed batches, guided decks, positioned parts or authored-slide reuse, then visually review and export.' }, async () => ({ contents: [{ uri: 'aislide://authoring/workflow', mimeType: 'text/plain', text: authoringWorkflow }] }));
server.registerPrompt('author_presentation', { description: 'Choose an authoring path, create, preview, diagnose, revise and export an editable presentation with AISlide.' }, async () => ({ messages: [{ role: 'user', content: { type: 'text', text: authoringWorkflow } }] }));

await server.connect(new StdioServerTransport(process.stdin, process.stdout, { maxBufferSize: MAX_REQUEST_BYTES }));
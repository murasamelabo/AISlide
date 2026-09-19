import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { z } from 'zod';
import { randomUUID, createHash } from 'node:crypto';
import { mkdir, realpath, lstat } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { requestCore, MAX_REQUEST_BYTES } from './core-client.mjs';
import { CAPACITY_PROFILES, FONT_LIMITS } from '../packages/client/index.mjs';
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
const currentVisualSchema = visualSchema.extend({ path: optional(pathSchema) }).strict();
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
const elementSchema = z.lazy(() => z.discriminatedUnion('type', [
	z.object({ type: z.literal('text'), ...boundsSchema, ...textFields, visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('rect'), ...boundsSchema, fill: colorSchema, visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('polygon'), ...boundsSchema, points: z.array(pointSchema).min(3).max(4096), fill: z.union([colorSchema, z.literal('none')]), stroke: colorSchema, stroke_width: z.number().min(0).max(20), visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('shape'), ...boundsSchema, ...textFields, preset: z.string().min(1).max(80), fill: z.union([colorSchema, z.literal('none')]), stroke: colorSchema, stroke_width: z.number().min(0).max(20), rotation: z.number().min(-360).max(360).optional(), visual: optional(currentVisualSchema) }).strict(),
	z.object({ type: z.literal('table'), ...boundsSchema, rows: z.array(z.array(z.string().max(200)).min(1).max(8)).min(1).max(12), font_size: z.number().min(8).max(120), format: tableFormatSchema.optional() }).strict(),
	z.object({ type: z.literal('chart'), ...boundsSchema, ...chartSchema.shape }).strict(),
	z.object({ type: z.literal('picture'), ...boundsSchema, base64: z.string().max(1398104), mime_type: z.enum(['image/png', 'image/jpeg']), alt: z.string().max(500), crop: z.object({ left: alpha.optional(), right: alpha.optional(), top: alpha.optional(), bottom: alpha.optional() }).strict().optional(), visual: optional(currentVisualSchema), svg: optional(svgSchema) }).strict(),
	z.object({ type: z.literal('connector'), ...boundsSchema, color: colorSchema, stroke_width: z.number().min(0).max(20), arrow: z.boolean(), flip_v: z.boolean().optional(), start: optional(connectionSchema), end: optional(connectionSchema), routing: optional(z.object({ points: z.array(pointSchema).min(2).max(4), start_arrow: z.boolean().optional(), dashed: z.boolean().optional() }).strict()), visual: optional(currentVisualSchema) }).strict(),
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
const templateKind = z.enum(['potx', 'thmx']);
const templateFilename = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,79}\.(?:potx|thmx)$/).refine((name) => !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])\./i.test(name), 'Reserved Windows filename');

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
	const selectsProfile = ['create_presentation', 'compile_report', 'open_pptx', 'import_pptx', 'open_project', 'import_template', 'create_guided_presentation', 'verify_recovery', 'recover_presentation'].includes(name);
	server.registerTool(name, {
		description,
		inputSchema: z.object({ ...inputSchema, ...(selectsProfile ? { capacity_profile: capacityProfileSchema.optional() } : {}) }).strict(),
		annotations: { readOnlyHint: readOnly, destructiveHint: false, openWorldHint: name === 'generate_report' },
	}, async (input, extra) => {
		if (!readOnly && activeMutation) return { isError: true, content: [{ type: 'text', text: 'Another mutation is in progress' }] };
		if (!readOnly) activeMutation = true;
		try {
			const { capacity_profile, ...parameters } = input;
			const profile = capacity_profile ?? (input.deck_id ? getDeck(input.deck_id).capacityProfile : 'large');
			const budget = CAPACITY_PROFILES[profile].request_bytes;
			if (Buffer.byteLength(JSON.stringify(input), 'utf8') > budget) throw new Error('JSON input exceeds selected capacity profile');
			if (extra.signal.aborted) throw new Error('Operation cancelled');
			const result = await action(parameters, extra.signal, { signal: extra.signal, capacityProfile: profile });
			const text = JSON.stringify(result);
			const response = { content: [{ type: 'text', text }], ...(Buffer.byteLength(text) <= 65536 ? { structuredContent: result } : {}) };
			if (Buffer.byteLength(JSON.stringify(response), 'utf8') > budget) throw new Error('JSON tool output exceeds selected capacity profile; request a smaller result');
			return response;
		} catch (error) {
			return { isError: true, content: [{ type: 'text', text: error instanceof Error ? error.message : 'Operation failed' }] };
		} finally { if (!readOnly) activeMutation = false; }
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
register('set_accessibility', 'Set or remove typed accessibility metadata in one revision. Picture descriptions map to alt text. Decorative is not an accessibility certification.', { ...mutationInput, slide_id: slideId, element_id: slideId, metadata: accessibilitySchema.nullable() }, false, async ({ deck_id, expected_revision, slide_id, element_id, metadata }, signal) => {
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
register('create_guided_presentation', 'Create a NEW evidence-led presentation from supplied claims and native parts after the same strict validation. Consulting multi-page decks require 3-6 issues, C02 summary and C03 close. Unknown numbers must remain xx in qualitative content. Does not invent content, call a model, overwrite a deck or save a file; use export_pptx separately. Review semantics and actual rendering.', { input: guidedInput }, false, async ({ input }, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session; close a deck first');
	const id = randomUUID(); const result = await client.createGuidedPresentation(id, input, options);
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
register('apply_theme', 'Apply twelve native theme color slots and heading/body/script fonts. Colors matching the previous theme become live references; unrelated custom colors are retained. Never installs or fetches fonts.', { deck_id: handle, expected_revision: z.number().int().min(0), theme: themeSchema }, false, async ({ deck_id, expected_revision, theme }, signal) => {
	const state = getDeck(deck_id); await state.applyTheme(theme, { signal, expectedRevision: expected_revision }); return { deck_id, revision: state.revision };
});
register('assign_layout', 'Apply or reset a native slide layout while preserving existing matching text. Placeholder geometry and format follow later design edits until explicitly detached.', { deck_id: handle, expected_revision: z.number().int().min(0), slide_id: shortText, layout_id: z.string().min(1).max(80), preserve_freeform: z.boolean().optional() }, false, async ({ deck_id, expected_revision, slide_id, layout_id, preserve_freeform }, signal) => {
	const state = getDeck(deck_id); await state.assignLayout(slide_id, layout_id, { signal, expectedRevision: expected_revision, preserveFreeform: preserve_freeform }); return { deck_id, revision: state.revision };
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
register('compile_report', 'Compile structured content into native text, rectangles, tables, charts and process groups. Returns a session document handle. Deterministic layout, not a model call.', { report: reportSchema }, false, async ({ report }, signal, options) => {
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
register('open_project', 'Legacy compatibility only: open an exact PPTX/checkpoint pair. Prefer open_pptx for a standalone presentation. Does not read paths, fetch content or ignore integrity failures.', { base64: archiveBase64, checkpoint: z.object({ format: z.literal('aislide.project'), version: z.literal(1), pptx_sha256: z.string().length(64), document: z.record(z.string(), z.unknown()) }).strict() }, false, async (input, signal, options) => {
	if (decks.size >= 8) throw new Error('At most eight decks per session');
	const state = await client.openProject(input, options); const id = randomUUID(); decks.set(id, state);
	return { deck_id: id, revision: state.revision, slides: state.document.deck.slides.length };
});

server.registerResource('report-example', 'aislide://report/example', { mimeType: 'application/json' }, async () => ({ contents: [{ uri: 'aislide://report/example', mimeType: 'application/json', text: JSON.stringify(await requestCore({ op: 'sample' })) }] }));

await server.connect(new StdioServerTransport(process.stdin, process.stdout, { maxBufferSize: MAX_REQUEST_BYTES }));
import { test, expect, type Page } from '@playwright/test';
import { resolve } from 'node:path';
import AxeBuilder from '@axe-core/playwright';
import sharp from 'sharp';
import type { AislideDocument, Element } from '../../packages/client/types';
import { waitForCoreOperation } from './fixtures';

const bounds = { id: 'object', x: 40, y: 80, width: 500, height: 300 };
const chart: Element = { ...bounds, type: 'chart', kind: 'combo', categories: ['A', 'B'], series: [
  { name: 'Revenue', values: [20, 30], color: '087F73', kind: 'column', axis: 'primary', error_bars: { kind: 'custom', direction: 'y', bar_type: 'both', plus: [2, 3], minus: [1, 2] } },
  { name: 'Rate', values: [2, 3], color: 'AA3333', kind: 'line', axis: 'secondary', trendline: { kind: 'linear', display_r_squared: true } },
], options: { primary_axis: { min: 0, max: 50, minor_unit: 1 }, secondary_axis: { min: 0, max: 5 }, legend: 'right', data_labels: { show_value: true, number_format: '0.00' } } };
const table: Element = { ...bounds, type: 'table', font_size: 20, rows: [['A', ''], ['', '']], format: { column_widths: { unit: 'relative', values: [1, 2] }, cells: [{ row: 0, column: 0, style: { fill: 'FFFFFF', text_format: { paragraphs: [{ runs: [{ text: 'A', style: { italic: true } }] }] } } }] } };
const richTable: Element = { ...table, rows: [['AB', ''], ['', '']], format: { ...table.format, cells: [{ row: 0, column: 0, style: { fill: 'FFFFFF', text_format: { paragraphs: [{ runs: [{ text: 'A', style: { italic: true } }, { text: 'B', style: { bold: true } }] }] } } }] } };
const text: Element = { ...bounds, type: 'text', text: 'First\nSecond', font_size: 24, color: '000000', bold: false, format: { paragraphs: [{ alignment: 'right', runs: [{ text: 'First', style: { italic: true } }] }, { runs: [{ text: 'Second', style: { bold: true } }] }] } };

type RequestLog = { op: string; [key: string]: unknown };
async function mount(page: Page, mode: 'object' | 'review' | 'setup' | 'inline', element: Element = chart, real = false, native = false) {
  if (real) test.setTimeout(90_000);
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const calls: RequestLog[] = [];
  page.on('request', (request) => { if (request.url().endsWith('/api/core')) calls.push(request.postDataJSON()); });
  if (!real) await page.route('**/api/core', async (route) => {
    const request = route.request().postDataJSON();
    if (request.op === 'edit_vector') {
      const element = structuredClone(request.element);
      element.points = request.path.commands.flatMap((command: { op: string; point?: number[]; control?: number[]; control1?: number[]; control2?: number[] }) => command.op === 'close' ? [] : [command.control, command.control1, command.control2, command.point].filter(Boolean));
      element.visual = { ...element.visual, path: request.path };
      return route.fulfill({ json: element });
    }
    if (request.op === 'edit_image') return route.fulfill({ json: { base64: request.base64, mime_type: request.mime_type, width: 2, height: 2 } });
    if (request.op === 'inspect_document') return route.fulfill({ json: { findings: [{ category: 'notes', count: 1, paths: ['slides/0/notes'] }, { category: 'comments', count: 0, paths: [] }], limitations: ['No complete personal-data detection.'], complete_personal_data_detection: false } });
    if (request.op === 'check_accessibility') return route.fulfill({ json: { issues: [{ code: 'description_missing', slide_id: 'slide', element_id: 'object', status: 'warning', contrast_ratio: null, required_ratio: null }], checked_features: ['image_descriptions'], limitations: ['Not a WCAG certification.'], wcag_certified: false } });
    if (request.op === 'export_template') return route.fulfill({ json: { base64: 'UEs=', filename: `template.${request.kind}` } });
    if (request.op === 'export_clean_copy') return route.fulfill({ json: { document: { ...request.document, id: request.options.new_document_id }, base64: 'UEs=', filename: 'fixture.pptx', inspection: {} } });
    const document = structuredClone(request.document);
    if (!document) return route.fulfill({ status: 400, json: { error: 'Unsupported test request: ' + request.op } });
    document.revision += 1;
    document.hash = 'fixture-' + document.revision;
    const slide = document.deck.slides[0];
    if (request.op === 'transaction') for (const operation of request.transaction.operations) {
      const path = operation.path.split('/').slice(1);
      let target = document;
      for (const key of path.slice(0, -1)) target = target[key];
      target[path.at(-1)!] = operation.value;
    }
    if (request.op === 'resize_canvas') { document.deck.width = request.width; document.deck.height = request.height; }
    if (request.op === 'update_paragraphs') { slide.elements[0].format.paragraphs = request.paragraphs; slide.elements[0].text = request.paragraphs.map((paragraph: { runs: { text: string }[] }) => paragraph.runs.map((run) => run.text).join('')).join('\n'); }
    if (request.op === 'add_comment' || request.op === 'reply_comment') { slide.review ??= {}; slide.review.comments ??= []; slide.review.comments.push({ ...request.comment, parent_id: request.parent_id ?? null }); }
    if (request.op === 'resolve_comment') slide.review.comments.find((comment: { id: string }) => comment.id === request.comment_id).resolved = request.resolved;
    if (request.op === 'remove_comment') slide.review.comments = slide.review.comments.filter((comment: { id: string; parent_id: string }) => comment.id !== request.comment_id && comment.parent_id !== request.comment_id);
    if (request.op === 'set_accessibility') { slide.review ??= {}; slide.review.accessibility = { [request.element_id]: request.metadata }; }
    return route.fulfill({ json: { document, receipt: null, ...(request.op === 'set_reading_order' ? { warnings: ['Z-order updated.'] } : {}) } });
  });
  const clientUrl = `/@fs/${resolve('packages/client/index.mjs').replaceAll('\\', '/')}`;
  await page.route('**/__object-panels-test', (route) => route.fulfill({ contentType: 'text/html; charset=utf-8', body: `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Standalone panel test</title></head><body style="margin:0"><main id="root" style="max-width:380px;margin:auto"></main><script type="module">
    import RefreshRuntime from '/@react-refresh';
    RefreshRuntime.injectIntoGlobalHook(window);
    window.$RefreshReg$ = () => {}; window.$RefreshSig$ = () => (type) => type;
    window.__vite_plugin_react_preamble_installed__ = true;
    const panelModule = await fetch('/src/ObjectToolsPanel.tsx').then((response) => response.text());
    const mainModule = await fetch('/src/main.tsx').then((response) => response.text());
    const toolModule = await fetch('/src/Tool.tsx').then((response) => response.text());
    const { default: React } = await import(panelModule.split('"').find((url) => url.includes('/react.js?')));
    const { default: ReactDOM } = await import(mainModule.split('"').find((url) => url.includes('/react-dom_client.js?')));
    const Tooltip = await import(toolModule.split('"').find((url) => url.includes('react-tooltip.js?')));
    await import('/src/studio.css');
    const { ObjectToolsPanel } = await import('/src/ObjectToolsPanel.tsx');
    const { ChartSurface } = await import('/src/ChartSurface.tsx');
    const { RichTextSurface } = await import('/src/RichTextSurface.tsx');
    const { InlineEditor } = await import('/src/InlineEditor.tsx');
    const { DocumentSetupPanel } = await import('/src/DocumentSetupPanel.tsx');
    const { ReviewPanel } = await import('/src/ReviewPanel.tsx');
    const { AislideClient, DocumentSession } = await import(${JSON.stringify(clientUrl)});
    const transport = async (request) => { const response = await fetch('/api/core', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(request) }); const value = await response.json(); if (!response.ok) throw new Error(value.error || 'Core request failed'); return value; };
    const fixture = { version: 1, id: 'panel-fixture', revision: 0, hash: 'fixture', sources: [], bindings: [], deck: { version: 1, title: 'Synthetic panel fixture', width: 1280, height: 720, slides: [{ id: 'slide', title: 'Fixture', background: 'FFFFFF', notes: 'Initial notes', elements: [${JSON.stringify(element)}, { ...${JSON.stringify(text)}, id: 'other', y: 450, height: 100 }] }] } };
    const client = new AislideClient(transport);
    const authored = ${real} ? await client.createDocument({ id: fixture.id, deck: fixture.deck }) : new DocumentSession(transport, fixture);
    const source = ${native} ? await authored.exportPresentation() : null;
    const initial = source ? (await client.openPresentation(fixture.id, source.base64)).session : authored;
    window.panelHarness = { initial, client, source, exports: [], changes: [], busy: [], replacements: [], cancelExport: false };
    function Harness() {
      const [session, setSession] = React.useState(initial);
      const [document, setDocument] = React.useState(initial.document);
      const onDocument = (document) => { window.panelHarness.changes.push(document); setDocument(document); };
      window.panelHarness.replacePreviewElement = async (element) => onDocument(await session.transact([{ op: 'replace', path: '/deck/slides/0/elements/0', value: element }]));
      const props = { session, onDocument, onBusy: (busy) => window.panelHarness.busy.push(busy), onExport: async (bytes, filename, mime) => { window.panelHarness.exports.push({ bytes: Array.from(bytes), filename, mime }); return !window.panelHarness.cancelExport; } };
      const control = ${JSON.stringify(mode)} === 'inline' ? React.createElement(InlineEditor, { element: document.deck.slides[0].elements[0], onCommit: async (element) => onDocument(await session.transact([{ op: 'replace', path: '/deck/slides/0/elements/0', value: element }], { expectedRevision: document.revision })), onCancel: () => window.panelHarness.cancelled = true, onBusy: props.onBusy }) : ${JSON.stringify(mode)} === 'object' ? React.createElement(ObjectToolsPanel, { ...props, element: document.deck.slides[0].elements[0], slideId: 'slide' }) : ${JSON.stringify(mode)} === 'review' ? React.createElement(ReviewPanel, { ...props, slideId: 'slide', selectedId: 'object', onNavigate: (...args) => window.panelHarness.navigation = args }) : React.createElement(DocumentSetupPanel, { ...props, onBeforeOpen: async () => { window.panelHarness.beforeOpen = true; return window.panelHarness.allowOpen !== false; }, onReplaceSession: (next) => { window.panelHarness.replacements.push(next.document); setSession(next); setDocument(next.document); } });
      return React.createElement(React.Fragment, {}, control,
        ${real} && document.deck.slides[0].elements[0].type === 'chart' ? React.createElement('div', { 'data-testid': 'chart-preview', style: { width: 320, height: 220 } }, React.createElement(ChartSurface, { element: { ...document.deck.slides[0].elements[0], width: 320, height: 220 } })) : null,
        ${real} && document.deck.slides[0].elements[0].type === 'text' ? React.createElement('div', { 'data-testid': 'wordart-preview', style: { position: 'relative', width: 320, height: 180, color: '#087F73', fontWeight: 700 } }, React.createElement(RichTextSurface, { text: document.deck.slides[0].elements[0].text, format: document.deck.slides[0].elements[0].format, size: document.deck.slides[0].elements[0].font_size, warp: document.deck.slides[0].elements[0].visual?.text_warp })) : null,
        React.createElement('button', { onClick: async () => onDocument(await session.undo()) }, 'Undo test edit'),
        React.createElement('button', { onClick: async () => setDocument(await session.updateNotes('slide', 'Concurrent update')) }, 'Concurrent change'),
        React.createElement('button', { onClick: () => { const next = new DocumentSession(transport, { ...fixture, id: 'other-session' }); setSession(next); setDocument(next.document); } }, 'Switch session'),
        React.createElement('button', { onClick: () => { const replaced = structuredClone(fixture); if (replaced.deck.slides[0].elements[0].type === 'chart') replaced.deck.slides[0].elements[0].series[0].values[0] = 80; const next = new DocumentSession(transport, replaced); setSession(next); setDocument(next.document); } }, 'Replace same identity'),
        React.createElement('output', { hidden: true, 'data-testid': 'document' }, JSON.stringify(document)));
    }
    ReactDOM.createRoot(document.getElementById('root')).render(React.createElement(Tooltip.Provider, {}, React.createElement(Harness)));
    document.documentElement.dataset.panelHarness = 'ready';
  </script></body></html>` }));
  await page.goto('/__object-panels-test');
  await page.waitForFunction(() => document.documentElement.dataset.panelHarness === 'ready');
  expect(errors).toEqual([]);
  if (mode === 'inline') await expect(page.getByRole('button', { name: 'Apply on-slide edit' })).toBeVisible({ timeout: real ? 30_000 : 5_000 });
  else await expect(page.getByRole('region', { name: mode === 'object' ? 'Object tools' : mode === 'setup' ? 'Document setup' : 'Review', exact: true })).toBeVisible({ timeout: real ? 30_000 : 5_000 });
  return { calls, errors };
}
async function documentOf(page: Page): Promise<AislideDocument> { return JSON.parse(await page.getByTestId('document').textContent() ?? '{}'); }
async function waitRevision(page: Page, revision: number) { await expect.poll(async () => (await documentOf(page)).revision).toBe(revision); }

test('chart transaction preserves custom series, axes, labels and all twenty-four kind choices', async ({ page }) => {
  const { calls, errors } = await mount(page, 'object');
  await expect(page.getByLabel('Chart kind').locator('option')).toHaveCount(24);
  await page.getByLabel('Category 1', { exact: true }).fill('Updated');
  await page.getByLabel('Series 1 value 1', { exact: true }).fill('25');
  await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
  await waitRevision(page, 1);
  const current = (await documentOf(page)).deck.slides[0].elements[0];
  expect(current).toEqual({ ...chart, categories: ['Updated', 'B'], series: chart.type === 'chart' ? chart.series.map((series, index) => index ? series : { ...series, values: [25, 30] }) : [] });
  expect(calls.filter((request) => request.op === 'transaction')).toHaveLength(1);
  expect(calls.at(-1)?.transaction).toMatchObject({ expected_revision: 0, operations: [{ op: 'replace', path: '/deck/slides/0/elements/0' }] });
  expect(errors).toEqual([]);
});

test('chartEx waterfall total controls retain native edits and byte-exact Undo', async ({ page }) => {
  const waterfall: Element = { ...bounds, type: 'chart', kind: 'waterfall', categories: ['Start','Change','End'], series: [{ name: 'Synthetic native', values: [120,-80,40], color: '087F73' }], options: { waterfall_totals: [0,2] } };
  const { errors } = await mount(page, 'object', waterfall, true, true);
  await expect(page.getByRole('button', { name: 'Add chart series', exact: true })).toBeDisabled();
  await expect(page.getByLabel('Step 3 is total')).toBeChecked();
  await page.getByLabel('Step 1 is total').uncheck();
  await page.getByRole('button', { name: 'Delete category 2', exact: true }).click();
  await expect(page.getByLabel('Step 2 is total')).toBeChecked();
  await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
  await waitRevision(page, 1);
  const current = (await documentOf(page)).deck.slides[0].elements[0];
  expect(current).toMatchObject({ kind: 'waterfall', categories: ['Start','End'], series: [{ values: [120,40] }], options: { waterfall_totals: [1] } });
  expect(await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; return (await client.openPresentation('chartex-reopen', (await initial.exportPresentation()).base64)).session.document.deck.slides[0].elements[0]; })()`)).toEqual(current);
  await page.getByRole('button', { name: 'Undo test edit', exact: true }).click();
  await waitRevision(page, 2);
  expect(await page.evaluate(`(async () => (await window.panelHarness.initial.exportPresentation()).base64 === window.panelHarness.source.base64)()`)).toBe(true);
  expect(errors).toEqual([]);
});

test('chartEx raw sample form retains samples and explicit bin controls', async ({ page }) => {
  const histogram: Element = { ...bounds, type: 'chart', kind: 'histogram', categories: [], series: [{ name: 'Synthetic samples', values: [], color: '087F73' }], options: { histogram: { samples: [1, 2, 3, 4], binning: { rule: 'count', count: 2 }, interval_closed: 'right' } } };
  const { calls, errors } = await mount(page, 'object', histogram);
  await page.getByLabel('Raw samples 1', { exact: true }).fill('1, 2, 2, 5, 8');
  await page.getByLabel('Bin rule', { exact: true }).selectOption('width');
  await page.getByLabel('Bin width', { exact: true }).fill('2');
  await page.getByLabel('Underflow threshold', { exact: true }).fill('1');
  await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
  await waitRevision(page, 1);
  expect((await documentOf(page)).deck.slides[0].elements[0]).toMatchObject({ categories: [], series: [{ values: [] }], options: { histogram: { samples: [1, 2, 2, 5, 8], binning: { rule: 'width', width: 2 }, underflow: 1 } } });
  expect(calls.filter((request) => request.op === 'transaction')).toHaveLength(1);
  expect(errors).toEqual([]);
});

for (const kind of ['histogram', 'box_whisker', 'treemap', 'sunburst'] as const) {
  test(`chartEx ${kind} real native controls save reopen reject and recover`, async ({ page }, testInfo) => {
    const statistical = kind === 'histogram' || kind === 'box_whisker';
    const element: Element = { ...bounds, type: 'chart', kind, categories: kind === 'histogram' ? [] : ['A', 'B'], series: [{ name: 'Synthetic raw data', values: statistical ? [] : [8.25, 12.5], color: '087F73' }], options: kind === 'histogram' ? { histogram: { samples: [1, 2, 2, 4, 8], binning: { rule: 'count', count: 3 }, interval_closed: 'right' } } : kind === 'box_whisker' ? { box_whisker: { samples: [[1, 2, 3, 4], [2, 4, 8, 12, 40]], quartile_method: 'inclusive', mean_line: false, mean_marker: true, nonoutliers: false, outliers: true } } : { hierarchy: { paths: [['North', 'A'], ['South', 'B']] } } };
    const { errors } = await mount(page, 'object', element, true, true);
    await expect(page.getByLabel('Chart kind').locator('option')).toHaveCount(24);
    if (statistical) await page.getByLabel('Raw samples 1', { exact: true }).fill('1, invalid, 3');
    else await page.getByLabel('Leaf 1 value', { exact: true }).fill('0');
    await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
    await expect(page.getByRole('alert')).toBeVisible();
    expect((await documentOf(page)).revision).toBe(0);
    if (statistical) {
      await page.getByLabel('Raw samples 1', { exact: true }).fill('1, 2,');
      await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
      await expect(page.getByRole('alert')).toBeVisible();
      expect((await documentOf(page)).revision).toBe(0);
    }
    if (statistical) await page.getByLabel('Raw samples 1', { exact: true }).fill('9.5, -1.25, 2, 2, 5, 8');
    if (kind === 'histogram') {
      await page.getByLabel('Bin count', { exact: true }).fill('5');
      await page.getByLabel('Interval closed', { exact: true }).selectOption('left');
    } else if (kind === 'box_whisker') {
      await page.getByLabel('Group 1 label', { exact: true }).fill('Updated group');
      await page.getByLabel('Quartile method', { exact: true }).selectOption('exclusive');
      await page.getByLabel('mean line', { exact: true }).check();
      await page.getByLabel('outliers', { exact: true }).uncheck();
    } else {
      await page.getByLabel('Leaf 1 level 1', { exact: true }).fill('East');
      await page.getByLabel('Leaf 1 level 2', { exact: true }).fill('Updated leaf');
      await page.getByLabel('Leaf 1 value', { exact: true }).fill('14.75');
      if (kind === 'treemap') await page.getByLabel('Parent labels', { exact: true }).selectOption('banner');
    }
    await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
    await waitRevision(page, 1);
    await expect(page.getByRole('alert')).toHaveCount(0);
    const current = (await documentOf(page)).deck.slides[0].elements[0];
    if (kind === 'histogram') expect(current).toMatchObject({ categories: [], series: [{ values: [] }], options: { histogram: { samples: [9.5, -1.25, 2, 2, 5, 8], binning: { rule: 'count', count: 5 }, interval_closed: 'left' } } });
    else if (kind === 'box_whisker') expect(current).toMatchObject({ categories: ['Updated group', 'B'], series: [{ values: [] }], options: { box_whisker: { samples: [[9.5, -1.25, 2, 2, 5, 8], [2, 4, 8, 12, 40]], quartile_method: 'exclusive', mean_line: true, outliers: false } } });
    else expect(current).toMatchObject({ categories: ['Updated leaf', 'B'], series: [{ values: [14.75, 12.5] }], options: { hierarchy: { paths: [['East', 'Updated leaf'], ['South', 'B']] } } });
    expect(await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; return (await client.openPresentation('typed-reopen', (await initial.exportPresentation()).base64)).session.document.deck.slides[0].elements[0]; })()`)).toEqual(current);
    const preview = page.getByTestId('chart-preview').locator(`[data-chart-layout="${kind}"]`);
    await expect(preview).toBeVisible();
    await expect(preview).toHaveAttribute('data-preview-warnings', /Office/);
    if (kind === 'histogram') await expect(preview.locator('.recharts-bar-rectangle')).toHaveCount(4);
    if (kind === 'box_whisker') await expect(preview.locator('[data-box-group]')).toHaveCount(2);
    if (kind === 'treemap') await expect(preview.locator('.recharts-rectangle').first()).toBeVisible();
    if (kind === 'sunburst') await expect(preview.locator('.recharts-sector').first()).toBeVisible();
    for (const width of [1440, 390]) {
      await page.setViewportSize({ width, height: 960 });
      await page.evaluate(async () => { await document.fonts.ready; });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      const pixels = await sharp(await preview.screenshot({ path: testInfo.outputPath(`${kind}-preview-${width}.png`) })).removeAlpha().raw().toBuffer({ resolveWithObject: true });
      let colored = 0;
      for (let offset = 0; offset < pixels.data.length; offset += pixels.info.channels) {
        const channels = [...pixels.data.subarray(offset, offset + 3)];
        if (Math.max(...channels) - Math.min(...channels) > 25) colored++;
      }
      expect(colored).toBeGreaterThan(100);
      await page.screenshot({ path: testInfo.outputPath(`${kind}-${width}.png`), fullPage: true });
    }
    await page.getByRole('button', { name: 'Undo test edit', exact: true }).click();
    await waitRevision(page, 2);
    expect(await page.evaluate(`(async () => (await window.panelHarness.initial.exportPresentation()).base64 === window.panelHarness.source.base64)()`)).toBe(true);
    expect(errors).toEqual([]);
  });
}

test('stale chart drafts are never committed over a concurrent revision', async ({ page }) => {
  const { calls } = await mount(page, 'object');
  await page.getByLabel('Series 1 value 1', { exact: true }).fill('99');
  await page.getByRole('button', { name: 'Concurrent change', exact: true }).click();
  await waitRevision(page, 1);
  await expect(page.getByRole('alert')).toHaveText(/Document changed/);
  await expect(page.getByRole('button', { name: 'Apply chart', exact: true })).toBeDisabled();
  expect(calls.filter((request) => request.op === 'transaction')).toHaveLength(1);
  await expect(page.getByLabel('Series 1 value 1', { exact: true })).toHaveValue('99');
  await page.getByRole('button', { name: 'Reload object tools' }).click();
  await expect(page.getByLabel('Series 1 value 1', { exact: true })).toHaveValue('20');
});

test('table operations pass through the SDK and retain rich style fields', async ({ page }) => {
  const { calls } = await mount(page, 'object', table);
  await page.getByLabel('Row span', { exact: true }).fill('2');
  await page.getByLabel('Column span', { exact: true }).fill('2');
  await page.getByRole('button', { name: 'Merge selected cells' }).click();
  await waitRevision(page, 1);
  expect(calls.at(-1)).toMatchObject({ op: 'edit_table', expected_revision: 0, operations: [{ op: 'merge', region: { row: 0, column: 0, row_span: 2, col_span: 2 } }] });
  await page.getByRole('button', { name: 'Split selected cell' }).click();
  await waitRevision(page, 2);
  expect(calls.at(-1)).toMatchObject({ operations: [{ op: 'split', row: 0, column: 0 }] });
  await page.getByText('Cell style', { exact: true }).click();
  await page.getByLabel('Cell font size', { exact: true }).fill('28');
  await page.getByRole('button', { name: 'Apply cell style' }).click();
  await waitRevision(page, 3);
  expect(calls.at(-1)).toMatchObject({ operations: [{ op: 'set_cell_style', style: { fill: 'FFFFFF', text_style: { font_size: 28 }, text_format: { paragraphs: [{ runs: [{ text: 'A', style: { italic: true } }] }] } } }] });
});

test('rich-cell and vector apply buttons request typed operations with captured revision', async ({ page }) => {
  const { calls } = await mount(page, 'object', table);
  await page.getByText('Cell data', { exact: true }).click();
  await page.getByLabel('Cell 1, 1', { exact: true }).fill('Changed');
  await expect(page.getByLabel('Cell 1, 1', { exact: true })).toHaveAttribute('maxlength', '200');
  await page.getByRole('button', { name: 'Apply table data' }).click();
  await expect.poll(() => calls.filter((request) => request.op === 'edit_table').length).toBe(1);
  await waitRevision(page, 1);
  expect(calls.at(-1)).toMatchObject({ op: 'edit_table', expected_revision: 0, operations: [{ op: 'set_cell_text', row: 0, column: 0, text: 'Changed' }] });
  const vector = { ...bounds, type: 'polygon' as const, points: [[0, 0], [1, 0], [0.5, 1]] as [number, number][], fill: '087F73', stroke: '000000', stroke_width: 1, visual: { opacity: 0.5 } };
  const vectorCalls = (await mount(page, 'object', vector)).calls;
  await page.getByText('Path vertices', { exact: true }).click();
  await page.getByLabel('Vertex 2 command').selectOption('cubic');
  await page.getByLabel('Vertex 2 control1 x').fill('0.3');
  await page.getByRole('button', { name: 'Apply path vertices' }).evaluate((button) => { (button as HTMLButtonElement).click(); (button as HTMLButtonElement).click(); });
  await expect.poll(() => vectorCalls.filter((request) => request.op === 'edit_vector').length).toBe(1);
  await waitRevision(page, 1);
  expect(vectorCalls.map((request) => request.op)).toEqual(['edit_vector', 'transaction']);
  expect(vectorCalls[0].element).toEqual(vector);
  expect(vectorCalls[1].transaction).toMatchObject({ expected_revision: 0 });
  expect((await documentOf(page)).deck.slides[0].elements[0]).toMatchObject({ visual: { opacity: 0.5 }, points: [[0, 0], [0.3, 0], [1, 0], [1, 0], [0.5, 1]] });
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('G12 local AI actual model review native apply and Undo', async ({ page }, testInfo) => {
  test.skip(process.env.AISLIDE_REAL_AI_TEST !== '1', 'requires explicitly installed pinned model');
  const pixels = Buffer.alloc(320 * 320 * 4);
  for (let row = 0; row < 320; row++) for (let column = 0; column < 320; column++) {
    const offset = (row * 320 + column) * 4;
    const vase = row >= 65 && row < 265 && Math.abs(column - 160) < 44 + Math.floor((row - 65) / 5);
    const background = 210 + (column + row) % 29;
    pixels.set(vase ? [180 + column % 35,45 + row % 15,35,255] : [background,background,background - 10,255],offset);
  }
  const base64 = (await sharp(pixels,{ raw:{ width:320,height:320,channels:4 } }).png().toBuffer()).toString('base64');
  const picture: Element = { ...bounds, type:'picture',base64,mime_type:'image/png',alt:'Original synthetic vase',crop:{left:0,top:0,right:0,bottom:0} };
  const { errors } = await mount(page,'object',picture,true,true);
  test.setTimeout(120000);
  const original = await page.evaluate('window.panelHarness.initial.document.hash');
  await page.getByRole('button',{name:'Generate cutout',exact:true}).click();
  const review = page.getByRole('region',{name:'AI image review'});
  await expect(review).toBeVisible({timeout:90000});
  expect(await page.evaluate('window.panelHarness.initial.document.hash')).toBe(original);
  const url = await page.getByAltText('AI cutout candidate',{exact:true}).getAttribute('src');
  const output = await sharp(Buffer.from(url!.split(',')[1],'base64')).ensureAlpha().raw().toBuffer();
  expect(output[(160 * 320 + 160) * 4 + 3]).toBeGreaterThan(200);
  expect(output[(10 * 320 + 10) * 4 + 3]).toBeLessThan(64);
  for (const width of [1440,390]) {
    await page.setViewportSize({width,height:960});
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await review.screenshot({path:testInfo.outputPath(`local-ai-cutout-${width}.png`)});
  }
  await page.getByRole('button',{name:'Apply AI cutout',exact:true}).click();
  await expect.poll(async () => page.evaluate('window.panelHarness.initial.revision')).toBe(1);
  const current = await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; return (await client.openPresentation('cutout-reopen',(await initial.exportPresentation()).base64)).session.document.deck.slides[0].elements[0]; })()`) as Element;
  expect(current.type).toBe('picture');
  if (current.type === 'picture') { expect(current.alt).toBe(picture.alt); expect(current.base64).toBe(url!.split(',')[1]); }
  await page.getByRole('button',{name:'Undo test edit',exact:true}).click();
  await expect.poll(async () => page.evaluate('window.panelHarness.initial.document.hash')).toBe(original);
  expect(await page.evaluate(`(async () => (await window.panelHarness.initial.exportPresentation()).base64 === window.panelHarness.source.base64)()`)).toBe(true);
  expect(errors).toEqual([]);
});

test('picture edit is prepared once, busy guarded and applied with captured revision', async ({ page }) => {
  const keyWarnings: string[] = [];
  page.on('console', (message) => { if (/two children with the same key/.test(message.text())) keyWarnings.push(message.text()); });
  const png = (await sharp({ create: { width: 2, height: 2, channels: 4, background: '#ffffff' } }).png().toBuffer()).toString('base64');
  const picture: Element = { ...bounds, type: 'picture', base64: png, mime_type: 'image/png', alt: 'Synthetic fixture', crop: { left: 0.1, top: 0, right: 0, bottom: 0 } };
  const { calls } = await mount(page, 'object', picture);
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  await page.route('**/api/core', async (route) => { if (route.request().postDataJSON().op !== 'edit_image') return route.fallback(); await gate; return route.fallback(); });
  await page.getByLabel('brightness', { exact: true }).fill('0.25');
  await page.getByLabel('Remove selected color', { exact: true }).check();
  await page.getByLabel('Color tolerance', { exact: true }).fill('10');
  await page.getByRole('button', { name: 'Apply picture edit' }).evaluate((button) => { (button as HTMLButtonElement).click(); (button as HTMLButtonElement).click(); });
  await expect.poll(() => calls.filter((request) => request.op === 'edit_image').length).toBe(1);
  await expect(page.getByRole('button', { name: 'Apply picture edit', exact: true })).toHaveCount(1);
  await expect(page.getByRole('button', { name: 'Apply picture edit' })).toBeDisabled();
  release();
  await waitRevision(page, 1);
  await expect(page.getByRole('button', { name: 'Apply picture edit', exact: true })).toHaveCount(1);
  expect(calls.map((request) => request.op)).toEqual(['edit_image', 'apply_image_edit']);
  expect(calls[0].params).toMatchObject({ brightness: 0.25, background_key: { color: [255, 255, 255], tolerance: 10 } });
  expect(calls[1]).toMatchObject({ id: 'object', expected_revision: 0 });
  expect(await page.evaluate('window.panelHarness.busy')).toEqual([true, false]);
  expect(keyWarnings).toEqual([]);
});

test('picture preparation cannot overwrite changes made while it is pending', async ({ page }) => {
  const png = (await sharp({ create: { width: 2, height: 2, channels: 4, background: '#ffffff' } }).png().toBuffer()).toString('base64');
  const { calls } = await mount(page, 'object', { ...bounds, type: 'picture', base64: png, mime_type: 'image/png', alt: '', crop: { left: 0, top: 0, right: 0, bottom: 0 } });
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  await page.route('**/api/core', async (route) => { if (route.request().postDataJSON().op !== 'edit_image') return route.fallback(); await gate; return route.fallback(); });
  await page.getByRole('button', { name: 'Apply picture edit' }).click();
  await expect.poll(() => calls.filter((request) => request.op === 'edit_image').length).toBe(1);
  await page.getByRole('button', { name: 'Concurrent change' }).click();
  await waitRevision(page, 1);
  release();
  await expect(page.getByRole('alert').last()).toContainText(/Revision conflict/);
  expect(calls.filter((request) => request.op === 'apply_image_edit')).toHaveLength(0);
});

test('visual form uses legacy shape rotation and synchronizes gradient fill', async ({ page }) => {
  const { calls } = await mount(page, 'object', { ...bounds, type: 'shape', preset: 'roundRect', rotation: 15, text: '', font_size: 20, color: '000000', bold: false, fill: '087F73', stroke: '000000', stroke_width: 1, visual: { locked: true } });
  await page.getByLabel('Rotation', { exact: true }).fill('45');
  await page.getByLabel('Flip horizontally', { exact: true }).check();
  await page.getByText('Gradient', { exact: true }).click();
  await page.getByLabel('Gradient kind').selectOption('radial');
  await page.getByLabel('Stop 1 color').fill('#cc2244');
  await page.getByLabel('Stop 2 opacity').fill('0.4');
  await expect(page.getByRole('button', { name: 'Delete gradient stop 1' })).toBeDisabled();
  await page.getByRole('button', { name: 'Apply visual style' }).click();
  await waitRevision(page, 1);
  const result = (calls[0].transaction as { operations: { value: Element }[] }).operations[0].value;
  expect(result).toMatchObject({ rotation: 45, fill: 'CC2244', visual: { locked: true, flip_h: true, gradient: { kind: 'radial', center: [0.5, 0.5], stops: [{ offset: 0, color: 'CC2244', opacity: 1 }, { offset: 1, color: 'FFFFFF', opacity: 0.4 }] } } });
  expect('visual' in result && result.visual?.rotation).toBeUndefined();
});

test('review notes and fields preserve paragraphs and use UUID plus the actual field kind', async ({ page }) => {
  const { calls } = await mount(page, 'review', text);
  await page.getByRole('textbox', { name: 'Speaker notes', exact: true }).fill('Revised notes');
  await page.getByRole('button', { name: 'Apply speaker notes' }).click();
  await waitRevision(page, 1);
  await page.getByLabel('Field type').selectOption('datetime1');
  await page.getByLabel('Reference date').fill('2026-09-17');
  await page.getByRole('button', { name: 'Add field to selected text' }).click();
  await waitRevision(page, 2);
  const request = calls.find((request) => request.op === 'update_paragraphs')!;
  expect(request).toMatchObject({ id: 'object', expected_revision: 1, paragraphs: [{ alignment: 'right', runs: [{ text: 'First', style: { italic: true } }] }, { runs: [{ text: 'Second', style: { bold: true } }, { text: '9/17/2026', field: { kind: 'datetime1', id: expect.stringMatching(/^[a-f\d-]{36}$/) } }] }] });
  await page.getByRole('button', { name: 'Refresh document fields' }).click();
  await waitRevision(page, 3);
  expect(calls.at(-1)).toMatchObject({ op: 'refresh_fields', reference_date: '2026-09-17', expected_revision: 2 });
});

test('comments require explicit author and support local replies, resolve and delete', async ({ page }) => {
  const { calls } = await mount(page, 'review', text);
  await page.getByLabel('Review section').selectOption('comments');
  await page.getByLabel('Comment text', { exact: true }).fill('Local @reviewer');
  await expect(page.getByRole('button', { name: 'Add comment', exact: true })).toBeDisabled();
  await page.getByLabel('Comment author', { exact: true }).fill('Test author');
  await page.getByLabel('Author initials').fill('TA');
  await page.getByRole('button', { name: 'Add comment', exact: true }).click();
  await waitRevision(page, 1);
  expect(calls[0]).toMatchObject({ op: 'add_comment', comment: { author: 'Test author', initials: 'TA', text: 'Local @reviewer', timestamp: expect.stringMatching(/^\d{4}-\d\d-\d\dT.*Z$/) } });
  await page.getByRole('button', { name: 'Reply to comment by Test author' }).click();
  await page.getByLabel('Reply text').fill('Reply');
  await page.getByRole('button', { name: 'Add reply', exact: true }).click();
  await waitRevision(page, 2);
  expect(calls[1]).toMatchObject({ op: 'reply_comment', parent_id: (calls[0].comment as { id: string }).id });
  await page.getByRole('button', { name: 'Resolve comment by Test author', exact: true }).first().click();
  await waitRevision(page, 3);
  await page.getByRole('button', { name: 'Delete comment by Test author and its replies' }).first().click();
  await waitRevision(page, 4);
  expect((await documentOf(page)).deck.slides[0].review?.comments).toEqual([]);
});

test('reading order is acknowledgement gated and checker issues can navigate', async ({ page }) => {
  const { calls } = await mount(page, 'review', text);
  await page.getByLabel('Review section').selectOption('accessibility');
  await page.getByLabel('Accessible description').fill('Description');
  await page.getByRole('button', { name: 'Apply accessibility', exact: true }).click();
  await waitRevision(page, 1);
  expect(calls.at(-1)).toMatchObject({ op: 'set_accessibility', element_id: 'object', metadata: { description: 'Description' } });
  await page.getByRole('button', { name: 'Move object later' }).click();
  await expect(page.getByRole('button', { name: 'Apply reading order' })).toBeDisabled();
  await page.getByLabel('I acknowledge this also changes object z-order').check();
  await page.getByRole('button', { name: 'Apply reading order' }).click();
  await waitRevision(page, 2);
  expect(calls.at(-1)).toMatchObject({ op: 'set_reading_order', order: ['other', 'object'], expected_revision: 1 });
  await page.getByRole('button', { name: 'Check accessibility', exact: true }).click();
  await page.getByRole('button', { name: 'Go to accessibility issue 1' }).click();
  expect(await page.evaluate('window.panelHarness.navigation')).toEqual(['slide', 'object']);
});

test('inspection exports only a confirmed new copy, never replaces the active session', async ({ page }) => {
  const { calls } = await mount(page, 'review', text);
  await page.getByLabel('Review section').selectOption('inspection');
  await expect(page.getByRole('button', { name: 'Download clean PPTX copy' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Inspect document' }).click();
  await page.getByLabel('notes (1)', { exact: true }).check();
  await expect(page.getByRole('button', { name: 'Download clean PPTX copy' })).toBeDisabled();
  await page.getByLabel('Remove selected categories from a new copy').check();
  await page.evaluate('window.panelHarness.cancelExport = true');
  await page.getByRole('button', { name: 'Download clean PPTX copy' }).click();
  await expect(page.getByRole('status')).toHaveText('Download cancelled.');
  expect(calls.at(-1)).toMatchObject({ op: 'export_clean_copy', options: { confirmed: true, categories: ['notes'], new_document_id: expect.stringMatching(/^clean-/) } });
  expect((await documentOf(page)).revision).toBe(0);
  expect(await page.evaluate('window.panelHarness.changes.length')).toBe(0);
  expect(await page.evaluate('window.panelHarness.exports[0]')).toMatchObject({ bytes: [80, 75], filename: expect.stringMatching(/^clean-copy-.*\.pptx$/) });
});

test('document setup resizes, exports through callback and guards template selection', async ({ page }) => {
  const { calls } = await mount(page, 'setup');
  await page.getByRole('combobox', { name: 'Preset', exact: true }).selectOption('portrait');
  await page.getByRole('combobox', { name: 'Content', exact: true }).selectOption('keep');
  await page.getByRole('button', { name: 'Apply page size' }).click();
  await waitRevision(page, 1);
  expect(calls.at(-1)).toMatchObject({ op: 'resize_canvas', width: 720, height: 1280, mode: 'keep', expected_revision: 0 });
  await expect(page.getByLabel('Page width')).toHaveAttribute('min', '320');
  await expect(page.getByLabel('Page width')).toHaveAttribute('max', '4096');
  await expect(page.getByLabel('Open POTX or THMX')).toBeDisabled();
  await expect(page.getByLabel('Remember imported template on this device')).not.toBeChecked();
  await page.getByLabel('Export format').selectOption('thmx');
  await page.getByRole('button', { name: 'Download template' }).click();
  await expect(page.getByRole('status')).toHaveText('Template exported.');
  expect(await page.evaluate('window.panelHarness.exports[0]')).toMatchObject({ filename: 'template.thmx', mime: 'application/vnd.openxmlformats-officedocument.theme', bytes: [80, 75] });
  await page.getByLabel('Replace active document').check();
  await page.evaluate('window.panelHarness.allowOpen = false');
  await page.getByLabel('Open POTX or THMX').setInputFiles({ name: 'brand.thmx', mimeType: 'application/octet-stream', buffer: Buffer.from('PK') });
  await expect.poll(async () => page.evaluate('window.panelHarness.beforeOpen')).toBe(true);
  expect(calls.filter((request) => request.op === 'import_template')).toHaveLength(0);
});

test('standalone panels fit narrow viewports and expose accessible controls', async ({ page }, testInfo) => {
  for (const mode of ['object', 'review', 'setup'] as const) {
    const { errors } = await mount(page, mode, text);
    for (const width of [1440, 320]) {
      await page.setViewportSize({ width, height: 960 });
      await page.evaluate(async () => { await document.fonts.ready; });
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
      await page.screenshot({ path: testInfo.outputPath(`${mode}-${width}.png`) });
    }
    const result = await new AxeBuilder({ page }).include('.object-tools-panel').withTags(['wcag2a', 'wcag2aa']).analyze();
    expect(result.violations).toEqual([]);
    expect(errors).toEqual([]);
  }
});

test('actual core table data preserves rich formatting through native reopen and Undo with empty-update noop', async ({ page }) => {
  const { calls, errors } = await mount(page, 'object', richTable, true, true);
  const before = await documentOf(page);
  await page.getByText('Cell data', { exact: true }).click();
  await page.getByRole('button', { name: 'Apply table data' }).click();
  await expect(page.getByRole('region', { name: 'Object tools', exact: true })).toHaveAttribute('aria-busy', 'false');
  expect(calls.filter((request) => request.op === 'edit_table')).toHaveLength(0);
  expect(await documentOf(page)).toEqual(before);
  await page.getByLabel('Cell 1, 1', { exact: true }).fill('ChangedB');
  await page.getByLabel('Cell 2, 2', { exact: true }).fill('Second');
  await page.getByRole('button', { name: 'Apply table data' }).click();
  await waitRevision(page, 1);
  const current = (await documentOf(page)).deck.slides[0].elements[0];
  expect(current).toMatchObject({ rows: [['ChangedB', ''], ['', 'Second']], format: { cells: [{ row: 0, column: 0, style: { text_format: { paragraphs: [{ runs: [{ text: 'Changed' }, { text: 'B' }] }] } } }] } });
  const original = before.deck.slides[0].elements[0];
  expect(current.type === 'table' && current.format?.cells?.[0].style.text_format?.paragraphs?.[0].runs.map((run) => run.style)).toEqual(original.type === 'table' && original.format?.cells?.[0].style.text_format?.paragraphs?.[0].runs.map((run) => run.style));
  expect(current.type === 'table' && current.format?.cells?.[0].style.text_style).toEqual(original.type === 'table' && original.format?.cells?.[0].style.text_style);
  expect(calls.filter((request) => request.op === 'edit_table')).toHaveLength(1);
  expect(calls.find((request) => request.op === 'edit_table')).toMatchObject({ expected_revision: 0, operations: [{ op: 'set_cell_text', row: 0, column: 0, text: 'ChangedB' }, { op: 'set_cell_text', row: 1, column: 1, text: 'Second' }] });
  expect(await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; const exported = await initial.exportPresentation(); return (await client.openPresentation('reopened', exported.base64)).session.document.deck.slides[0].elements[0]; })()`)).toEqual(current);
  await page.getByRole('button', { name: 'Undo test edit', exact: true }).click();
  await expect.poll(async () => (await documentOf(page)).hash).toBe(before.hash);
  expect(await page.evaluate(`(async () => (await window.panelHarness.initial.exportPresentation()).base64 === window.panelHarness.source.base64)()`)).toBe(true);
  expect(errors).toEqual([]);
});

test('actual core table data rejects a nonempty merge follower atomically and retains draft', async ({ page }) => {
  const merged: Element = { ...table, format: { ...table.format, merges: [{ row: 0, column: 0, row_span: 1, col_span: 2 }] } };
  const { calls } = await mount(page, 'object', merged, true);
  const before = await documentOf(page);
  await page.getByText('Cell data', { exact: true }).click();
  await page.getByLabel('Cell 1, 1', { exact: true }).fill('Changed');
  await page.getByLabel('Cell 1, 2', { exact: true }).fill('Rejected');
  await page.getByRole('button', { name: 'Apply table data' }).click();
  await expect(page.getByRole('alert')).toContainText('subordinate cells must be empty');
  expect(await documentOf(page)).toEqual(before);
  await expect(page.getByLabel('Cell 1, 1', { exact: true })).toHaveValue('Changed');
  expect(calls.filter((request) => request.op === 'edit_table')).toHaveLength(1);
});

test('inline table axis removal drops pending text and Cancel makes no core calls', async ({ page }) => {
  const { calls } = await mount(page, 'inline', table);
  await page.getByLabel('Table row 2 column 1').fill('Deleted row');
  await page.getByRole('button', { name: 'Remove last table row', exact: true }).click();
  await page.getByRole('button', { name: 'Add table row', exact: true }).click();
  await expect(page.getByLabel('Table row 2 column 1')).toHaveValue('');
  await page.getByLabel('Table row 1 column 2').fill('Deleted column');
  await page.getByRole('button', { name: 'Remove last table column', exact: true }).click();
  await page.getByRole('button', { name: 'Add table column', exact: true }).click();
  await expect(page.getByLabel('Table row 1 column 2')).toHaveValue('');
  await page.getByRole('button', { name: 'Cancel on-slide edit', exact: true }).click();
  expect(calls).toHaveLength(0);
  expect((await documentOf(page)).revision).toBe(0);
});

test('actual core inline table prepares rich cells only on Apply and preserves native formatting', async ({ page }) => {
  const { calls, errors } = await mount(page, 'inline', richTable, true, true);
  const before = (await documentOf(page)).deck.slides[0].elements[0];
  await page.getByLabel('Table row 1 column 1').fill('Typed valueB');
  await page.getByLabel('Table row 2 column 2').fill('Other');
  expect(calls.filter((request) => request.op === 'set_table_cell_text')).toHaveLength(0);
  expect((await documentOf(page)).revision).toBe(0);
  await page.getByRole('button', { name: 'Apply on-slide edit' }).click();
  await waitRevision(page, 1);
  expect(calls.filter((request) => request.op === 'set_table_cell_text')).toHaveLength(2);
  expect(calls.filter((request) => request.op === 'transaction')).toHaveLength(1);
  const current = (await documentOf(page)).deck.slides[0].elements[0];
  expect(current).toMatchObject({ rows: [['Typed valueB', ''], ['', 'Other']], format: { cells: [{ style: { text_format: { paragraphs: [{ runs: [{ text: 'Typed value' }, { text: 'B' }] }] } } }] } });
  expect(current.type === 'table' && current.format?.cells?.[0].style.text_format?.paragraphs?.[0].runs.map((run) => run.style)).toEqual(before.type === 'table' && before.format?.cells?.[0].style.text_format?.paragraphs?.[0].runs.map((run) => run.style));
  expect(current.type === 'table' && current.format?.cells?.[0].style.text_style).toEqual(before.type === 'table' && before.format?.cells?.[0].style.text_style);
  expect(await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; return (await client.openPresentation('inline-reopen', (await initial.exportPresentation()).base64)).session.document.deck.slides[0].elements[0]; })()`)).toEqual(current);
  expect(errors).toEqual([]);
});

test('actual core path edits synchronize curves and straight points through native reopen and Undo', async ({ page }) => {
  const polygon: Element = { ...bounds, type: 'polygon', points: [[0, 0], [1, 0], [0.5, 1]], fill: '@accent2', stroke: '@dk1', stroke_width: 1, visual: { opacity: 0.5, flip_h: true } };
  const { calls, errors } = await mount(page, 'object', polygon, true, true);
  const before = await documentOf(page);
  await page.getByText('Path vertices', { exact: true }).click();
  await page.getByLabel('Vertex 2 command').selectOption('cubic');
  await page.getByLabel('Vertex 2 control1 x').fill('1.2');
  await page.getByRole('button', { name: 'Apply path vertices' }).click();
  await expect(page.getByRole('alert')).toContainText('normalized');
  expect(await documentOf(page)).toEqual(before);
  expect(calls.filter((request) => request.op === 'transaction')).toHaveLength(0);
  await page.getByLabel('Vertex 2 control1 x').fill('0.3');
  const appliedCurve = waitForCoreOperation(page, 'transaction');
  await page.getByRole('button', { name: 'Apply path vertices' }).click();
  await appliedCurve;
  await waitRevision(page, 1);
  const curved = (await documentOf(page)).deck.slides[0].elements[0];
  expect(curved).toMatchObject({ points: [[0, 0], [0.3, 0], [1, 0], [1, 0], [0.5, 1]], visual: { opacity: 0.5, flip_h: true, path: { commands: [{ op: 'move' }, { op: 'cubic' }, { op: 'line' }, { op: 'close' }] } } });
  expect(await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; return (await client.openPresentation('path-reopen', (await initial.exportPresentation()).base64)).session.document.deck.slides[0].elements[0]; })()`)).toEqual(curved);
  await page.getByText('Path vertices', { exact: true }).click();
  await page.getByLabel('Vertex 2 command').selectOption('line');
  const appliedLine = waitForCoreOperation(page, 'transaction');
  await page.getByRole('button', { name: 'Apply path vertices' }).click();
  await appliedLine;
  await waitRevision(page, 2);
  expect((await documentOf(page)).deck.slides[0].elements[0]).toEqual(before.deck.slides[0].elements[0]);
  const undoneLine = waitForCoreOperation(page, 'undo_transaction');
  await page.getByRole('button', { name: 'Undo test edit', exact: true }).click();
  await undoneLine;
  await waitRevision(page, 3);
  expect((await documentOf(page)).deck.slides[0].elements[0]).toEqual(curved);
  const undoneCurve = waitForCoreOperation(page, 'undo_transaction');
  await page.getByRole('button', { name: 'Undo test edit', exact: true }).click();
  await undoneCurve;
  await waitRevision(page, 4);
  expect(await page.evaluate(`(async () => (await window.panelHarness.initial.exportPresentation()).base64 === window.panelHarness.source.base64)()`)).toBe(true);
  expect(errors).toEqual([]);
});

test('actual core table merge and split round-trip through the standalone panel', async ({ page }) => {
  await mount(page, 'object', table, true);
  await page.getByLabel('Row span', { exact: true }).fill('2');
  await page.getByLabel('Column span', { exact: true }).fill('2');
  await page.getByRole('button', { name: 'Merge selected cells' }).click();
  await waitRevision(page, 1);
  expect((await documentOf(page)).deck.slides[0].elements[0]).toMatchObject({ format: { merges: [{ row: 0, column: 0, row_span: 2, col_span: 2 }] } });
  await page.getByRole('button', { name: 'Split selected cell' }).click();
  await waitRevision(page, 2);
  expect((await documentOf(page)).deck.slides[0].elements[0]).toMatchObject({ format: { cells: [{ row: 0, column: 0, style: { text_format: { paragraphs: [{ runs: [{ text: 'A', style: { italic: true } }] }] } } }] } });
});

test('chart grid rejects blank numeric values before serialization', async ({ page }) => {
  const { calls } = await mount(page, 'object');
  await page.getByLabel('Series 1 value 1', { exact: true }).fill('');
  await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('finite number');
  expect(calls).toHaveLength(0);
  expect((await documentOf(page)).revision).toBe(0);
});

test('template import uses a fresh session, retains filename, and stores only by opt-in', async ({ page }) => {
  const { calls } = await mount(page, 'setup');
  const source = await documentOf(page);
  await page.route('**/api/core', (route) => {
    const request = route.request().postDataJSON();
    if (request.op !== 'import_template') return route.fallback();
    return route.fulfill({ json: { ...source, id: request.id, hash: 'new-template' } });
  });
  await page.getByLabel('Replace active document').check();
  await page.getByLabel('Open POTX or THMX').setInputFiles({ name: 'Brand.thmx', mimeType: 'application/octet-stream', buffer: Buffer.from('PK') });
  await expect(page.getByRole('status', { name: 'Imported template', exact: true })).toHaveText('Brand.thmx');
  expect(calls.at(-1)).toMatchObject({ op: 'import_template', id: expect.stringMatching(/^template-/), kind: 'thmx', base64: 'UEs=' });
  expect(await page.evaluate('window.panelHarness.replacements.length')).toBe(1);
  expect((await documentOf(page)).id).not.toBe(source.id);
  await page.getByRole('button', { name: 'Load local template favorites' }).click();
  await expect(page.getByRole('button', { name: 'Open Brand.thmx', exact: true })).toHaveCount(0);
  await page.getByLabel('Replace active document').check();
  await page.getByLabel('Remember imported template on this device').check();
  await page.getByLabel('Open POTX or THMX').setInputFiles({ name: 'Saved.thmx', mimeType: 'application/octet-stream', buffer: Buffer.from('PK') });
  await expect(page.getByRole('status', { name: 'Imported template', exact: true })).toHaveText('Saved.thmx');
  await page.getByRole('button', { name: 'Load local template favorites' }).click();
  await expect(page.getByRole('button', { name: 'Open Saved.thmx', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Forget Saved.thmx', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open Saved.thmx', exact: true })).toHaveCount(0);
});

test('replacement sessions with matching IDs and revision cannot inherit stale chart drafts', async ({ page }) => {
  await mount(page, 'object');
  await page.getByLabel('Series 1 value 1', { exact: true }).fill('99');
  await page.getByRole('button', { name: 'Replace same identity' }).click();
  await expect(page.getByLabel('Series 1 value 1', { exact: true })).toHaveValue('80');
  await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
  await waitRevision(page, 1);
  expect((await documentOf(page)).deck.slides[0].elements[0]).toMatchObject({ series: [{ values: [80, 30] }, { values: [2, 3] }] });
});

test('phase3 real statistical settings use core projection and preserve native values through Undo', async ({ page }, testInfo) => {
  const element: Element = { ...bounds, type: 'chart', kind: 'line', categories: ['1','2','3'], series: [{ name: 'Measured', values: [3,5,7], color: '087F73', trendline: { kind: 'linear', forward: 1 }, error_bars: { kind: 'fixed_value', value: 1 } }], options: { legend: 'hidden' } };
  const { errors, calls } = await mount(page, 'object', element, true, true);
  const preview = page.getByTestId('chart-preview').locator('[data-core-preview]');
  await expect(preview).toHaveAttribute('data-core-preview', 'ready');
  const original = await preview.locator('img').getAttribute('src');
  await page.locator('summary').filter({ hasText: /^Series$/ }).click();
  await page.getByRole('combobox', { name: 'Trendline', exact: true }).selectOption('polynomial');
  await page.getByRole('spinbutton', { name: 'Trendline order', exact: true }).fill('2');
  await page.getByRole('spinbutton', { name: 'Trendline forward', exact: true }).fill('2');
  await page.getByRole('combobox', { name: 'Error bars', exact: true }).selectOption('standard_deviation');
  await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
  await waitRevision(page, 1);
  await expect(preview).toHaveAttribute('data-core-preview', 'ready');
  await expect(preview.locator('img')).not.toHaveAttribute('src', original!);
  const current = (await documentOf(page)).deck.slides[0].elements[0];
  expect(current).toMatchObject({ series: [{ values: [3,5,7], trendline: { kind: 'polynomial', order: 2, forward: 2 }, error_bars: { kind: 'standard_deviation', value: 1 } }] });
  expect(await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; const copy = await initial.copyFormat('slide', { id: 'object' }); const bytes = await initial.exportPresentation(); const reopened = (await client.openPresentation('phase3-ui-reopen', bytes.base64)).session; return { copy: Boolean(copy), element: reopened.document.deck.slides[0].elements[0] }; })()`)).toEqual({ copy: true, element: current });
  for (const width of [1440,390]) {
    await page.setViewportSize({ width, height: 960 });
    await preview.locator('img').evaluate(async (image: HTMLImageElement) => image.decode());
    const { data, info } = await sharp(await preview.screenshot({ path: testInfo.outputPath(`phase3-chart-${width}.png`) })).removeAlpha().raw().toBuffer({ resolveWithObject: true });
    let colored = 0;
    for (let offset = 0; offset < data.length; offset += info.channels) if (data[offset + 1] > data[offset] + 30) colored++;
    expect(colored).toBeGreaterThan(100);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.getByRole('button', { name: 'Undo test edit', exact: true }).click();
  await waitRevision(page, 2);
  await expect(preview).toHaveAttribute('data-core-preview', 'ready');
  await expect(preview.locator('img')).toHaveAttribute('src', original!);
  expect(await page.evaluate(`(async () => (await window.panelHarness.initial.exportPresentation()).base64 === window.panelHarness.source.base64)()`)).toBe(true);
  expect(calls.some(call => call.op === 'render_element_preview')).toBe(true);
  await expect(page.getByRole('alert')).toHaveText('Document changed. Reload before applying.');
  await page.getByRole('button', { name: 'Reload object tools' }).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('phase3 WordArt uses shaped core outlines and discards obsolete asynchronous previews', async ({ page }, testInfo) => {
  let release: (() => void) | undefined;
  let observed: (() => void) | undefined;
  const started = new Promise<void>(resolve => { observed = resolve; });
  const gate = new Promise<void>(resolve => { release = resolve; });
  let delayed = false;
  await page.route('**/api/core', async route => {
    const request = route.request().postDataJSON();
    if (request.op !== 'render_element_preview' || delayed) return route.continue();
    delayed = true;
    const response = await route.fetch();
    observed!();
    await gate;
    await route.fulfill({ response });
  });
  const element: Element = { ...bounds, type: 'text', width: 320, height: 180, text: 'office affinity 日本語', font_size: 24, color: '087F73', bold: true, visual: { text_warp: 'wave1' } };
  const { calls, errors } = await mount(page, 'object', element, true, true);
  await started;
  const changed = { ...element, text: 'Updated affinity 日本語', visual: { text_warp: 'arch_down' } };
  await page.evaluate(`(async () => { const harness = window.panelHarness; const current = harness.initial.document.deck.slides[0].elements[0]; const next = await harness.client.replaceElementText({ element: current, text: ${JSON.stringify(changed.text)} }); next.visual = { ...next.visual, text_warp: 'arch_down' }; await harness.replacePreviewElement(next); })()`);
  await waitRevision(page, 1);
  release!();
  const preview = page.getByTestId('wordart-preview').locator('[data-core-preview]');
  await expect(preview).toHaveAttribute('data-core-preview', 'ready');
  const image = preview.locator('img');
  await expect(image).toHaveAttribute('alt', changed.text);
  const svg = decodeURIComponent((await image.getAttribute('src'))!.split(',').slice(1).join(','));
  expect(svg).toContain(changed.text);
  expect(svg).not.toContain('office affinity');
  expect(svg).toContain('<path');
  await expect(preview.locator('span[style*="transform"]')).toHaveCount(0);
  for (const width of [1440,390]) {
    await page.setViewportSize({ width, height: 960 });
    await image.evaluate(async (image: HTMLImageElement) => image.decode());
    const { data, info } = await sharp(await preview.screenshot({ path: testInfo.outputPath(`phase3-wordart-${width}.png`) })).removeAlpha().raw().toBuffer({ resolveWithObject: true });
    let colored = 0;
    for (let offset = 0; offset < data.length; offset += info.channels) if (data[offset + 1] > data[offset] + 30) colored++;
    expect(colored).toBeGreaterThan(100);
  }
  expect((await documentOf(page)).revision).toBe(1);
  expect(calls.filter(call => call.op === 'transaction')).toHaveLength(1);
  expect(await page.evaluate(`(async () => { const { initial, client } = window.panelHarness; return (await client.openPresentation('wordart-ui-reopen', (await initial.exportPresentation()).base64)).session.document.deck.slides[0].elements[0].visual.text_warp; })()`)).toBe('arch_down');
  await page.getByRole('button', { name: 'Undo test edit', exact: true }).click();
  await waitRevision(page, 2);
  await expect(preview).toHaveAttribute('data-core-preview', 'ready');
  await expect(image).toHaveAttribute('alt', element.text);
  expect(await page.evaluate(`(async () => (await window.panelHarness.initial.exportPresentation()).base64 === window.panelHarness.source.base64)()`)).toBe(true);
  expect(errors).toEqual([]);
});
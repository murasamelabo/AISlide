import { test, expect, type Page } from '@playwright/test';
import type { Element, ChartKind } from '../../packages/client/types';
import sharp from 'sharp';

const bounds = { id: 'content', x: 80, y: 80, width: 800, height: 480 };
const text: Extract<Element, { type: 'text' }> = { ...bounds, type: 'text', text: 'Heading\nSecond', font_size: 24, color: '202525', bold: false };

async function render(page: Page, elements: Element[]) {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op !== 'create_presentation') return route.continue();
    await route.fulfill({ json: { version: 1, id: 'render-fixture', revision: 0, hash: 'synthetic', sources: [], bindings: [], parts: [], deck: { version: 1, title: 'Synthetic renderer fixture', width: 1280, height: 720, slides: [{ id: 'slide-1', title: 'Renderer', notes: '', background: 'FFFFFF', elements }] } } });
  });
  await page.goto('/');
  await expect(page.locator('.slide-stage [data-element-id]').first()).toBeVisible();
  await page.evaluate(async () => { await document.fonts.ready; });
  return errors;
}

test('rich paragraphs retain runs, units, numbering, language and explicit false styles', async ({ page }) => {
  await render(page, [{ ...text, bold: true, format: { paragraphs: [
    { alignment: 'right', margin_left: 190500, indent: -95250, line_spacing: { kind: 'points', value: 2250 }, space_before: { kind: 'percent', value: 50000 }, runs: [{ text: 'Heading', style: { bold: false, italic: true, underline: true, highlight: 'FFFF00', baseline: 30000, language: 'ja-JP', font_family: 'Noto Sans JP', font_size: 30, color: 'CC0000' } }] },
    { bullet: 'numbered', numbering: 'romanUcPeriod', number_start: 4, level: 1, runs: [{ text: 'Second' }] },
  ] } }]);
  const paragraph = page.locator('.slide-stage .document-paragraph').first();
  await expect(paragraph).toHaveCSS('text-align', 'right');
  await expect(paragraph).toHaveCSS('margin-left', '20px');
  await expect(paragraph).toHaveCSS('text-indent', '-10px');
  await expect(paragraph).toHaveCSS('line-height', '30px');
  const run = paragraph.locator('[lang="ja-JP"]');
  await expect(run).toHaveCSS('font-weight', '400');
  await expect(run).toHaveCSS('background-color', 'rgb(255, 255, 0)');
  await expect(run).toHaveCSS('vertical-align', '9px');
  await expect(page.locator('.slide-stage .document-list-marker')).toHaveText('IV.');
});

test('table dimensions, merged followers and rich cell styles render as a native table', async ({ page }) => {
  await render(page, [{ ...bounds, type: 'table', rows: [['Merged', '', 'C'], ['', '', 'D'], ['E', 'F', 'G']], font_size: 22, format: {
    column_widths: { unit: 'relative', values: [1, 2, 1] }, row_heights: { unit: 'absolute', values: [160, 160, 160] }, merges: [{ row: 0, column: 0, row_span: 2, col_span: 2 }],
    cells: [{ row: 0, column: 0, style: { fill: 'AAFFCC', outline: { color: 'CC0000', width: 3 }, padding: { left: 12, right: 14, top: 10, bottom: 8 }, vertical: 'middle', text_style: { bold: true }, text_format: { paragraphs: [{ runs: [{ text: 'Merged', style: { italic: true } }] }] } } }],
  } }]);
  const table = page.locator('.slide-stage table');
  await expect(table.locator('th, td')).toHaveCount(6);
  const anchor = table.getByRole('columnheader', { name: 'Merged' });
  await expect(anchor).toHaveAttribute('rowspan', '2');
  await expect(anchor).toHaveAttribute('colspan', '2');
  await expect(anchor).toHaveCSS('background-color', 'rgb(170, 255, 204)');
  await expect(anchor).toHaveCSS('padding-left', '12px');
  await expect(anchor).toHaveCSS('border-top-width', '3px');
  await expect(anchor.locator('span').last()).toHaveCSS('font-style', 'italic');
  const widths = await table.locator('col').evaluateAll((nodes) => nodes.map((node) => (node as HTMLElement).style.width));
  expect(widths).toEqual(['25%', '50%', '25%']);
});

test('visual rendering keeps hidden ink absent and uses safe SVG paths, masks and unique gradients', async ({ page }) => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#087f73"/></svg>';
  const fallback = (await sharp({ create: { width: 10, height: 10, channels: 4, background: '#087f73' } }).png().toBuffer()).toString('base64');
  await render(page, [
    { ...text, id: 'hidden', visual: { hidden: true } },
    { ...bounds, id: 'vector', type: 'polygon', points: [[0, 0], [0.2, 1], [0.8, 0], [1, 1]], fill: 'FF0000', stroke: '202525', stroke_width: 2, visual: { gradient: { kind: 'linear', angle: 45, stops: [{ offset: 0, color: 'FF0000', opacity: 1 }, { offset: 1, color: '0000FF', opacity: 0.4 }] }, path: { commands: [{ op: 'move', point: [0, 0] }, { op: 'cubic', control1: [0.2, 1], control2: [0.8, 0], point: [1, 1] }] } } },
    { ...bounds, id: 'svg', type: 'picture', width: 120, height: 120, base64: fallback, mime_type: 'image/png', alt: 'Synthetic SVG', crop: { left: 0, right: 0, top: 0, bottom: 0 }, svg, visual: { picture_mask: 'ellipse' } },
  ]);
  await expect(page.locator('.slide-stage [data-element-id="hidden"] .slide-text')).toHaveCount(0);
  const vector = page.locator('.slide-stage [data-element-id="vector"]');
  await expect(vector.locator('path[d*="C"]')).toHaveAttribute('fill', /^url\(#/);
  await expect(vector.locator('linearGradient stop')).toHaveCount(2);
  const ids = await page.locator('linearGradient').evaluateAll((nodes) => nodes.map((node) => node.id));
  expect(new Set(ids).size).toBe(ids.length);
  const image = page.locator('.slide-stage img');
  await expect(image).toHaveAttribute('src', /^data:image\/svg\+xml;base64,/);
  await expect(image).toHaveJSProperty('naturalWidth', 10);
  await expect(image.locator('..')).toHaveCSS('clip-path', 'ellipse(50% 50% at 50% 50%)');
  await image.evaluate((node) => node.dispatchEvent(new Event('error')));
  await expect(image).toHaveAttribute('src', `data:image/png;base64,${fallback}`);
  await expect(image).toHaveJSProperty('naturalWidth', 10);
});

for (const [kind, primitive] of [['combo', '.recharts-line'], ['bubble', '.recharts-scatter'], ['radar', '.recharts-radar'], ['radar_filled', '.recharts-radar'], ['percent_stacked_bar', '.recharts-bar']] as const) {
  test(`${kind} uses its actual chart family`, async ({ page }) => {
    const errors = await render(page, [{ ...bounds, type: 'chart', kind, categories: kind === 'bubble' ? ['1', '2', '3'] : ['A', 'B', 'C'], series: [
      { name: 'One', values: [10, 20, 30], color: '087F73', kind: kind === 'combo' ? 'column' : undefined, bubble_sizes: kind === 'bubble' ? [1, 4, 9] : undefined },
      { name: 'Two', values: [30, 20, 10], color: 'CC5847', kind: kind === 'combo' ? 'line' : undefined, axis: kind === 'combo' ? 'secondary' : undefined, bubble_sizes: kind === 'bubble' ? [9, 4, 1] : undefined },
    ], options: { legend: 'top', data_labels: { show_value: true } } }]);
    const chart = page.locator('.slide-stage [role="img"]');
    await expect(chart.locator(primitive).first()).toBeVisible();
    if (kind === 'percent_stacked_bar') await expect(chart.getByText('100%', { exact: true })).toBeVisible();
    expect(errors).toEqual([]);
  });
}

test('chart axes and actual-value labels honor options and retain a zero baseline', async ({ page }) => {
  await render(page, [{ ...bounds, type: 'chart', kind: 'column', categories: ['A', 'B'], series: [{ name: 'Revenue', values: [12, 18], color: '087F73', error_bars: { kind: 'fixed_value', value: 2 } }], options: { legend: 'hidden', primary_axis: { min: 0, max: 20, major_unit: 5, number_format: '0.0' }, data_labels: { show_value: true, number_format: '0.00' } } }]);
  const chart = page.locator('.slide-stage [data-core-preview]');
  await expect(chart).toHaveAttribute('data-core-preview', 'ready', { timeout: 20_000 });
  const result = await chart.locator('img').evaluate((image: HTMLImageElement) => {
    const source = decodeURIComponent(image.src.slice(image.src.indexOf(',') + 1));
    const svg = new DOMParser().parseFromString(source, 'image/svg+xml');
    return { labels: [...svg.querySelectorAll('title')].map(node => node.textContent), errors: svg.querySelectorAll('line[stroke-width="1.5"]').length, bars: [...svg.querySelectorAll('rect[fill="#087F73"]')].map(node => Number(node.getAttribute('y')) + Number(node.getAttribute('height'))) };
  });
  expect(result.labels).toContain('5.0');
  expect(result.labels).toContain('12.00');
  expect(result.errors).toBe(6);
  expect(result.bars).toHaveLength(2);
  expect(result.bars[0]).toBeCloseTo(result.bars[1]);
});

test('chartEx funnel and waterfall use distinct geometry and explicit totals', async ({ page }, testInfo) => {
  const errors = await render(page, [
    { ...bounds, id: 'funnel', width: 540, height: 430, type: 'chart', kind: 'funnel' as ChartKind, categories: ['Start', 'Middle', 'End'], series: [{ name: 'Synthetic funnel', color: '087F73', values: [120,80,40] }] },
    { ...bounds, id: 'waterfall', x: 660, width: 540, height: 430, type: 'chart', kind: 'waterfall' as ChartKind, categories: ['Start', 'Change', 'Total'], series: [{ name: 'Synthetic waterfall', color: '087F73', values: [120,-80,40] }], options: JSON.parse('{"waterfall_totals":[0,2]}') },
  ]);
  const funnel = page.locator('.slide-stage [data-chart-layout="funnel"]');
  const waterfall = page.locator('.slide-stage [data-chart-layout="waterfall"]');
  await expect(funnel.locator('.recharts-trapezoid')).toHaveCount(3);
  await expect(funnel.locator('.recharts-bar')).toHaveCount(0);
  await expect(waterfall.locator('.recharts-bar-rectangle')).toHaveCount(3);
  const ranges = await page.evaluate(async () => {
    const path = '/src/chart-render.ts';
    const helpers = await import(path);
    return helpers.waterfallData({ kind: 'waterfall', categories: ['Start','Change','Total'], series: [{ name: 'Series', color: '087F73', values: [120,-80,40] }], options: { waterfall_totals: [0,2] } });
  });
  expect(ranges.map((entry: { range: number[] }) => entry.range)).toEqual([[0,120],[120,40],[0,40]]);
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 960 });
    await page.evaluate(async () => { await document.fonts.ready; });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await expect(funnel).toHaveAttribute('data-preview-warnings', /Office/);
    await expect(waterfall).toHaveAttribute('data-preview-warnings', /Office/);
    await page.screenshot({ path: testInfo.outputPath(`chartex-${width}.png`) });
  }
  expect(errors).toEqual([]);
});

test('all eighteen kinds remain bounded on desktop and mobile with fonts ready', async ({ page }, testInfo) => {
  const kinds: ChartKind[] = ['column', 'bar', 'line', 'pie', 'doughnut', 'area', 'scatter', 'stacked_column', 'stacked_bar', 'percent_stacked_column', 'percent_stacked_bar', 'combo', 'bubble', 'radar', 'radar_filled', 'column3d', 'bar3d', 'pie3d'];
  const errors = await render(page, kinds.map((kind, index) => ({ ...bounds, id: kind, type: 'chart', kind, x: 12 + index % 6 * 210, y: 12 + Math.floor(index / 6) * 230, width: 200, height: 210, categories: ['1', '2', '3'], series: [{ name: 'Series', values: [1, 2, 3], color: '087F73', kind: kind === 'combo' ? 'line' : undefined, bubble_sizes: kind === 'bubble' ? [1, 2, 3] : undefined }] })));
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 960 });
    await page.evaluate(async () => { await document.fonts.ready; });
    await expect(page.locator('.slide-stage .recharts-wrapper > .recharts-surface')).toHaveCount(18);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const escaped = await page.locator('.slide-stage .recharts-wrapper > .recharts-surface').evaluateAll((nodes) => nodes.filter((node) => { const outer = node.closest('[data-element-id]')!.getBoundingClientRect(); const box = node.getBoundingClientRect(); return box.left < outer.left - 1 || box.top < outer.top - 1 || box.right > outer.right + 1 || box.bottom > outer.bottom + 1; }).length);
    expect(escaped).toBe(0);
    await page.screenshot({ path: testInfo.outputPath(`document-render-${width}.png`) });
  }
  await expect(page.locator('.slide-stage [data-element-id="column3d"] [role="img"]')).toHaveAttribute('title', /2D projection/);
  expect(errors).toEqual([]);
});

test('run decorations override inherited underline and table highlights do not replace cell fill', async ({ page }) => {
  await render(page, [
    { ...text, id: 'decoration', format: { underline: true, paragraphs: [{ runs: [{ text: 'Underlined' }, { text: 'Plain', style: { underline: false } }] }] } },
    { ...bounds, id: 'highlight-table', y: 300, height: 120, type: 'table', font_size: 24, rows: [['Highlighted']], format: { cells: [{ row: 0, column: 0, style: { fill: 'AAFFCC', text_style: { highlight: 'FFFF00', baseline: -20000, language: 'en-US' } } }] } },
  ]);
  const plain = page.locator('.slide-stage').getByText('Plain', { exact: true });
  const inheritedDecoration = await plain.evaluate((node) => { const decorated: string[] = []; for (let parent = node.parentElement; parent; parent = parent.parentElement) if (getComputedStyle(parent).textDecorationLine.includes('underline')) decorated.push(parent.className); return decorated; });
  expect(inheritedDecoration).toEqual([]);
  await expect(page.locator('.slide-stage').getByText('Underlined', { exact: true })).toHaveCSS('text-decoration-line', 'underline');
  await expect(page.locator('.slide-stage th')).toHaveCSS('background-color', 'rgb(170, 255, 204)');
  await expect(page.locator('.slide-stage th span[lang="en-US"]')).toHaveCSS('background-color', 'rgb(255, 255, 0)');
});

test('visual transforms preserve legacy shape rotation, child coordinates and editor hitboxes', async ({ page }) => {
  await render(page, [
    { ...bounds, id: 'shape', type: 'shape', preset: 'roundRect', width: 180, height: 100, text: 'Shape', font_size: 22, bold: false, color: '000000', rotation: 30, fill: 'AAFFCC', stroke: 'CC0000', stroke_width: 3, visual: { flip_h: true, opacity: 0.3, adjustments: [{ name: 'adj', value: 30000 }] } },
    { ...bounds, id: 'group', x: 400, width: 400, height: 200, type: 'group', view_width: 800, view_height: 400, visual: { rotation: 15 }, children: [{ ...text, id: 'child', x: 40, y: 60, width: 200, height: 120, text: 'Child', visual: { flip_v: true } }] },
  ]);
  const shape = page.locator('.slide-stage [data-element-id="shape"]');
  await expect(shape.getByRole('button', { name: 'Edit shape', exact: true })).toBeVisible();
  await expect(shape.locator('.document-ink')).toHaveCSS('transform', 'matrix(-1, 0, 0, 1, 0, 0)');
  await expect(shape.locator('.preset-shape path')).toHaveAttribute('fill-opacity', '0.3');
  await expect(shape.locator('.preset-shape path')).not.toHaveAttribute('stroke-opacity');
  const group = page.locator('.slide-stage [data-element-id="group"]');
  const child = group.getByText('Child', { exact: true });
  await expect(child).toBeVisible();
  expect(await child.evaluate((node) => { let parent = node.parentElement; while (parent && parent.style.left !== '40px') parent = parent.parentElement; return parent?.style.top; })).toBe('60px');
});

test('chart calculations are bounded and preserve actual labels, asymmetric errors and fitted values', async ({ page }) => {
  await render(page, [text]);
  const result = await page.evaluate(async () => {
    const path = '/src/chart-render.ts';
    const helpers = await import(path);
    const apiPath = '/src/api.ts';
    const { core } = await import(apiPath);
    const series = { name: 'Linear', values: [3, 5, 7], color: '087F73', trendline: { kind: 'linear', display_equation: true, display_r_squared: true } };
    const projection = (entry: unknown) => core({ op: 'compute_chart_presentation', kind: 'scatter', categories: ['1','2','3'], series: [entry] });
    return {
      bounded: helpers.axisTicks({ major_unit: 1e-12 }, [0, 1]),
      logarithmic: helpers.axisTicks({ log_base: 2 }, [1, 16]),
      domain: helpers.axisDomain(undefined, [12, 18]),
      negative: helpers.numberFormat(-1234.5, '#,##0.00;(#,##0.00)'),
      literal: helpers.numberFormat(12, '"Revenue "0.00'),
      fit: (await projection(series)).series[0].trend,
      moving: (await projection({ ...series, trendline: { kind: 'moving_average', period: 2 } })).series[0].trend,
      custom: (await projection({ ...series, error_bars: { kind: 'custom', plus: [1, 2, 3], bar_type: 'plus' } })).series[0].errors[1],
      percentage: (await projection({ ...series, error_bars: { kind: 'percentage', value: 10 } })).series[0].errors[1],
      standard: (await projection({ ...series, error_bars: { kind: 'standard_deviation', value: 1 } })).series[0].errors[0],
      standardError: (await projection({ ...series, error_bars: { kind: 'standard_error' } })).series[0].errors[1],
      warnings: helpers.chartWarnings({ kind: 'column3d', categories: ['1', '2', '3'], series: [{ ...series, trendline: { kind: 'polynomial', order: 2 } }] }),
    };
  });
  expect(result.bounded).toBeUndefined();
  expect(result.logarithmic).toEqual([1, 2, 4, 8, 16]);
  expect(result.domain).toEqual([0, 18]);
  expect(result.negative).toBe('(1,234.50)');
  expect(result.literal).toBe('Revenue 12.00');
  expect(result.fit.points[0].y).toBeCloseTo(3);
  expect(result.fit.points.at(-1).y).toBeCloseTo(7);
  expect(result.fit.equation).toContain('z =');
  expect(result.fit.r_squared).toBeCloseTo(1);
  expect(result.moving.points).toEqual([{ x: 2, y: 4 }, { x: 3, y: 6 }]);
  expect([result.custom.lower.y, result.custom.upper.y]).toEqual([5,7]);
  expect([result.percentage.lower.y, result.percentage.upper.y]).toEqual([4.5,5.5]);
  expect([result.standard.lower.y, result.standard.upper.y]).toEqual([3,7]);
  expect(result.standardError.lower.y).toBeCloseTo(5 - 2 / Math.sqrt(3));
  expect(result.warnings.join('; ')).toMatch(/2D projection/);
});

test('area error bars, reversed categories and logarithmic ticks are actually painted', async ({ page }) => {
  await render(page, [{ ...bounds, type: 'chart', kind: 'area', categories: ['1', '2', '3'], series: [{ name: 'Area', values: [2, 4, 8], color: '087F73', error_bars: { kind: 'fixed_value', value: 0.5 }, trendline: { kind: 'linear' } }], options: { legend: 'right', primary_axis: { log_base: 2, min: 1, max: 16 }, category_axis: { reverse: true } } }]);
  const chart = page.locator('.slide-stage [data-core-preview]');
  await expect(chart).toHaveAttribute('data-core-preview', 'ready', { timeout: 20_000 });
  const result = await chart.locator('img').evaluate((image: HTMLImageElement) => {
    const svg = new DOMParser().parseFromString(decodeURIComponent(image.src.slice(image.src.indexOf(',') + 1)), 'image/svg+xml');
    return { errors: svg.querySelectorAll('line[stroke-width="1.5"]').length, labels: [...svg.querySelectorAll('title')].map(node => node.textContent), positions: [...svg.querySelectorAll('title')].filter(node => ['1','3'].includes(node.textContent ?? '')).map(node => { const values = node.closest('g[transform]')!.getAttribute('transform')!.match(/[-\d.]+/g)!.map(Number); return { text: node.textContent, x: values[0], y: values[1] }; }) };
  });
  expect(result.errors).toBe(9);
  expect(result.labels).toContain('16');
  const positions = result.positions;
  const categoryThree = positions.find((entry) => entry.text === '3')!;
  const categoryOne = positions.filter((entry) => entry.text === '1').sort((first, second) => second.y - first.y)[0];
  expect(categoryOne.x).toBeGreaterThan(categoryThree.x);
});

test('radar category reversal changes angular placement rather than ignoring the option', async ({ page }) => {
  await render(page, [{ ...bounds, type: 'chart', kind: 'radar', categories: ['A', 'B', 'C'], series: [{ name: 'Radar', values: [1, 2, 3], color: '087F73' }], options: { category_axis: { reverse: true } } }]);
  const chart = page.locator('.slide-stage .document-chart');
  const second = await chart.getByText('B', { exact: true }).boundingBox();
  const third = await chart.getByText('C', { exact: true }).boundingBox();
  expect(second!.x).toBeLessThan(third!.x);
});

test('effects, masks, tabs and stored field text are rendered without navigation or execution', async ({ page }) => {
  const image = (await sharp({ create: { width: 10, height: 10, channels: 4, background: '#087f73' } }).png().toBuffer()).toString('base64');
  const elements: Element[] = [
    { ...text, text: 'Stored\t<script>literal</script>', format: { paragraphs: [{ tabs: [{ position: 914400, alignment: 'left' }], runs: [{ text: 'Stored', field: { id: '{00000000-0000-0000-0000-000000000001}', kind: 'datetime1' } }, { text: '\t<script>literal</script>' }] }] }, visual: { text_warp: 'wave1', shadow: { color: '000000', opacity: 0.5, blur: 4, distance: 10, angle: 0 }, glow: { color: '00FF00', opacity: 0.25, radius: 8 }, soft_edge: 2, reflection: { blur: 3, distance: 4, start_opacity: 0.4, end_opacity: 0, end_position: 0.8 } } },
    { ...bounds, id: 'radial', x: 920, width: 180, height: 160, type: 'rect', fill: 'FF0000', visual: { gradient: { kind: 'radial', center: [0.3, 0.7], stops: [{ offset: 0, color: 'FF0000', opacity: 0.5 }, { offset: 1, color: '0000FF', opacity: 1 }] } } },
    ...(['ellipse', 'round_rect', 'diamond', 'hexagon'] as const).map((mask, index) => ({ ...bounds, id: mask, x: 80 + index * 200, y: 540, width: 100, height: 100, type: 'picture' as const, base64: image, mime_type: 'image/png' as const, alt: mask, crop: { left: 0.1, right: 0.1, top: 0, bottom: 0 }, visual: { picture_mask: mask } })),
  ];
  const errors = await render(page, elements);
  const content = page.locator('.slide-stage [data-element-id="content"]');
  await expect(content.locator('[data-field-kind="datetime1"]')).toHaveText('Stored');
  await expect(content.locator('script')).toHaveCount(0);
  await expect(content.locator('.document-tabs')).toHaveCSS('grid-template-columns', '96px 704px');
  await expect(content.locator('.document-ink')).toHaveAttribute('title', /reflection-only blur/i);
  expect(await content.locator('.document-ink').evaluate((node) => getComputedStyle(node).filter)).toContain('drop-shadow');
  const radial = page.locator('.slide-stage [data-element-id="radial"]');
  await expect(radial.locator('radialGradient')).toHaveAttribute('cx', '0.3');
  await expect(radial.locator('stop').first()).toHaveAttribute('stop-opacity', '0.5');
  for (const mask of ['ellipse', 'round_rect', 'diamond', 'hexagon']) {
    const picture = page.locator(`.slide-stage [data-element-id="${mask}"] img`);
    expect(await picture.locator('..').evaluate((node) => getComputedStyle(node).clipPath)).not.toBe('none');
    await expect(picture).toHaveCSS('width', '125px');
  }
  expect(errors).toEqual([]);
});
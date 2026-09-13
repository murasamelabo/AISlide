import { test, expect } from '@playwright/test';
import sharp from 'sharp';
import { readFile } from 'node:fs/promises';
import AxeBuilder from '@axe-core/playwright';
import { requestCore } from '../../tools/core-client.mjs';

test('native chart can be added, edited, validated and undone', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await expect(page.getByRole('button', { name: 'Add chart', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Add chart', exact: true }).click();
  await expect(page.locator('.slide-stage .recharts-wrapper')).toHaveCount(1);
  await page.getByLabel('Chart type', { exact: true }).selectOption('line');
  await page.getByLabel('Chart data', { exact: true }).fill(JSON.stringify({ categories: ['Jan', 'Feb', 'Mar'], series: [{ name: 'Fixture values', values: [-5, 0, 12], color: '087F73' }] }));
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
  await expect(page.locator('.slide-stage .recharts-line')).toHaveCount(1);
  await page.getByLabel('Chart data', { exact: true }).fill(JSON.stringify({ categories: ['Jan'], series: [{ name: 'Invalid', values: [1, 2], color: '087F73' }] }));
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('chart series');
  await expect(page.locator('.slide-stage .recharts-line')).toHaveCount(1);
  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage .recharts-bar')).toHaveCount(1);
  await expect(page.locator('.slide-stage .recharts-bar-rectangle')).toHaveCount(3);
  const barHeights = await page.locator('.slide-stage .recharts-bar-rectangle').evaluateAll((bars) => bars.map((bar) => (bar as SVGGraphicsElement).getBBox().height));
  expect(barHeights.every((height) => height > 0)).toBe(true);
  await page.screenshot({ path: '.artifacts/poc-chart-desktop.png' });
});

test('a raster picture is decoded, cropped and undoable', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await expect(page.getByRole('button', { name: 'Add picture', exact: true })).toBeVisible();
  const chooser = page.waitForEvent('filechooser');
  await page.getByRole('button', { name: 'Add picture', exact: true }).click();
  await (await chooser).setFiles({ name: 'synthetic-pixels.png', mimeType: 'image/png', buffer: await sharp({ create: { width: 120, height: 80, channels: 3, background: '#087F73' } }).png().toBuffer() });
  const picture = page.locator('.slide-stage img');
  await expect(picture).toHaveAttribute('alt', 'synthetic-pixels.png');
  expect(await picture.evaluate((image) => (image as HTMLImageElement).naturalWidth)).toBe(120);
  await page.getByLabel('Crop left (%)', { exact: true }).fill('10');
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
  await expect(page.getByLabel('Crop left (%)', { exact: true })).toHaveValue('10');
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await page.getByRole('button', { name: /^Select picture-/ }).click();
  await expect(page.getByLabel('Crop left (%)', { exact: true })).toHaveValue('0');
});

test('connected process groups remain editable and move as one object', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await expect(page.getByRole('button', { name: 'Add process diagram', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Add process diagram', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Collect', { exact: true })).toBeVisible();
  await page.getByLabel('Step 1', { exact: true }).fill('Approved input');
  await page.getByLabel('Element x', { exact: true }).fill('80');
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Approved input', { exact: true })).toBeVisible();
  await expect(page.locator('.slide-stage .connector-line')).toHaveCount(2);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Collect', { exact: true })).toBeVisible();
});

test('source mapping preserves provenance across project save and reopen', async ({ page }, testInfo) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await expect(page.getByRole('button', { name: 'Sources', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Sources', exact: true }).click();
  await page.getByLabel('Source file', { exact: true }).setInputFiles({ name: 'provided.csv', mimeType: 'text/csv', buffer: Buffer.from('Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n') });
  await expect(page.locator('.source-preview')).toContainText('Q1');
  await page.getByLabel('Report title', { exact: true }).fill('Source-bound report');
  await page.getByRole('button', { name: 'Compile data report', exact: true }).click();
  await expect(page.locator('.document-name strong')).toHaveText('Source-bound report');
  await page.getByRole('button', { name: /^Slide 4:/ }).click();
  await expect(page.locator('.slide-stage .recharts-wrapper')).toHaveCount(1);
  const downloads: import('@playwright/test').Download[] = [];
  page.on('download', (download) => downloads.push(download));
  await page.getByRole('button', { name: 'Save project', exact: true }).click();
  await expect.poll(() => downloads.length).toBe(2);
  const paths: string[] = [];
  for (const download of downloads) { const path = testInfo.outputPath(download.suggestedFilename()); await download.saveAs(path); paths.push(path); }
  const checkpointPath = paths.find((path) => path.endsWith('.aislide.json'))!;
  const checkpoint = JSON.parse(await readFile(checkpointPath, 'utf8'));
  expect(checkpoint.document.sources[0].name).toBe('provided.csv');
  expect(checkpoint.document.bindings.length).toBeGreaterThan(0);
  await page.getByRole('button', { name: /^Slide 1:/ }).click();
  await page.getByRole('button', { name: 'Select title', exact: true }).click();
  await page.getByLabel('Text content', { exact: true }).fill('Manual later edit');
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Manual later edit', { exact: true })).toBeVisible();
  await page.getByLabel('Open project files', { exact: true }).setInputFiles(paths);
  await expect(page.locator('.slide-stage').getByText('Source-bound report', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Sources', exact: true }).click();
  await expect(page.getByText('provided.csv', { exact: true }).first()).toBeVisible();
  await page.screenshot({ path: '.artifacts/poc-sources-desktop.png' });
});

test('native import preserves no-op bytes and allows a supported text edit', async ({ page }, testInfo) => {
  const report = await requestCore({ op: 'sample' });
  const compiled = await requestCore({ op: 'compile', report });
  const original = await requestCore({ op: 'export', deck: compiled.deck });
  const bytes = Buffer.from(original.base64, 'base64');
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByLabel('Open presentation file', { exact: true }).setInputFiles({ name: 'original.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: bytes });
  await expect(page.getByRole('dialog', { name: 'Imported presentation', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  const noOp = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export PPTX', exact: true }).click();
  const saved = await noOp; const path = testInfo.outputPath('no-op.pptx'); await saved.saveAs(path);
  expect(await readFile(path)).toEqual(bytes);
  await page.getByRole('button', { name: /^Slide 3:/ }).click();
  await page.getByRole('button', { name: 'Select shape-4', exact: true }).click();
  await page.getByLabel('Text content', { exact: true }).fill('Imported native title');
  await page.getByRole('button', { name: 'Apply changes', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Imported native title', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Validate layout', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Layout validation' })).toContainText('cosmic-text');
  await page.screenshot({ path: '.artifacts/poc-import-layout.png' });
});

test('source controls are labeled and fit a small viewport', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Sources', exact: true }).click();
  await page.getByLabel('Source file', { exact: true }).setInputFiles({ name: 'small.csv', mimeType: 'text/csv', buffer: Buffer.from('Quarter,Value\nQ1,1\nQ2,2\n') });
  await expect(page.locator('.source-preview')).toContainText('Q1');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const audit = await new AxeBuilder({ page }).include('dialog').analyze();
  expect(audit.violations.filter((issue) => ['critical', 'serious'].includes(issue.impact ?? ''))).toEqual([]);
  await page.screenshot({ path: '.artifacts/poc-sources-mobile.png', fullPage: true });
});
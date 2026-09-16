import { test, expect } from '@playwright/test';
import { openSample } from './fixtures';
import AxeBuilder from '@axe-core/playwright';
import sharp from 'sharp';
import type { PartCatalog } from '../../apps/studio/src/types';

test('parts library inserts, updates and reopens editable metadata from one PPTX', async ({ page }) => {
  await openSample(page);
  await page.getByRole('button', { name: 'Parts library', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Parts library', exact: true });
  await dialog.getByLabel('Part category', { exact: true }).selectOption('vertical-bar-graph');
  await dialog.getByRole('button', { name: 'Balanced columns', exact: true }).click();
  await dialog.getByLabel('Part title', { exact: true }).fill('Quarterly part');
  await dialog.getByRole('button', { name: 'Insert part', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('.slide-stage').getByText('Quarterly part', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Edit part data', exact: true }).click();
  await dialog.getByLabel('Part title', { exact: true }).fill('Revised part');
  await dialog.getByRole('button', { name: 'Update part', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Revised part', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Quarterly part', { exact: true })).toBeVisible();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const file = await download;
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles((await file.path())!);
  await expect(page.getByRole('button', { name: /^Select part-/ })).toHaveCount(1);
  await page.getByRole('button', { name: /^Select part-/ }).click();
  await expect(page.getByRole('button', { name: 'Edit part data', exact: true })).toBeEnabled();
});

for (const width of [1440,390]) {
  test(`parts library fits and is labeled at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await openSample(page);
    await page.getByRole('button', { name: 'Parts library', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Parts library', exact: true });
    await dialog.getByLabel('Part category', { exact: true }).selectOption('map');
    await dialog.getByRole('button', { name: 'Global locations', exact: true }).click();
    await expect(dialog.locator('.part-preview svg').first()).toBeVisible();
    await expect(dialog.locator('.part-preset.active')).toHaveCSS('border-left-color', 'rgb(0, 23, 193)');
    await expect(dialog.locator('.part-preset.active strong')).toHaveCSS('font-size', '16px');
    await page.evaluate(() => document.fonts.ready);
    expect(await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
    const results = await new AxeBuilder({page}).include('.parts-panel').analyze();
    expect(results.violations.filter((issue) => ['serious','critical'].includes(issue.impact ?? ''))).toEqual([]);
    await dialog.screenshot({ path: `.artifacts/parts-library-${width}.png` });
  });
}

test('all 108 part previews render with visible content and fitted labels', async ({ page }) => {
  test.setTimeout(240_000);
  await page.setViewportSize({ width: 1500, height: 1000 });
  await openSample(page);
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  const catalog = await page.evaluate(async () => (await fetch('/api/core', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ op: 'part_catalog' }) })).json()) as PartCatalog;
  await page.getByRole('button', { name: 'Parts library', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Parts library', exact: true });
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  for (const preset of catalog.presets) {
    await dialog.getByLabel('Part category', { exact: true }).selectOption(preset.category);
    await dialog.getByRole('button', { name: preset.name, exact: true }).click();
    await expect(dialog.getByRole('button', { name: 'Insert part', exact: true })).toBeEnabled();
    await expect(dialog.locator('.part-preview [role="status"]')).toHaveCount(0);
    await page.evaluate(() => document.fonts.ready);
    const overflow = await dialog.locator('.part-preview .slide-text').evaluateAll((nodes) => nodes.filter((node) => node.scrollHeight > node.clientHeight + 2 || node.scrollWidth > node.clientWidth + 2).map((node) => node.textContent));
    expect(overflow, preset.id).toEqual([]);
    const pixels = await dialog.locator('.part-preview').screenshot({ path: `.artifacts/parts-previews/${preset.id.replaceAll('/', '-')}.png` });
    const stats = await sharp(pixels).removeAlpha().stats();
    expect(stats.channels.some((channel) => channel.stdev > 8), preset.id).toBe(true);
  }
  expect(errors).toEqual([]);
});

test('radial connectors have a visible SVG viewport', async ({ page }) => {
  await openSample(page);
  await page.getByRole('button', { name: 'Parts library', exact: true }).click();
  const dialog=page.getByRole('dialog', { name: 'Parts library', exact: true });
  await dialog.getByLabel('Part category', { exact: true }).selectOption('radiation');
  await expect(dialog.getByRole('button', { name: 'Insert part', exact: true })).toBeEnabled();
  const extents=await dialog.locator('.part-preview svg.connector-line').evaluateAll((nodes)=>nodes.map((node)=>[Number(node.getAttribute('width')),Number(node.getAttribute('height'))]));
  expect(extents).toHaveLength(4);
  expect(extents.every(([width,height])=>width>=1 && height>=1)).toBe(true);
});

test('part drafts survive category browsing and JSON to fields switching', async ({ page }) => {
  await openSample(page);
  await page.getByRole('button', { name: 'Parts library', exact: true }).click();
  const dialog=page.getByRole('dialog', { name: 'Parts library', exact: true });
  await dialog.getByLabel('Part category', { exact: true }).selectOption('vertical-bar-graph');
  await dialog.getByLabel('Part title', { exact: true }).fill('My retained draft');
  await dialog.getByRole('tab', { name: 'JSON', exact: true }).click();
  const input=dialog.getByLabel('Part metadata JSON', { exact: true });
  const data=JSON.parse(await input.inputValue()); data.categories[0]='Edited in JSON';
  await input.fill(JSON.stringify(data));
  await dialog.getByRole('tab', { name: 'JSON', exact: true }).click();
  await expect(input).toHaveValue(JSON.stringify(data));
  await dialog.locator('.part-preset').nth(1).click();
  await expect(input).toHaveValue(JSON.stringify(data));
  await dialog.getByRole('tab', { name: 'Data', exact: true }).click();
  await expect(dialog.getByLabel('categories 1', { exact: true })).toHaveValue('Edited in JSON');
  await dialog.getByLabel('Part category', { exact: true }).selectOption('map');
  await dialog.getByLabel('Part category', { exact: true }).selectOption('vertical-bar-graph');
  await expect(dialog.getByLabel('Part title', { exact: true })).toHaveValue('My retained draft');
  await expect(dialog.getByLabel('categories 1', { exact: true })).toHaveValue('Edited in JSON');
  await dialog.getByRole('tab', { name: 'JSON', exact: true }).click();
  await input.fill('{invalid');
  await dialog.getByRole('tab', { name: 'Data', exact: true }).click();
  await expect(input).toBeVisible();
  await expect(dialog.getByRole('alert')).toBeVisible();
});
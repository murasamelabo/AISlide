import { test, expect } from '@playwright/test';
import { openSample } from './fixtures';
import AxeBuilder from '@axe-core/playwright';

test('edit, undo and export a real twelve-slide deck', async ({ page }) => {
  await openSample(page);
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Select title', exact: true }).click();
  await page.getByLabel('Text content', { exact: true }).fill('Edited quarterly report');
  await page.getByRole('button', { name: 'Apply changes' }).click();
  await expect(page.locator('.slide-stage').getByText('Edited quarterly report', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Edited quarterly report', { exact: true })).toHaveCount(0);
  const pending = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const download = await pending;
  expect(download.suggestedFilename()).toBe('sample-report.pptx');
  await download.saveAs('.artifacts/report.pptx');
  await page.screenshot({ path: '.artifacts/studio-desktop.png', fullPage: true });
});

test('invalid report does not discard the open deck', async ({ page }) => {
  await openSample(page);
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Report data', exact: true }).click();
  await page.getByLabel('Report JSON', { exact: true }).fill('{"title": "incomplete"}');
  await page.getByRole('button', { name: 'Compile report', exact: true }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
});

test('small viewport remains usable and has no horizontal overflow', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await openSample(page);
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: '.artifacts/studio-mobile.png', fullPage: true });
});

test('editor controls have no serious automated accessibility violations', async ({ page }) => {
  await openSample(page);
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  const results = await new AxeBuilder({ page }).exclude('.slide-surface').analyze();
  expect(results.violations.filter((violation) => ['serious', 'critical'].includes(violation.impact ?? ''))).toEqual([]);
});

test('a standard PPTX restores its own title without a scene checkpoint', async ({ page }) => {
  await openSample(page);
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  const headers = { Origin: new URL(page.url()).origin };
  const response = await page.request.post('/api/core', { headers, data: { op: 'sample' } });
  const report = await response.json();
  const compiled = await page.request.post('/api/core', { headers, data: { op: 'compile', report } });
  const { deck } = await compiled.json();
  deck.title = 'Restored native presentation';
  const exported = await page.request.post('/api/core', { headers, data: { op: 'export', deck } });
  const { base64 } = await exported.json();
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'saved.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(base64, 'base64') });
  await expect(page.locator('.document-name strong')).toHaveText('Restored native presentation');
});
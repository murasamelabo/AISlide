import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

test('edit, undo and export a real twelve-slide deck', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Select title', exact: true }).click();
  await page.getByLabel('Text content', { exact: true }).fill('Edited quarterly report');
  await page.getByRole('button', { name: 'Apply changes' }).click();
  await expect(page.locator('.slide-stage').getByText('Edited quarterly report', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Edited quarterly report', { exact: true })).toHaveCount(0);
  const pending = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export PPTX', exact: true }).click();
  const download = await pending;
  expect(download.suggestedFilename()).toBe('report.pptx');
  await download.saveAs('.artifacts/report.pptx');
  await page.screenshot({ path: '.artifacts/studio-desktop.png', fullPage: true });
});

test('invalid report does not discard the open deck', async ({ page }) => {
  await page.goto('/');
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
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: '.artifacts/studio-mobile.png', fullPage: true });
});

test('editor controls have no serious automated accessibility violations', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  const results = await new AxeBuilder({ page }).exclude('.slide-surface').analyze();
  expect(results.violations.filter((violation) => ['serious', 'critical'].includes(violation.impact ?? ''))).toEqual([]);
});

test('a scene checkpoint can be reopened', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  const response = await page.request.post('/api/core', { headers: { Origin: 'http://127.0.0.1:4173' }, data: { op: 'sample' } });
  const report = await response.json();
  const compiled = await page.request.post('/api/core', { headers: { Origin: 'http://127.0.0.1:4173' }, data: { op: 'compile', report } });
  const { deck } = await compiled.json();
  deck.title = 'Restored checkpoint';
  await page.getByLabel('Open scene file').setInputFiles({ name: 'saved.scene.json', mimeType: 'application/json', buffer: Buffer.from(JSON.stringify(deck)) });
  await expect(page.locator('.document-name strong')).toHaveText('Restored checkpoint');
});
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { openSample } from './fixtures';

test.beforeEach(async ({ page }) => {
  await openSample(page);
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Generate with AI', exact: true }).click();
  await expect(page.getByText('local-fixture-model', { exact: true })).toBeVisible();
  await page.getByLabel('Slide count', { exact: true }).fill('3');
});

test('a model draft is reviewed before application and can be undone', async ({ page }) => {
  await page.getByLabel('Brief', { exact: true }).fill('Prepare a concise data report');
  await page.getByLabel('Source text', { exact: true }).fill('Synthetic fixture notes. Not factual data.');
  await page.getByRole('button', { name: 'Generate draft', exact: true }).click();
  await expect(page.locator('.generation-review')).toBeVisible();
  await expect(page.locator('.document-name strong')).toHaveText('Quarterly performance');
  await expect(page.getByText('AI content unverified', { exact: true })).toBeVisible();
  await page.screenshot({ path: '.artifacts/generation-desktop.png', fullPage: true });
  await page.getByRole('button', { name: 'Apply draft', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(3);
  await expect(page.locator('.document-name strong')).toHaveText('Fixture-generated analysis');
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
});

test('invalid model output preserves the existing presentation', async ({ page }) => {
  await page.getByLabel('Brief', { exact: true }).fill('FAIL_INVALID_JSON');
  await page.getByRole('button', { name: 'Generate draft', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('model output is not valid');
  await expect(page.getByRole('button', { name: 'Apply draft', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
});

test('cancel stops generation without changing the deck and permits a retry', async ({ page }) => {
  await page.getByLabel('Brief', { exact: true }).fill('WAIT_FOR_CANCELLATION');
  const pending = page.waitForRequest((request) => request.url().endsWith('/api/core') && request.postDataJSON()?.op === 'generate');
  await page.getByRole('button', { name: 'Generate draft', exact: true }).click();
  await pending;
  await page.getByRole('button', { name: 'Cancel generation', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Generation cancelled');
  await expect(page.locator('.document-name strong')).toHaveText('Quarterly performance');
  await page.getByLabel('Brief', { exact: true }).fill('Retry after cancellation');
  await page.getByRole('button', { name: 'Generate draft', exact: true }).click();
  await expect(page.locator('.generation-review')).toBeVisible();
});

test('generation dialog fits small screens and labels its controls', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const dialog = page.getByRole('dialog', { name: 'Generate report' });
  const bounds = await dialog.boundingBox();
  expect(bounds!.width).toBeLessThanOrEqual(390);
  const results = await new AxeBuilder({ page }).include('dialog').analyze();
  expect(results.violations.filter((issue) => ['critical', 'serious'].includes(issue.impact ?? ''))).toEqual([]);
  await page.screenshot({ path: '.artifacts/generation-mobile.png', fullPage: true });
});
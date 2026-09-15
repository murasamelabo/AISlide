import { test, expect } from '@playwright/test';

test('text edits directly on the slide, supports multiline input and undo', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Edit title', exact: true }).dblclick();
  const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
  await expect(editor).toBeVisible();
  await editor.fill('Direct slide editing\n\u65e5\u672c\u8a9e\u5165\u529b');
  await editor.press('Control+Enter');
  await expect(editor).toHaveCount(0);
  await expect(page.locator('.slide-stage').getByText('Direct slide editing', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Quarterly performance', { exact: false })).toBeVisible();
});

test('escape cancels a direct edit and IME composition does not submit it', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Edit title', exact: true }).dblclick();
  const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
  await editor.fill('Discard this draft');
  await editor.dispatchEvent('compositionstart');
  await editor.press('Control+Enter');
  await expect(editor).toBeVisible();
  await editor.dispatchEvent('compositionend');
  await editor.press('Escape');
  await expect(editor).toHaveCount(0);
  await expect(page.locator('.slide-stage').getByText('Quarterly performance', { exact: false })).toBeVisible();
});

test('escape cannot dismiss an on-slide commit that is already in flight', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Edit title', exact: true }).dblclick();
  const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
  await editor.fill('Pending commit is not a cancellation');
  let release = () => {};
  let started = () => {};
  const waiting = new Promise<void>((resolve) => { release = resolve; });
  const observed = new Promise<void>((resolve) => { started = resolve; });
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op === 'transaction') { started(); await waiting; }
    await route.continue();
  });
  try {
    await page.getByRole('button', { name: 'Apply on-slide edit', exact: true }).click();
    await observed;
    await page.locator('.inline-editor').dispatchEvent('keydown', { key: 'Escape', bubbles: true });
    await expect(editor).toBeVisible();
    await expect(page.getByRole('button', { name: 'Cancel on-slide edit', exact: true })).toBeDisabled();
  } finally { release(); }
  await expect(editor).toHaveCount(0);
  await expect(page.locator('.slide-stage').getByText('Pending commit is not a cancellation', { exact: true })).toBeVisible();
});

test('table cells edit on the slide without a JSON editor', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: /^Slide 4:/ }).click();
  await page.getByRole('button', { name: 'Edit data-table', exact: true }).dblclick();
  const cell = page.getByRole('textbox', { name: 'Table row 2 column 1', exact: true });
  await expect(cell).toBeVisible();
  await cell.fill('North America');
  await page.getByRole('button', { name: 'Apply on-slide edit', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('North America', { exact: true })).toBeVisible();
});

test('selected objects resize on the canvas and group labels edit in place', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Edit title', exact: true }).click();
  const title = page.locator('.canvas-workspace [data-element-id="title"]');
  const before = await title.evaluate((node) => parseFloat((node as HTMLElement).style.width));
  await page.getByRole('button', { name: 'Resize title', exact: true }).focus();
  await page.keyboard.press('ArrowLeft');
  await expect.poll(() => title.evaluate((node) => parseFloat((node as HTMLElement).style.width))).toBe(before - 1);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => title.evaluate((node) => parseFloat((node as HTMLElement).style.width))).toBe(before);
  await page.getByRole('button', { name: 'Add process diagram', exact: true }).click();
  await page.locator('.canvas-workspace .element-hitbox[aria-label^="Edit process-"]').dblclick();
  await page.getByRole('textbox', { name: /^Group text/ }).first().fill('Updated process');
  await page.getByRole('button', { name: 'Apply on-slide edit', exact: true }).click();
  await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Updated process' })).toBeVisible();
});

test('table rows and columns can be added from the canvas editor', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: /^Slide 4:/ }).click();
  await page.getByRole('button', { name: 'Edit data-table', exact: true }).dblclick();
  const rows = await page.locator('.inline-table tr').count();
  const columns = await page.locator('.inline-table tr').first().locator('td').count();
  await page.getByRole('button', { name: 'Add table row', exact: true }).click();
  await page.getByRole('button', { name: 'Add table column', exact: true }).click();
  await expect(page.locator('.inline-table tr')).toHaveCount(rows + 1);
  await expect(page.locator('.inline-table tr').first().locator('td')).toHaveCount(columns + 1);
  await page.getByRole('button', { name: 'Apply on-slide edit', exact: true }).click();
  await expect(page.locator('.canvas-workspace .slide-table tr')).toHaveCount(rows + 1);
});
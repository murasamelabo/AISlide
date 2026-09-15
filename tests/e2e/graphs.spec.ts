import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { readFile } from 'node:fs/promises';

test('graph preview initialization survives repeated editor mounts without a busy error', async ({ page }) => {
  await page.goto('/');
  for (let iteration = 0; iteration < 3; iteration++) {
    await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
    await expect(dialog.locator('.graph-fitted-label')).toHaveCount(3);
    await expect(dialog.locator('.graph-error')).toHaveCount(0);
    await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  }
});

test('architecture graphs can be created, edited, undone and reopened from PPTX', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
  await expect(dialog.locator('.react-flow__node')).toHaveCount(3);
  await dialog.getByLabel('Graph title', { exact: true }).fill('Service topology');
  await dialog.getByRole('button', { name: 'Select node api', exact: true }).click();
  await dialog.getByLabel('Node label', { exact: true }).fill('Service API');
  await dialog.getByRole('button', { name: 'Apply node', exact: true }).click();
  await dialog.getByRole('button', { name: 'Select edge request', exact: true }).click();
  await dialog.getByLabel('Edge route', { exact: true }).selectOption('elbow');
  await dialog.getByRole('button', { name: 'Apply edge', exact: true }).click();
  await dialog.getByRole('button', { name: 'Insert graph', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('.slide-stage').getByText('Service API', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Edit graph', exact: true }).click();
  await dialog.getByLabel('Graph title', { exact: true }).fill('Updated topology');
  await dialog.getByRole('button', { name: 'Update graph', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Updated topology', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Service topology', { exact: true })).toBeVisible();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const file = await download;
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: await readFile((await file.path())!) });
  await page.getByRole('button', { name: /^Select graph-/ }).click();
  await expect(page.getByRole('button', { name: 'Edit graph', exact: true })).toBeEnabled();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('graph canvas movement and resizing survive preview while cancel preserves the slide', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
  const node = dialog.locator('.react-flow__node[data-id="user"]');
  await expect(node).toBeVisible();
  const bounds = (await node.boundingBox())!;
  await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
  await page.mouse.down();
  await page.mouse.move(bounds.x + bounds.width / 2 + 40, bounds.y + bounds.height / 2 + 24, { steps: 8 });
  await page.mouse.up();
  await expect(dialog.getByLabel('Graph x', { exact: true })).not.toHaveValue('40');
  const moved = Number(await dialog.getByLabel('Graph x', { exact: true }).inputValue());
  await node.focus();
  await node.press('ArrowRight');
  await expect(dialog.getByLabel('Graph x', { exact: true })).toHaveValue(String(moved + 8));
  await dialog.getByLabel('Graph width', { exact: true }).fill('208');
  await dialog.getByRole('button', { name: 'Apply node', exact: true }).click();
  await dialog.getByRole('tab', { name: 'JSON', exact: true }).click();
  const value = JSON.parse(await dialog.getByLabel('Graph JSON', { exact: true }).inputValue());
  expect(value.nodes.find((entry: { id: string }) => entry.id === 'user')).toMatchObject({ x: moved + 8, width: 208 });
  await dialog.getByRole('tab', { name: 'Preview', exact: true }).click();
  await expect(dialog.locator('.graph-native-preview')).toContainText('Client');
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Select graph-/ })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
});

test('graph node and connection edits use undo without leaving dangling edges', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
  await dialog.getByRole('button', { name: 'Add ellipse node', exact: true }).click();
  await expect(dialog.locator('.react-flow__node')).toHaveCount(4);
  await dialog.getByLabel('Connect from', { exact: true }).selectOption('data');
  await dialog.getByLabel('Connect to', { exact: true }).selectOption({ label: 'Ellipse' });
  await dialog.getByRole('button', { name: 'Connect nodes', exact: true }).click();
  await expect(dialog.locator('.react-flow__edge')).toHaveCount(3);
  await dialog.getByRole('button', { name: /^Select node node-/ }).click();
  await dialog.getByRole('button', { name: 'Delete graph selection', exact: true }).click();
  await expect(dialog.locator('.react-flow__node')).toHaveCount(3);
  await expect(dialog.locator('.react-flow__edge')).toHaveCount(2);
  await dialog.getByRole('button', { name: 'Undo diagram edit', exact: true }).click();
  await expect(dialog.locator('.react-flow__node')).toHaveCount(4);
  await expect(dialog.locator('.react-flow__edge')).toHaveCount(3);
  await dialog.getByRole('button', { name: 'Redo diagram edit', exact: true }).click();
  await expect(dialog.locator('.react-flow__node')).toHaveCount(3);
  await expect(dialog.locator('.graph-error')).toHaveCount(0);
});

test('graph JSON errors retain the draft and do not change the active tab', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
  await dialog.getByRole('tab', { name: 'JSON', exact: true }).click();
  const input = dialog.getByLabel('Graph JSON', { exact: true });
  const value = JSON.parse(await input.inputValue());
  await input.fill('{invalid');
  await dialog.getByRole('tab', { name: 'Canvas', exact: true }).click();
  await expect(input).toHaveValue('{invalid');
  await expect(dialog.getByRole('tab', { name: 'JSON', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(dialog.locator('.graph-error')).toBeVisible();
  value.nodes[0].label = 'JSON edited client';
  await input.fill(JSON.stringify(value));
  await dialog.getByRole('tab', { name: 'Preview', exact: true }).click();
  await expect(dialog.locator('.graph-native-preview')).toContainText('JSON edited client');
  await expect(dialog.locator('.graph-error')).toHaveCount(0);
});

for (const width of [1440, 390]) {
  test(`DADS-inspired graph controls fit and are accessible at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 960 });
    await page.goto('/');
    await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
    await expect(dialog.locator('.react-flow__node')).toHaveCount(3);
    expect(await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth + 1)).toBe(true);
    const results = await new AxeBuilder({ page }).include('.graph-editor').analyze();
    expect(results.violations.filter((issue) => ['serious', 'critical'].includes(issue.impact ?? ''))).toEqual([]);
    await page.evaluate(() => document.fonts.ready);
    await dialog.screenshot({ path: `.artifacts/graph-editor-${width}.png` });
  });
}
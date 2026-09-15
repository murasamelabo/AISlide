import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import AxeBuilder from '@axe-core/playwright';

test('file and slide commands create, duplicate, rename and reopen a presentation', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'New presentation', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(1);
  await expect(page.locator('.slide-stage .element-hitbox')).toHaveCount(0);
  await page.getByRole('button', { name: 'New slide', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(2);
  await page.getByRole('button', { name: /^Slide 2:/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Duplicate slide', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(3);
  await page.getByRole('button', { name: /^Slide 3:/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Rename slide', exact: true }).click();
  await page.getByRole('dialog').getByLabel('Slide title', { exact: true }).fill('Design review');
  await page.getByRole('dialog').getByRole('button', { name: 'Apply', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Slide 3: Design review', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Save as', exact: true }).click();
  await page.getByLabel('PPTX filename', { exact: true }).fill('workspace-sample.pptx');
  const pending = page.waitForEvent('download');
  await page.getByRole('dialog').getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const file = await pending;
  expect(file.suggestedFilename()).toBe('workspace-sample.pptx');
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: await readFile((await file.path())!) });
  await expect(page.getByRole('button', { name: 'Slide 3: Design review', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'New slide', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(4);
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('new presentation protects unsaved changes with cancel and discard', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Add text', exact: true }).click();
  await page.getByRole('button', { name: 'New presentation', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Unsaved changes', exact: true });
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('New text', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'New presentation', exact: true }).click();
  await dialog.getByRole('button', { name: 'Discard changes', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(1);
  await expect(page.locator('.slide-stage .element-hitbox')).toHaveCount(0);
});

for (const mode of ['inline', 'properties']) {
  test(`file replacement protects unapplied ${mode} drafts`, async ({ page }) => {
    await page.goto('/');
    if (mode === 'inline') await page.locator('.slide-stage').getByRole('button', { name: 'Edit title', exact: true }).dblclick();
    else await page.getByRole('button', { name: 'Select title', exact: true }).click();
    const field = page.getByRole('textbox', { name: mode === 'inline' ? 'Slide text editor' : 'Text content', exact: true });
    await field.fill('Unapplied title\nDraft');
    await page.getByRole('button', { name: 'New presentation', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Unsaved changes', exact: true });
    await expect(dialog).toBeVisible();
    await dialog.getByRole('button', { name: 'Save and continue', exact: true }).click();
    await expect(dialog.getByRole('alert')).toContainText('Apply or cancel');
    await expect(page.locator('.thumbnail')).toHaveCount(12);
    await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
    await expect(field).toHaveValue('Unapplied title\nDraft');
    await page.getByRole('button', { name: mode === 'inline' ? 'Apply on-slide edit' : 'Apply changes', exact: true }).click();
    await expect(page.locator('.slide-stage').getByText('Unapplied title\nDraft', { exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'New presentation', exact: true }).click();
    const pending = page.waitForEvent('download');
    await dialog.getByRole('button', { name: 'Save and continue', exact: true }).click();
    const saved = await pending;
    expect(saved.suggestedFilename()).toMatch(/\.pptx$/);
    await expect(page.locator('.thumbnail')).toHaveCount(1);
  });
}

test('context commands target the clicked element and support keyboard dismissal', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Add rectangle', exact: true }).click();
  const title = page.locator('.slide-stage').getByRole('button', { name: 'Edit title', exact: true });
  await title.click({ button: 'right', position: { x: 3, y: 3 } });
  await page.getByRole('menuitem', { name: 'Duplicate element', exact: true }).click();
  await expect(page.locator('.layer-list button[aria-label^="Select copy-"]')).toHaveCount(1);
  await page.getByRole('button', { name: 'Select title', exact: true }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Delete element', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Select title', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Select title', exact: true })).toBeVisible();
  const layer = page.getByRole('button', { name: 'Select title', exact: true });
  await layer.focus(); await layer.press('Shift+F10');
  await expect(page.getByRole('menu')).toBeVisible();
  await page.keyboard.press('ArrowDown'); await page.keyboard.press('Escape');
  await expect(page.getByRole('menu')).toHaveCount(0);
  await expect(layer).toBeFocused();
});

test('built-in icons and pasted SVG become editable PNG pictures without unsafe SVG execution', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Icons and assets', exact: true });
  await dialog.getByLabel('Search icons', { exact: true }).fill('database');
  await dialog.getByRole('button', { name: 'Database icon', exact: true }).click();
  await dialog.getByRole('button', { name: 'Insert icon', exact: true }).click();
  await expect(page.locator('.slide-stage img')).toHaveCount(1);
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="8" fill="#007f73"/></svg>';
  await page.locator('.canvas-workspace').evaluate((element, text) => {
    const clipboardData = new DataTransfer(); clipboardData.setData('text/plain', text);
    element.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
  }, svg);
  await expect(page.locator('.slide-stage img')).toHaveCount(2);
  await expect(page.locator('.slide-stage img').last()).toHaveAttribute('src', /^data:image\/png;base64,/);
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  await dialog.getByLabel('Import asset files', { exact: true }).setInputFiles({ name: 'unsafe.svg', mimeType: 'image/svg+xml', buffer: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>') });
  await expect(dialog.getByRole('alert')).toContainText(/unsupported|inert/);
  await expect(page.locator('.slide-stage img')).toHaveCount(2);
});

test('graph context menus duplicate and remove the clicked node inside the dialog', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
  await expect(dialog.locator('.graph-fitted-label')).toHaveCount(3);
  await dialog.locator('.react-flow__node[data-id="user"]').click({ button: 'right' });
  await dialog.getByRole('menuitem', { name: 'Duplicate node', exact: true }).click();
  await expect(dialog.locator('.react-flow__node')).toHaveCount(4);
  await dialog.locator('.react-flow__node[data-id="data"]').click({ button: 'right' });
  await dialog.getByRole('menuitem', { name: 'Delete selection', exact: true }).click();
  await expect(dialog.locator('.react-flow__node')).toHaveCount(3);
  await dialog.getByRole('button', { name: 'Undo diagram edit', exact: true }).click();
  await expect(dialog.locator('.react-flow__node')).toHaveCount(4);
  await expect(dialog.getByRole('alert')).toHaveCount(0);
});

test('asset drops are atomic and keyboard file commands preserve unsaved work', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Insert icons', exact: true })).toBeEnabled();
  const canvas = page.locator('.canvas-workspace');
  const valid = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2 2h20v20H2z" fill="#0017c1"/></svg>';
  const invalid = '<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.com/private.png"/></svg>';
  await canvas.evaluate((element, inputs) => {
    const dataTransfer = new DataTransfer();
    inputs.forEach((svg, index) => dataTransfer.items.add(new File([svg], `${index}.svg`, { type: 'image/svg+xml' })));
    element.dispatchEvent(new DragEvent('drop', { dataTransfer, bubbles: true, cancelable: true, clientX: 400, clientY: 300 }));
  }, [valid, invalid]);
  await expect(page.getByRole('alert')).toContainText(/unsupported|inert/);
  await expect(page.locator('.slide-stage img')).toHaveCount(0);
  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click();
  await canvas.evaluate((element, svg) => {
    const dataTransfer = new DataTransfer(); dataTransfer.items.add(new File([svg], 'valid.svg', { type: 'image/svg+xml' }));
    element.dispatchEvent(new DragEvent('drop', { dataTransfer, bubbles: true, cancelable: true, clientX: 400, clientY: 300 }));
  }, valid);
  await expect(page.locator('.slide-stage img')).toHaveCount(1);
  await page.keyboard.press('Control+m');
  await expect(page.locator('.thumbnail')).toHaveCount(13);
  await page.getByRole('button', { name: 'New report', exact: true }).click();
  await page.getByRole('dialog', { name: 'Unsaved changes', exact: true }).getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(13);
  await page.keyboard.press('Control+s');
  await expect(page.locator('.dirty-indicator')).toHaveCount(0);
});

for (const width of [1440, 1200, 390]) {
  test(`context menu and asset controls fit and are labeled at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 1000 });
    await page.goto('/');
    await expect(page.getByRole('button', { name: 'File operations', exact: true })).toBeEnabled();
    await expect(page.getByRole('button', { name: 'Rename presentation', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Add rectangle', exact: true }).click();
    await expect(page.locator('.dirty-indicator')).toBeVisible();
    await page.locator('.canvas-workspace').evaluate((element, horizontal) => element.dispatchEvent(new MouseEvent('contextmenu', { clientX: horizontal - 4, clientY: 996, bubbles: true, cancelable: true })), width);
    const menu = page.getByRole('menu');
    await expect(menu).toBeVisible();
    const bounds = await menu.boundingBox();
    expect(bounds).not.toBeNull();
    expect(bounds!.x).toBeGreaterThanOrEqual(0);
    expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(width);
    expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(1000);
    const menuA11y = await new AxeBuilder({ page }).include('.context-menu').disableRules(['color-contrast']).analyze();
    expect(menuA11y.violations.filter((entry) => ['serious', 'critical'].includes(entry.impact ?? ''))).toEqual([]);
    await page.screenshot({ path: `.artifacts/workspace-menu-${width}.png` });
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByLabel('Search icons', { exact: true })).toBeVisible();
    const assetA11y = await new AxeBuilder({ page }).include('.asset-panel').disableRules(['color-contrast']).analyze();
    expect(assetA11y.violations.filter((entry) => ['serious', 'critical'].includes(entry.impact ?? ''))).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: `.artifacts/workspace-assets-${width}.png`, fullPage: true });
  });
}
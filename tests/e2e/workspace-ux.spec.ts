import { test, expect } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import AxeBuilder from '@axe-core/playwright';
import { icons as lucideIcons } from 'lucide-react';
import { openSample } from './fixtures';

test('startup opens a clean blank presentation and the sample is explicit', async ({ page }) => {
  const operations: string[] = [];
  page.on('request', (request) => { if (request.url().endsWith('/api/core') && request.method() === 'POST') operations.push(request.postDataJSON().op); });
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  await expect(page.locator('.thumbnail')).toHaveCount(1);
  await expect(page.locator('.slide-stage .element-hitbox')).toHaveCount(0);
  await expect(page.locator('.document-name strong')).toHaveText('Untitled presentation');
  await expect(page.locator('.dirty-indicator')).toHaveCount(0);
  expect(operations).toContain('create_presentation');
  expect(operations).not.toContain('sample');
  await page.getByRole('button', { name: 'New report', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(12);
  expect(operations).toContain('sample');
  await expect(page.getByRole('alert')).toHaveCount(0);
});

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
    await openSample(page);
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
  await openSample(page);
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

test('master presets preview spacing and native layouts while retaining content and undo', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto('/');
  await page.getByRole('button', { name: 'Add text', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('New text', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Masters and layouts', exact: true });
  await dialog.getByRole('button', { name: 'Browse presets', exact: true }).click();
  await expect(dialog.locator('.master-preset-choice')).toHaveCount(7);
  await dialog.getByRole('button', { name: 'Preset Minimal space', exact: true }).click();
  await expect(dialog.getByLabel('Preset margin', { exact: true })).toHaveText('96 px');
  await expect(dialog.getByLabel('Preset gutter', { exact: true })).toHaveText('56 px');
  await expect(dialog.getByLabel('Preset heading font', { exact: true })).toHaveText('Noto Sans JP');
  await dialog.getByLabel('Preset preview layout', { exact: true }).selectOption('preset-two-columns');
  await expect(dialog.locator('.preset-preview .slide-text')).toHaveCount(3);
  await dialog.getByRole('button', { name: 'Use preset', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('.slide-stage').getByText('New text', { exact: true })).toBeVisible();
  await expect(page.getByLabel('Slide layout', { exact: true })).toHaveValue('preset-blank');
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.getByLabel('Slide layout', { exact: true })).toHaveValue('blank');
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await page.getByLabel('Slide layout', { exact: true }).selectOption('preset-two-columns');
  await expect(page.locator('.slide-stage').getByText('New text', { exact: true })).toBeVisible();
  await expect(page.locator('.slide-stage').getByText('Left content', { exact: true })).toBeVisible();
  const pending = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const file = await pending;
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: await readFile((await file.path())!) });
  await expect(page.getByLabel('Slide layout', { exact: true })).toHaveValue('preset-two-columns');
  await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click();
  await dialog.getByRole('button', { name: 'Browse presets', exact: true }).click();
  await dialog.getByRole('button', { name: 'Preset Trusted report', exact: true }).click();
  await dialog.getByRole('button', { name: 'Use preset', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator('.slide-stage').getByText('New text', { exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('all seven master presets have fitted native previews and accessible controls', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto('/');
  await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Masters and layouts', exact: true });
  await dialog.getByRole('button', { name: 'Browse presets', exact: true }).click();
  await expect(dialog.locator('.master-preset-choice')).toHaveCount(7);
  const names = await dialog.locator('.master-preset-choice').evaluateAll((buttons) => buttons.map((button) => button.getAttribute('aria-label')!));
  for (const name of names) {
    await dialog.getByRole('button', { name, exact: true }).click();
    if (name === 'Preset Studio editorial') {
      await page.evaluate(() => document.fonts.ready);
      expect(await page.evaluate(() => [...document.fonts].some((font) => font.family.replaceAll('"', '') === 'IBM Plex Sans' && font.status === 'loaded'))).toBe(true);
    }
    const layouts = await dialog.getByLabel('Preset preview layout', { exact: true }).locator('option').evaluateAll((options) => options.map((option) => option.value));
    expect(layouts).toHaveLength(7);
    for (const layout of layouts) {
      await dialog.getByLabel('Preset preview layout', { exact: true }).selectOption(layout);
      await page.evaluate(() => document.fonts.ready);
      const overflow = await dialog.locator('.preset-preview .slide-text').evaluateAll((elements) => elements.filter((element) => element.scrollWidth > element.clientWidth + 1 || element.scrollHeight > element.clientHeight + 1).map((element) => element.textContent));
      expect(overflow, `${name}: ${layout}`).toEqual([]);
    }
    await dialog.getByLabel('Preset preview layout', { exact: true }).selectOption('preset-cover');
    await dialog.locator('.preset-preview').screenshot({ path: `.artifacts/preset-${name.replaceAll(' ', '-')}.png` });
  }
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 1000 });
    const results = await new AxeBuilder({ page }).include('.preset-browser').analyze();
    expect(results.violations.filter((issue) => ['serious', 'critical'].includes(issue.impact ?? ''))).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const apply = dialog.getByRole('button', { name: 'Use preset', exact: true });
    await apply.scrollIntoViewIfNeeded();
    await expect(apply).toBeInViewport({ ratio: 1 });
    await dialog.screenshot({ path: `.artifacts/preset-controls-${width}.png` });
  }
});

test('icon categories intersect search and reset pages in both pickers', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Icons and assets', exact: true });
  await dialog.getByRole('button', { name: 'Next icon page', exact: true }).click();
  await expect(dialog.getByRole('navigation', { name: 'Icon pages', exact: true })).toContainText('61-120');
  await dialog.getByLabel('Icon category', { exact: true }).selectOption('development');
  await expect(dialog.getByRole('button', { name: 'Previous icon page', exact: true })).toBeDisabled();
  await dialog.getByLabel('Search icons', { exact: true }).fill('database');
  await expect(dialog.getByRole('button', { name: 'Database icon', exact: true })).toBeVisible();
  await dialog.getByLabel('Icon category', { exact: true }).selectOption('devices');
  await expect(dialog.getByRole('button', { name: 'Database icon', exact: true })).toBeVisible();
  await dialog.getByLabel('Icon category', { exact: true }).selectOption('weather');
  await expect(dialog.locator('.icon-grid button')).toHaveCount(0);
  await expect(dialog.getByText('No matching icons', { exact: true })).toBeVisible();
  await dialog.getByLabel('Search icons', { exact: true }).fill('');
  await expect(dialog.getByRole('button', { name: 'Cloud icon', exact: true })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Database icon', exact: true })).toHaveCount(0);
  await dialog.getByLabel('Icon category', { exact: true }).selectOption('all');
  await expect(dialog.getByRole('status')).toHaveText(`${Object.keys(lucideIcons).length} icons`);
  await dialog.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
  const graph = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
  await graph.getByRole('button', { name: 'Select node data', exact: true }).click();
  await graph.getByRole('button', { name: 'Choose node icon', exact: true }).click();
  const picker = graph.getByRole('region', { name: 'Node icon', exact: true });
  await picker.getByLabel('Icon category', { exact: true }).selectOption('development');
  await picker.getByLabel('Search icons', { exact: true }).fill('database');
  await picker.getByRole('button', { name: 'Database icon', exact: true }).click();
  await picker.getByRole('button', { name: 'Insert icon', exact: true }).click();
  await expect(graph.locator('.graph-node-icon img')).toHaveAttribute('alt', 'Database (Lucide)');
});

test('complete Lucide library pages through every icon and reopens inserted pictures', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto('/');
  await page.getByRole('button', { name: 'New presentation', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(1);
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Icons and assets', exact: true });
  const choices = dialog.locator('.icon-grid button');
  const expected = Object.keys(lucideIcons);
  await expect(choices).toHaveCount(60);
  await expect(dialog.getByRole('status')).toHaveText(`${expected.length} icons`);
  const previous = dialog.getByRole('button', { name: 'Previous icon page', exact: true });
  const next = dialog.getByRole('button', { name: 'Next icon page', exact: true });
  await expect(previous).toBeDisabled();
  await page.evaluate(() => document.fonts.ready);
  const entries: { id: string | null; name: string | null; rendered: boolean; fits: boolean }[] = [];
  for (let offset = 0; offset < expected.length; offset += 60) {
    const end = Math.min(offset + 60, expected.length);
    await expect(dialog.getByRole('navigation', { name: 'Icon pages', exact: true })).toContainText(`${offset + 1}-${end} / ${expected.length}`);
    await expect(choices).toHaveCount(end - offset);
    entries.push(...await choices.evaluateAll((buttons) => buttons.map((button) => {
      const symbol = button.querySelector('svg')?.getBBox();
      const bounds = button.getBoundingClientRect();
      const label = button.querySelector('span')!.getBoundingClientRect();
      return { id: button.getAttribute('data-icon-name'), name: button.getAttribute('aria-label'), rendered: Boolean(symbol && (symbol.width > 0 || symbol.height > 0)), fits: label.left >= bounds.left && label.right <= bounds.right && label.top >= bounds.top && label.bottom <= bounds.bottom };
    })));
    if (end < expected.length) await next.click();
  }
  await expect(next).toBeDisabled();
  expect(entries.map((entry) => entry.id).sort()).toEqual(expected.sort());
  expect(entries.filter((entry) => !entry.rendered)).toEqual([]);
  expect(entries.filter((entry) => !entry.fits)).toEqual([]);
  const names = entries.map((entry) => entry.name);
  expect(entries.filter((entry, index) => entries.findIndex((other) => other.name === entry.name) !== index)).toEqual([]);
  const retained = ['Database', 'Server', 'Cloud', 'Globe', 'Router', 'Monitor', 'Smartphone', 'Folder', 'Document', 'Mail', 'User', 'Users', 'Shield', 'Lock', 'Key', 'Check', 'Alert', 'Search', 'Settings', 'Workflow', 'Network', 'Branch', 'Arrow', 'Exchange', 'Calendar', 'Clock', 'Chart', 'Building', 'Briefcase', 'Layers', 'Link', 'Message', 'Upload', 'Download', 'Drive', 'Bell'];
  expect(names).toEqual(expect.arrayContaining(retained.map((name) => `${name} icon`)));
  await previous.click();
  await expect(next).toBeEnabled();
  for (const [query, name] of [['  ロボット  ', 'Bot'], ['coNTaINer', 'Container'], ['決済', 'Credit card'], ['配送', 'Truck'], ['教育', 'Graduation cap'], ['環境', 'Leaf'], ['database', 'Database'], ['zodiac-virgo', 'Zodiac Virgo']]) {
    await dialog.getByLabel('Search icons', { exact: true }).fill(query);
    await expect(dialog.getByRole('button', { name: `${name} icon`, exact: true })).toBeVisible();
    await expect(previous).toBeDisabled();
  }
  await dialog.getByLabel('Search icons', { exact: true }).fill('no-icon-with-this-name');
  await expect(choices).toHaveCount(0);
  await expect(dialog.getByText('No matching icons', { exact: true })).toBeVisible();
  await expect(previous).toBeDisabled();
  await expect(next).toBeDisabled();
  await dialog.getByLabel('Search icons', { exact: true }).fill('zodiac virgo');
  const choice = dialog.getByRole('button', { name: 'Zodiac Virgo icon', exact: true });
  await choice.focus();
  await choice.press('Enter');
  await expect(choice).toHaveAttribute('aria-pressed', 'true');
  await dialog.getByLabel('Icon color', { exact: true }).fill('#007a4d');
  await dialog.getByLabel('Icon stroke width', { exact: true }).fill('1.5');
  await dialog.getByLabel('Asset size', { exact: true }).fill('80');
  await dialog.getByRole('button', { name: 'Insert icon', exact: true }).click();
  const image = page.locator('.slide-stage img');
  await expect(image).toHaveCount(1);
  await expect(image).toHaveAttribute('alt', 'Zodiac Virgo (Lucide)');
  await expect(image).toHaveJSProperty('complete', true);
  expect(await image.evaluate((element) => (element as HTMLImageElement).naturalWidth)).toBeGreaterThan(0);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(image).toHaveCount(0);
  await page.getByRole('button', { name: 'Redo', exact: true }).click();
  await expect(image).toHaveCount(1);
  const pending = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const file = await pending;
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: await readFile((await file.path())!) });
  await expect(image).toHaveAttribute('alt', 'Zodiac Virgo (Lucide)');
  await expect(image).toHaveAttribute('src', /^data:image\/png;base64,/);
  await expect(image).toHaveJSProperty('complete', true);
  expect(await image.evaluate((element) => (element as HTMLImageElement).naturalWidth)).toBeGreaterThan(0);
  await expect(page.getByRole('alert')).toHaveCount(0);
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
  await openSample(page);
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
    await expect(page.locator('.icon-grid button')).toHaveCount(60);
    await expect(page.getByRole('dialog').getByRole('status')).toHaveText(`${Object.keys(lucideIcons).length} icons`);
    await page.evaluate(() => document.fonts.ready);
    const invalidIcons = await page.locator('.icon-grid button').evaluateAll((buttons) => buttons.filter((button) => {
      const bounds = button.getBoundingClientRect();
      const label = button.querySelector('span')!.getBoundingClientRect();
      const symbol = button.querySelector('svg')!.getBBox();
      return label.left < bounds.left || label.right > bounds.right || label.top < bounds.top || label.bottom > bounds.bottom || symbol.width <= 0 || symbol.height <= 0;
    }).map((button) => button.getAttribute('aria-label')));
    expect(invalidIcons).toEqual([]);
    const assetA11y = await new AxeBuilder({ page }).include('.asset-panel').disableRules(['color-contrast']).analyze();
    expect(assetA11y.violations.filter((entry) => ['serious', 'critical'].includes(entry.impact ?? ''))).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.getByRole('button', { name: 'Next icon page', exact: true }).click();
    await expect(page.getByRole('navigation', { name: 'Icon pages', exact: true })).toContainText('61-120');
    await page.locator('.icon-grid button').last().scrollIntoViewIfNeeded();
    await expect(page.locator('.icon-grid button').last()).toBeVisible();
    await page.screenshot({ path: `.artifacts/workspace-assets-${width}.png`, fullPage: true });
    const longestName = Object.keys(lucideIcons).reduce((longest, name) => name.length > longest.length ? name : longest, '');
    await page.getByLabel('Search icons', { exact: true }).fill(longestName);
    const longest = page.locator(`.icon-grid button[data-icon-name="${longestName}"]`);
    await expect(longest).toBeVisible();
    expect(await longest.evaluate((button) => {
      const bounds = button.getBoundingClientRect();
      const label = button.querySelector('span')!.getBoundingClientRect();
      return label.left >= bounds.left && label.right <= bounds.right && label.top >= bounds.top && label.bottom <= bounds.bottom;
    })).toBe(true);
    await longest.focus();
    await longest.press('Enter');
    await expect(longest).toHaveAttribute('aria-pressed', 'true');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    const insert = page.getByRole('dialog').getByRole('button', { name: 'Insert icon', exact: true });
    await insert.scrollIntoViewIfNeeded();
    await expect(insert).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: `.artifacts/lucide-long-name-${width}.png`, fullPage: true });
    await insert.click();
    await expect(page.locator('.slide-stage img')).toHaveCount(1);
    await expect(page.locator('.slide-stage img')).toHaveJSProperty('complete', true);
    await expect(page.getByRole('alert')).toHaveCount(0);
  });
}
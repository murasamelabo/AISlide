import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { readFile } from 'node:fs/promises';
import { unzipSync } from 'fflate';

test('PowerPoint content export is explicit and downloads separate playback and manifest files', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/');
  await expect(page.locator('.thumbnail')).toHaveCount(1, { timeout: 20_000 });
  const exportButton = page.getByRole('button', { name: 'PowerPoint interactive export', exact: true });
  await expect(exportButton).toBeDisabled();
  await page.getByRole('button', { name: 'Interactive part', exact: true }).click();
  const editor = page.getByRole('dialog', { name: 'Interactive part', exact: true });
  await expect(editor.getByRole('status')).toHaveText('Selected: Identity');
  await editor.getByRole('button', { name: 'Save default and insert part', exact: true }).click();
  await expect(editor).toHaveCount(0);
  await exportButton.click();
  const dialog = page.getByRole('dialog', { name: 'PowerPoint interactive export', exact: true });
  const prepare = dialog.getByRole('button', { name: 'Prepare PowerPoint interactive export', exact: true });
  await expect(prepare).toBeDisabled();
  await expect(dialog.getByLabel('PowerPoint catalog UNC path', { exact: true })).toHaveAccessibleDescription(/server\/share names.*stored in the PPTX.*visible to recipients/);
  await dialog.getByLabel('PowerPoint catalog UNC path', { exact: true }).fill('C:\\not-a-catalog');
  await prepare.click();
  await expect(dialog.getByRole('alert')).toContainText('UNC path');
  await dialog.getByLabel('PowerPoint catalog UNC path', { exact: true }).fill('\\\\localhost\\AISlideAddins');
  await prepare.click();
  await expect(dialog.getByRole('status')).toContainText('Prepared 1 content add-ins');
  const pending = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Download interactive PPTX', exact: true }).click();
  const download = await pending;
  const path = await download.path();
  if (!path) throw new Error('No playback download');
  const files = unzipSync(await readFile(path));
  expect(files['ppt/webextensions/webextension1.xml']).toBeDefined();
  const pendingManifest = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Download add-in manifest', exact: true }).click();
  const manifest = await pendingManifest;
  expect(manifest.suggestedFilename()).toBe('aislide.addin.xml');
  await dialog.getByLabel('PowerPoint playback target', { exact: true }).selectOption('copilot');
  await expect(dialog.getByLabel('PowerPoint runtime URL', { exact: true })).toHaveCount(0);
  await expect(dialog.getByLabel('PowerPoint catalog UNC path', { exact: true })).toHaveCount(0);
  await expect(prepare).toBeDisabled();
  await dialog.getByRole('checkbox', { name: 'Acknowledge experimental Copilot catalog', exact: true }).check();
  await prepare.click();
  await expect(dialog.getByRole('status')).toContainText('Prepared 1 content add-ins');
  await expect(dialog.getByRole('button', { name: 'Download add-in manifest', exact: true })).toHaveCount(0);
  const pendingExperiment = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Download interactive PPTX', exact: true }).click();
  const experiment = await pendingExperiment;
  expect(experiment.suggestedFilename()).toBe('interactive-copilot-experimental.pptx');
  const experimentPath = await experiment.path();
  if (!experimentPath) throw new Error('No Copilot experiment download');
  const extension = new TextDecoder().decode(unzipSync(await readFile(experimentPath))['ppt/webextensions/webextension1.xml']);
  expect(extension).toContain('id="fa000000210"');
  expect(extension).toContain('storeType="sdxcatalog"');
  expect(extension).toContain('name="contentBlockHtml"');
  await dialog.getByLabel('PowerPoint playback target', { exact: true }).selectOption('aislide');
  await expect(dialog.getByLabel('PowerPoint catalog UNC path', { exact: true })).toHaveValue('\\\\localhost\\AISlideAddins');
  await expect(dialog.getByRole('button', { name: 'Download interactive PPTX', exact: true })).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Close dialog', exact: true }).click();
  const staticPending = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const staticFile = await staticPending;
  const staticPath = await staticFile.path();
  if (!staticPath) throw new Error('No static source download');
  expect(unzipSync(await readFile(staticPath))['ppt/webextensions/webextension1.xml']).toBeUndefined();
});

test('pending previews prevent conflicting document actions and reattach after JSON validation', async ({ page }) => {
  test.setTimeout(120_000);
  let release = () => {};
  const ready = new Promise<void>(resolve => { release = resolve; });
  let blocking = true;
  await page.route('**/api/core', async route => {
    if (blocking && route.request().postDataJSON().op === 'preview_interactive') await ready;
    await route.continue();
  });
  try {
    await page.goto('/');
    await expect(page.locator('.thumbnail')).toHaveCount(1, { timeout: 20_000 });
    await page.getByRole('button', { name: 'Interactive part', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Interactive part', exact: true });
    await expect(dialog.getByRole('button', { name: 'Close dialog', exact: true })).toBeDisabled();
    await page.keyboard.press('Escape');
    await expect(dialog).toBeVisible();
    blocking = false;
    release();
    await expect(dialog.getByRole('status')).toHaveText('Selected: Identity');
    await dialog.getByRole('button', { name: 'Edit interactive data', exact: true }).click();
    const input = dialog.getByLabel('Interactive data JSON', { exact: true });
    const spec = JSON.parse(await input.inputValue());
    spec.title = 'Revalidated preview';
    await input.fill(JSON.stringify(spec));
    await dialog.getByRole('button', { name: 'Validate data', exact: true }).click();
    await expect(dialog.getByRole('status')).toHaveText('Selected: Identity');
    await page.setViewportSize({ width: 390, height: 1000 });
    await expect.poll(() => dialog.locator('.interactive-preview').evaluate(node => Math.abs((node.firstElementChild?.getBoundingClientRect().width ?? 0) - node.clientWidth))).toBeLessThan(1);
    await dialog.getByRole('button', { name: 'Close dialog', exact: true }).click();
    await page.getByRole('button', { name: 'Add text', exact: true }).click();
    await expect(page.locator('.slide-stage .element-hitbox')).toHaveCount(1);
  } finally { release(); }
});

test('interactive dashboard explores transiently, saves defaults, reopens and undoes', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/');
  await expect(page.locator('.thumbnail')).toHaveCount(1, { timeout: 20_000 });
  await page.getByRole('button', { name: 'Interactive part', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Interactive part', exact: true });
  await dialog.getByLabel('Interactive template', { exact: true }).selectOption('interactive/dashboard');
  await expect(dialog.getByRole('status')).toHaveText('Selected: Identity');
  await dialog.getByLabel('Interactive scale', { exact: true }).selectOption('logarithmic');
  await dialog.getByLabel('Interactive group', { exact: true }).selectOption('after');
  await expect(dialog.getByRole('status')).toHaveText('Selected: Resource');
  await expect(page.locator('.slide-stage').getByText('Resource', { exact: true })).toHaveCount(0);
  await dialog.getByRole('button', { name: 'Save default and insert part', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole('button', { name: 'Edit part data', exact: true }).click();
  await expect(dialog.getByRole('status')).toHaveText('Selected: Resource');
  await expect(dialog.getByLabel('Interactive scale', { exact: true })).toHaveValue('logarithmic');
  await dialog.getByLabel('Interactive group', { exact: true }).selectOption('');
  await dialog.getByRole('button', { name: 'Recovery', exact: true }).click();
  await expect(dialog.getByRole('status')).toHaveText('Selected: Recovery');
  await dialog.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Edit part data', exact: true }).click();
  await expect(dialog.getByRole('status')).toHaveText('Selected: Resource');
  await dialog.getByRole('button', { name: 'Close dialog', exact: true }).click();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const saved = await download;
  const file = await saved.path();
  if (!file) throw new Error('No downloaded PPTX');
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: saved.suggestedFilename(), mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: await (await import('node:fs/promises')).readFile(file) });
  const details = page.getByRole('dialog', { name: 'PPTX details' });
  if (await details.isVisible()) await details.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.locator('.slide-stage .element-hitbox').first().click();
  await page.getByRole('button', { name: 'Edit part data', exact: true }).click();
  await expect(dialog.getByRole('status')).toHaveText('Selected: Resource');
  await dialog.getByLabel('Interactive group', { exact: true }).selectOption('');
  await dialog.getByRole('button', { name: 'Recovery', exact: true }).click();
  await expect(dialog.getByRole('status')).toHaveText('Selected: Recovery');
  await dialog.getByRole('button', { name: 'Save default and update part', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await page.locator('.slide-stage .element-hitbox').first().click();
  await page.getByRole('button', { name: 'Edit part data', exact: true }).click();
  await expect(dialog.getByRole('status')).toHaveText('Selected: Resource');
});

for (const width of [1440, 390]) {
  test(`all interactive types are keyboard-operable and responsive at ${width}`, async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width, height: 1000 });
    await page.goto('/');
    await expect(page.locator('.thumbnail')).toHaveCount(1, { timeout: 20_000 });
    await page.getByRole('button', { name: 'Interactive part', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Interactive part', exact: true });
    for (const template of ['summary', 'dashboard', 'timeline', 'comparison', 'map']) {
      await dialog.getByLabel('Interactive template', { exact: true }).selectOption(`interactive/${template}`);
      await expect(dialog.getByRole('status')).toHaveText('Selected: Identity');
      const resource = dialog.getByRole('button', { name: template === 'timeline' ? '2. Resource' : 'Resource', exact: true });
      await resource.focus();
      await page.keyboard.press('Enter');
      await expect(dialog.getByRole('status')).toHaveText('Selected: Resource');
      if (template === 'timeline') {
        await dialog.getByRole('button', { name: 'Next stage', exact: true }).click();
        await expect(dialog.getByRole('status')).toHaveText('Selected: Recovery');
      }
      if (template === 'comparison') {
        await dialog.getByLabel('Interactive dimension', { exact: true }).selectOption('1');
        await expect(dialog.getByRole('status')).toHaveText('Selected: Resource');
      }
      await expect(dialog.getByRole('alert')).toHaveCount(0);
      await dialog.screenshot({ path: `.artifacts/interactive-prototype/${template}-${width}.png` });
    }
    const violations = await new AxeBuilder({ page }).include('.interactive-panel').analyze();
    expect(violations.violations.filter(item => ['critical', 'serious'].includes(item.impact ?? ''))).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await dialog.getByRole('button', { name: 'Edit interactive data', exact: true }).click();
    const original = await dialog.getByLabel('Interactive data JSON', { exact: true }).inputValue();
    const invalid = JSON.parse(original);
    invalid.data.spec.html = '<script>globalThis.stolen = true</script>';
    await dialog.getByLabel('Interactive data JSON', { exact: true }).fill(JSON.stringify(invalid));
    await dialog.getByRole('button', { name: 'Validate data', exact: true }).click();
    await expect(dialog.getByRole('alert')).toBeVisible();
    await expect(dialog.getByLabel('Interactive data JSON', { exact: true })).toHaveValue(JSON.stringify(invalid));
    await expect(page.locator('.slide-stage').getByText('Resource', { exact: true })).toHaveCount(0);
  });
}

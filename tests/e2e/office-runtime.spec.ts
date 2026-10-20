import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { unzipSync } from 'fflate';

test('self-contained experimental Copilot HTML executes all five generated views with no host SDK or network', async ({ page }) => {
  const directory = process.env.AISLIDE_COPILOT_TEST_DIR ?? resolve('.artifacts', 'copilot-interactive');
  const file = resolve(directory, 'interactive-copilot-experimental.pptx');
  const parts = unzipSync(await readFile(file));
  const requests: string[] = [];
  page.on('request', request => requests.push(request.url()));
  for (let index = 1; index <= 5; index++) {
    const xml = new TextDecoder().decode(parts[`ppt/webextensions/webextension${index}.xml`]);
    const html = await page.evaluate(xml => {
      const document = new DOMParser().parseFromString(xml, 'application/xml');
      const node = document.getElementsByTagNameNS('http://schemas.microsoft.com/office/webextensions/webextension/2010/11', 'property')[0];
      if (node?.getAttribute('name') !== 'contentBlockHtml') throw new Error('Wrong Copilot property');
      const value = node.getAttribute('value');
      if (!value) throw new Error('Empty HTML');
      return JSON.parse(value) as string;
    }, xml);
    await page.goto('about:blank');
    await page.setViewportSize({ width: 1324, height: 746 });
    await page.setContent(html);
    await page.addStyleTag({ content: 'body { margin: 140px auto 0; max-width: 440px; } main { width: 440px; max-width: 440px; height: 540px; }' });
    await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
    const drawing = page.locator('.aislide-interactive__drawing');
    await expect.poll(() => drawing.evaluate(node => node.getBoundingClientRect().width)).toBeGreaterThan(1000);
    await page.getByText('Readable view text', { exact: true }).focus();
    await page.keyboard.press('Tab');
    await expect(page.getByRole('combobox', { name: 'Group', exact: true })).toBeFocused();
    await page.getByRole('button', { name: 'Resource', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Resource — selected', exact: true })).toBeVisible();
    await expect(page.locator('.aislide-interactive__text').filter({ hasText: 'Review Resource with its owner' })).toBeInViewport();
    await page.setViewportSize({ width: 640, height: 300 });
    await expect.poll(() => drawing.evaluate(node => {
      const rect = node.getBoundingClientRect();
      return rect.left >= -1 && rect.top >= -1 && rect.right <= innerWidth + 1 && rect.bottom <= innerHeight + 1;
    })).toBe(true);
    await page.getByRole('button', { name: 'Resource — selected', exact: true }).focus();
    await page.keyboard.press('Tab');
    await page.keyboard.press('Shift+Tab');
    await expect(page.getByRole('button', { name: 'Resource — selected', exact: true })).toBeFocused();
    await page.getByRole('button', { name: 'Reset', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
    expect(await page.evaluate(() => Reflect.get(globalThis, 'Office'))).toBeUndefined();
    await page.setViewportSize({ width: 1324, height: 746 });
    await page.screenshot({ path: resolve(directory, `self-contained-${index}.png`) });
  }
  expect(requests).toEqual([]);
});

async function fixture(template: string) {
  return JSON.parse(await readFile(resolve('.artifacts', 'office-interactive', `${template}.json`), 'utf8'));
}

test('all playback types fill a constrained host without becoming a clipped sidebar preview', async ({ page }) => {
  for (const template of ['summary', 'dashboard', 'timeline', 'comparison', 'map']) {
    const payload = await fixture(template);
    await page.route('**/preview.json', route => route.fulfill({ json: payload }));
    await page.setViewportSize({ width: 1324, height: 746 });
    await page.goto('/preview.html');
    await page.addStyleTag({ content: 'body { margin: 140px auto 0; max-width: 440px; } main { width: 440px; max-width: 440px; height: 540px; }' });
    await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
    const drawing = page.locator('.aislide-interactive__drawing');
    await expect.poll(() => drawing.evaluate(node => node.getBoundingClientRect().width)).toBeGreaterThan(1000);
    for (const viewport of [{ width: 1324, height: 746 }, { width: 864, height: 408 }, { width: 640, height: 300 }, { width: 390, height: 900 }]) {
      await page.setViewportSize(viewport);
      await expect.poll(() => drawing.evaluate(node => {
        const rect = node.getBoundingClientRect();
        return rect.left >= -1 && rect.top >= -1 && rect.right <= innerWidth + 1 && rect.bottom <= innerHeight + 1;
      })).toBe(true);
      await expect(page.getByRole('button', { name: 'Resource', exact: true })).toBeInViewport();
      await page.getByRole('button', { name: 'Resource', exact: true }).click();
      await expect(page.getByRole('button', { name: 'Resource — selected', exact: true })).toBeInViewport();
      await expect(page.locator('.aislide-interactive__text').filter({ hasText: 'Review Resource with its owner' })).toBeInViewport();
      await page.getByRole('button', { name: 'Reset', exact: true }).click();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight)).toBe(true);
    }
    await page.setViewportSize({ width: 1324, height: 746 });
    await page.screenshot({ path: `.artifacts/playback-layout/${template}-fixed.png` });
    await page.getByText('Readable view text', { exact: true }).click();
    await expect(page.locator('.aislide-interactive__transcript p').filter({ hasText: 'Review Identity with its owner' })).toBeVisible();
    await page.getByText('Readable view text', { exact: true }).click();
    await expect.poll(() => drawing.evaluate(node => node.getBoundingClientRect().width)).toBeGreaterThan(1000);
    await page.unroute('**/preview.json');
  }
});

for (const template of ['summary', 'dashboard', 'timeline', 'comparison', 'map']) {
  test(`real core ${template} payload renders and changes selection without external requests`, async ({ page }) => {
    const payload = await fixture(template);
    const requests: string[] = [];
    page.on('request', request => requests.push(request.url()));
    await page.route('**/preview.json', route => route.fulfill({ json: payload }));
    await page.goto('/preview.html');
    await expect(page.getByRole('alert')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toHaveAttribute('aria-pressed', 'true');
    await page.getByRole('button', { name: 'Resource', exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('button', { name: 'Resource — selected', exact: true })).toHaveAttribute('aria-pressed', 'true');
    if (template === 'dashboard') {
      await page.getByRole('combobox', { name: 'Scale', exact: true }).selectOption('logarithmic');
      await expect(page.locator('.aislide-interactive__text').filter({ hasText: 'log(1 + value)' })).toBeVisible();
    }
    if (template === 'timeline') {
      await page.getByRole('button', { name: 'Next', exact: true }).click();
      await expect(page.getByRole('button', { name: 'Recovery — selected', exact: true })).toBeVisible();
    }
    if (template === 'comparison') await page.getByRole('combobox', { name: 'Dimension', exact: true }).selectOption('1');
    await page.getByRole('combobox', { name: 'Group', exact: true }).selectOption('after');
    await expect(page.getByRole('button', { name: 'Identity', exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: 'Reset', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
    await page.screenshot({ path: `.artifacts/office-interactive/${template}-html.png`, fullPage: true });
    expect(requests.every(url => url.startsWith('http://127.0.0.1:4198/'))).toBe(true);
    const audit = await new AxeBuilder({ page }).analyze();
    expect(audit.violations.filter(item => ['serious', 'critical'].includes(item.impact ?? ''))).toEqual([]);
  });
}

test('Office settings bridge mounts the actual compiled payload and never saves transient state', async ({ page }) => {
  const payload = await fixture('dashboard');
  await page.route('https://appsforoffice.microsoft.com/**', route => route.fulfill({ contentType: 'application/javascript', body: `
    globalThis.officeWrites = 0;
    globalThis.Office = {
      HostType: { PowerPoint: 'PowerPoint' },
      onReady: () => Promise.resolve({host: 'PowerPoint'}),
      context: {document: {settings: {
        get: name => name === 'aislideInteractive' ? ${JSON.stringify(payload).replace(/</g, '\\u003c')} : undefined,
        set: () => { globalThis.officeWrites++; throw Error('Unexpected write'); },
        saveAsync: () => { globalThis.officeWrites++; throw Error('Unexpected save'); }
      }}}
    };` }));
  await page.goto('/office.html');
  await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Resource', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Resource — selected', exact: true })).toBeVisible();
  expect(await page.evaluate(() => Reflect.get(globalThis, 'officeWrites'))).toBe(0);
  await page.reload();
  await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
});

test('host and unsafe payload errors are explicit without executing text', async ({ page }) => {
  await page.route('https://appsforoffice.microsoft.com/**', route => route.fulfill({ contentType: 'application/javascript', body: 'globalThis.Office={HostType:{PowerPoint:"PowerPoint"},onReady:()=>Promise.resolve({host:"Word"})};' }));
  await page.goto('/office.html');
  await expect(page.getByRole('alert')).toContainText('requires PowerPoint');
  const payload = await fixture('summary');
  for (const view of payload.views) {
    const text = view.element.children.find((item: { type: string }) => item.type === 'text');
    text.text = '<img src=x onerror="globalThis.injected=true"><script>globalThis.injected=true</script>';
  }
  await page.route('**/preview.json', route => route.fulfill({ json: payload }));
  await page.goto('/preview.html');
  await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
  expect(await page.evaluate(() => Reflect.get(globalThis, 'injected'))).toBeUndefined();
  expect(await page.locator('#interactive-root img, #interactive-root script').count()).toBe(0);
  const bad = await fixture('summary');
  bad.views[0].element.children.push({ type: 'html', html: '<script>globalThis.injected=true</script>' });
  await page.route('**/preview.json', route => route.fulfill({ json: bad }));
  await page.reload();
  await expect(page.getByRole('alert')).toBeVisible();
});

test('runtime reflows controls on narrow screens and resets deterministically', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 900 });
  await page.goto('/preview.html');
  await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Resource', exact: true }).click();
  await page.getByRole('button', { name: 'Reset', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Identity — selected', exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  const audit = await new AxeBuilder({ page }).analyze();
  expect(audit.violations.filter(item => ['serious', 'critical'].includes(item.impact ?? ''))).toEqual([]);
});

test('interactive hotspots reject unknown items and out-of-bounds frames', async ({ page }) => {
  for (const hotspot of [
    { item_id: 'unknown', x: 0, y: 0, width: 20, height: 20 },
    { item_id: 'identity', x: 4090, y: 0, width: 20, height: 20 },
    { item_id: 'identity', x: 0, y: 0, width: 0, height: 20 },
    { item_id: 'identity', x: 0, y: 0, width: 20, height: 20, html: '<script/>' },
  ]) {
    const payload = await fixture('summary');
    payload.views[0].hotspots = [hotspot];
    await page.route('**/preview.json', route => route.fulfill({ json: payload }));
    await page.goto('/preview.html');
    await expect(page.getByRole('alert')).toBeVisible();
    await expect(page.getByRole('button')).toHaveCount(0);
    await page.unroute('**/preview.json');
  }
});

test('embedded control frames reject unsupported or overflowing geometry', async ({ page }) => {
  for (const frame of [
    { x: 4000, y: 0, width: 200, height: 40 },
    { x: 0, y: 0, width: -1, height: 40 },
    { x: 0, y: 0, width: 200, height: 40, html: '<script/>' },
  ]) {
    const payload = await fixture('summary');
    payload.views[0].controls_frame = frame;
    await page.route('**/preview.json', route => route.fulfill({ json: payload }));
    await page.goto('/preview.html');
    await expect(page.getByRole('alert')).toBeVisible();
    await page.unroute('**/preview.json');
  }
});

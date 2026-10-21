import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { chromium, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const { values } = parseArgs({ options: { directory: { type: 'string', default: '.artifacts\\rich-interactive-showcase' } } });
const directory = resolve(values.directory);
const ledger = JSON.parse(await readFile(join(directory, 'aurora-authoring.json'), 'utf8'));
const receipt = JSON.parse(await readFile(join(directory, 'aurora.manifest.json'), 'utf8'));
for (const file of receipt.files) {
  assert.equal(createHash('sha256').update(await readFile(join(directory, file.filename))).digest('hex'), file.sha256);
}
const screenshots = join(directory, 'screenshots');
await mkdir(screenshots, { recursive: true });
const browser = await chromium.launch({ channel: 'msedge' });
const report = { synthetic: true, powerpoint_host_interaction_verified: false, pages: [], requests: [] };
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('request', request => report.requests.push(request.url()));
  const checkText = async () => {
    const problems = await page.locator('.aislide-interactive__text').evaluateAll(nodes => nodes.filter(node => node.textContent.trim() && (node.scrollWidth > node.clientWidth + 2 || node.scrollHeight > node.clientHeight + 2)).map(node => ({
      text: node.textContent, actual: [node.scrollWidth, node.scrollHeight], frame: [node.clientWidth, node.clientHeight],
    })));
    assert.deepEqual(problems, [], 'Visible native text is clipped');
    await expect(page.getByRole('alert')).toHaveCount(0);
    await page.mouse.move(0, 0);
    const painted = await page.locator('.aislide-interactive__hotspot[aria-pressed="true"]').evaluateAll(nodes => nodes.map(node => {
      const style = getComputedStyle(node);
      return { background: style.backgroundColor, border: style.borderTopWidth, padding: style.paddingTop };
    }));
    assert.ok(painted.every(style => style.background === 'rgba(0, 0, 0, 0)' && style.border === '0px' && style.padding === '0px'), 'Selected hotspot must not paint over the native card/text');
  };
  const buttonFor = label => page.getByRole('button', { name: `Select ${label}`, exact: true });
  const openOptions = async () => {
    const toggle = page.locator('.aislide-interactive__options > summary');
    if (await toggle.isVisible() && await toggle.evaluate(node => !node.parentElement.open)) await toggle.click();
  };
  const closeOptions = async () => {
    const toggle = page.locator('.aislide-interactive__options > summary');
    if (await toggle.isVisible() && await toggle.evaluate(node => node.parentElement.open)) await toggle.click();
  };
  for (const [index, slide] of ledger.slides.entries()) {
    const spec = slide.spec.data.spec;
    const filename = `${String(index + 1).padStart(2, '0')}-${slide.id}.html`;
    await page.goto(pathToFileURL(join(directory, filename)).href);
    await page.evaluate(() => document.fonts.ready);
    await expect(buttonFor(spec.items[0].label)).toHaveAttribute('aria-pressed', 'true');
    await checkText();
    await page.screenshot({ path: join(screenshots, `${String(index + 1).padStart(2, '0')}-default.png`) });
    await page.getByRole('button', { name: `Select ${spec.items[1].label}`, exact: true }).focus();
    await page.keyboard.press('Enter');
    await expect(buttonFor(spec.items[1].label)).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByRole('button', { name: `Select ${spec.items[1].label}`, exact: true })).toBeFocused();
    await page.getByRole('button', { name: 'Reset', exact: true }).click();
    let states = 0;
    const groups = [null, ...spec.groups.map(group => group.id)];
    const dimensions = spec.template === 'comparison' ? spec.dimensions.map((_, i) => i) : [0];
    const scales = spec.template === 'dashboard' ? ['linear', 'logarithmic'] : ['linear'];
    for (const group of groups) {
      if (spec.groups.length) await page.getByRole('combobox', { name: 'Group', exact: true }).selectOption(group ?? '');
      for (const dimension of dimensions) {
        if (spec.template === 'comparison') await page.getByRole('combobox', { name: 'Dimension', exact: true }).selectOption(String(dimension));
        for (const scale of scales) {
          if (spec.template === 'dashboard') await page.getByRole('combobox', { name: 'Scale', exact: true }).selectOption(scale);
          for (const item of spec.items.filter(item => !group || item.group_id === group)) {
            await page.getByRole('button', { name: `Select ${item.label}`, exact: true }).click();
            await expect(buttonFor(item.label)).toHaveAttribute('aria-pressed', 'true');
            const texts = await page.locator('.aislide-interactive__text').allTextContents();
            assert.ok(texts.includes(item.detail), `Detail not updated: ${slide.id}/${item.id}`);
            for (const metric of item.metrics) {
              assert.ok(texts.some(text => text.includes(metric.value)), `Metric missing: ${slide.id}/${item.id}/${metric.label}`);
            }
            if (spec.template === 'comparison') assert.ok(texts.includes(item.values[dimension]), `Comparison dimension did not update: ${item.id}/${dimension}`);
            if (spec.template === 'dashboard') {
              const visible = spec.items.filter(item => !group || item.group_id === group);
              const maximum = Math.max(...visible.map(item => item.value));
              const expected = visible.filter(item => item.value > 0).map(item => (slide.spec.layout.width * 0.64 - 60) * 0.43
                * (scale === 'linear' ? item.value / maximum : Math.log1p(item.value) / Math.log1p(maximum)));
              const actual = await page.locator('.aislide-interactive__rect').evaluateAll((nodes, colors) => {
                const normal = document.createElement('span');
                const shades = colors.map(color => { normal.style.color = `#${color}`; return normal.style.color; });
                return nodes.filter(node => node.style.height === '18px' && shades.includes(node.style.backgroundColor)).map(node => parseFloat(node.style.width));
              }, [ledger.theme.colors.accent1, ledger.theme.colors.accent3]);
              assert.equal(actual.length, expected.length);
              actual.forEach((width, i) => assert.ok(Math.abs(width - expected[i]) < 0.001, 'Dashboard bar must use the declared scale and visible-group maximum'));
            }
            await checkText();
            states++;
          }
        }
      }
    }
    assert.equal(states, slide.compiled_states);
    await page.screenshot({ path: join(screenshots, `${String(index + 1).padStart(2, '0')}-selected.png`) });
    await page.getByRole('button', { name: 'Reset', exact: true }).click();
    await expect(buttonFor(spec.items[0].label)).toHaveAttribute('aria-pressed', 'true');
    if (spec.template === 'timeline') {
      await page.getByRole('button', { name: 'Next', exact: true }).click();
      await expect(buttonFor(spec.items[1].label)).toHaveAttribute('aria-pressed', 'true');
      await page.getByRole('button', { name: 'Previous', exact: true }).click();
      await expect(buttonFor(spec.items[0].label)).toHaveAttribute('aria-pressed', 'true');
    }
    for (const viewport of [{ width: 1324, height: 746 }, { width: 864, height: 408 }, { width: 640, height: 300 }, { width: 390, height: 900 }, { width: 440, height: 540 }]) {
      await page.setViewportSize(viewport);
      await expect.poll(() => page.locator('.aislide-interactive__drawing').evaluate(node => {
        const rect = node.getBoundingClientRect();
        return rect.width > 0 && rect.height > 0 && rect.left >= -1 && rect.top >= -1 && rect.right <= innerWidth + 1 && rect.bottom <= innerHeight + 1;
      })).toBe(true);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight), true);
      await expect.poll(() => page.locator('#interactive-root').evaluate(node => Math.abs(node.getBoundingClientRect().width - innerWidth))).toBeLessThan(1);
      assert.equal(await page.locator('.aislide-interactive__items').isVisible(), false, 'Rich content must not duplicate item buttons below the slide');
      const menu = await page.locator('.aislide-interactive__menu').evaluate(node => {
        const rect = node.getBoundingClientRect();
        const drawing = document.querySelector('.aislide-interactive__drawing').getBoundingClientRect();
        return { top: rect.top, bottom: rect.bottom, left: rect.left, right: rect.right, drawTop: drawing.top, drawBottom: drawing.bottom, screenWidth: innerWidth };
      });
      assert.ok(menu.top >= menu.drawTop - 1 && menu.bottom <= menu.drawBottom + 1 && menu.left >= -1 && menu.right <= menu.screenWidth + 1, 'Controls must remain inside the rendered canvas');
      const separation = await page.locator('.aislide-interactive__menu').evaluate(node => {
        const menu = node.getBoundingClientRect();
        const targets = [...document.querySelectorAll('.aislide-interactive__hotspot')].map(node => node.getBoundingClientRect());
        return targets.every(rect => menu.bottom <= rect.top + 1 || menu.right <= rect.left + 1 || menu.left >= rect.right - 1);
      });
      assert.equal(separation, true, 'Collapsed controls must not cover a card, bar or stage');
      await openOptions();
      await page.getByRole('button', { name: 'Reset', exact: true }).scrollIntoViewIfNeeded();
      await expect(page.getByRole('button', { name: 'Reset', exact: true })).toBeInViewport();
      await page.getByRole('button', { name: 'Reset', exact: true }).click();
      await closeOptions();
    }
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.addStyleTag({ content: 'body{margin:140px auto 0;max-width:440px}main{width:440px;max-width:440px;height:540px}' });
    await expect.poll(() => page.locator('.aislide-interactive__drawing').evaluate(node => node.getBoundingClientRect().width)).toBeGreaterThan(1000);
    const audit = await new AxeBuilder({ page }).analyze();
    assert.deepEqual(audit.violations.filter(item => ['serious', 'critical'].includes(item.impact ?? '')), []);
    report.pages.push({ id: slide.id, template: spec.template, states, viewports: 5, narrow_host_width: 440, host_style_override_checked: true, serious_accessibility_violations: 0 });
    console.log(`Verified ${slide.id}: ${states} states, five viewports, no clipped text or opaque selection overlays`);
  }
  assert.deepEqual(errors, []);
  assert.ok(report.requests.every(url => url.startsWith(pathToFileURL(directory).href)));
  report.total_states = report.pages.reduce((sum, page) => sum + page.states, 0);
  report.network_requests = report.requests.filter(url => /^https?:/.test(url)).length;
  await writeFile(join(directory, 'browser-verification.json'), JSON.stringify(report, null, 2), { flag: 'wx' });
  console.log(`Verified ${report.pages.length} pages / ${report.total_states} states; no network requests.`);
} finally { await browser.close(); }

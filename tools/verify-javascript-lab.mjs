import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import { createHash } from 'node:crypto';
import { chromium, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const { values } = parseArgs({ options: { directory: { type: 'string', default: '.artifacts\\javascript-expression-lab' } } });
const directory = resolve(values.directory);
const manifest = JSON.parse(await readFile(join(directory, 'javascript-lab.manifest.json'), 'utf8'));
for (const file of manifest.files) {
  assert.equal(createHash('sha256').update(await readFile(join(directory, file.filename))).digest('hex'), file.sha256);
}
await mkdir(join(directory, 'screenshots'), { recursive: true });
const report = { format: 'aislide.javascript-lab-browser-verification', version: 1, synthetic: true, powerpoint_host_interaction_verified: false, checks: [], network_requests: [], errors: [] };
const browser = await chromium.launch({ channel: 'msedge' });
try {
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  const page = await context.newPage();
  const checkCanvas = async () => {
    await expect.poll(() => page.evaluate(() => {
      const problems = [];
      const shell = document.querySelector('.lab-shell');
      if (!shell) return ['Missing slide canvas'];
      const bounds = shell.getBoundingClientRect();
      if (bounds.width <= 0 || bounds.height <= 0 || bounds.left < -1 || bounds.top < -1 || bounds.right > innerWidth + 1 || bounds.bottom > innerHeight + 1) problems.push('Canvas is outside the WebView');
      const containers = '.lab-shell,.lab-header,.lab-main,.lab-card,.lab-grid,.lab-visual,.lab-controls,.lab-footer,.lab-table-wrap,.lab-comparison,.lab-mock,.lab-explanation,.lab-explanation-page';
      for (const node of document.querySelectorAll(containers)) {
        if (!node.getClientRects().length || node.clientHeight <= 0) continue;
        if (node.scrollHeight > node.clientHeight + 1 || node.scrollWidth > node.clientWidth + 1) problems.push(`${node.className}: ${node.scrollWidth}x${node.scrollHeight} exceeds ${node.clientWidth}x${node.clientHeight}`);
      }
      if (document.documentElement.scrollHeight > innerHeight + 1 || document.documentElement.scrollWidth > innerWidth + 1) problems.push('Document scrollbar');
      return problems;
    })).toEqual([]);
  };
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('request', request => { if (/^https?:/.test(request.url())) report.network_requests.push(request.url()); });
  const open = async kind => {
    const definition = manifest.pages.find(page => page.kind === kind);
    await page.goto(pathToFileURL(join(directory, definition.html)).href);
    await expect(page.getByRole('heading', { name: definition.title, exact: true })).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
    return definition;
  };
  const range = async (name, value) => {
    const input = page.getByLabel(name, { exact: true });
    await input.evaluate((element, value) => { element.value = String(value); element.dispatchEvent(new Event('input', { bubbles: true })); element.dispatchEvent(new Event('change', { bubbles: true })); }, value);
  };
  const numeric = async id => Number((await page.getByTestId(id).textContent()).replace(/[^\d.-]/g, ''));
  const capture = async (kind, state) => {
    await checkCanvas();
    await page.screenshot({ path: join(directory, 'screenshots', `${kind}-${state}.png`) });
  };
  const reset = async () => page.getByRole('button', { name: 'リセット', exact: true }).click();
  await page.setViewportSize({ width: 1280, height: 720 });
  for (const definition of manifest.pages) {
    await open(definition.kind);
    await checkCanvas();
    assert.equal(await page.locator('.lab-shell').evaluate(node => node.clientWidth), 1280);
    assert.equal(await page.locator('.lab-shell').evaluate(node => node.clientHeight), 720);
  }
  await page.setViewportSize({ width: 1440, height: 900 });

  await open('simulator');
  assert.equal(await numeric('revenue'), 96000);
  assert.equal(await numeric('cost'), 60000);
  assert.equal(await numeric('balance'), 36000);
  await capture('simulator', 'default');
  await page.getByLabel('台数', { exact: true }).focus();
  await page.keyboard.press('End');
  assert.equal(await numeric('revenue'), 240000);
  await range('1台あたりの利用回数', 10);
  await range('1回の料金', 1000);
  assert.equal(await numeric('revenue'), 2000000);
  assert.equal(await numeric('cost'), 120000);
  assert.equal(await numeric('balance'), 1880000);
  await capture('simulator', 'changed');
  await reset();
  assert.equal(await numeric('revenue'), 96000);
  await range('台数', 20); await range('1台あたりの利用回数', 1); await range('1回の料金', 100);
  assert.equal(await numeric('balance'), -28000);
  await expect(page.locator('.lab-balance')).toHaveAttribute('data-negative', 'true');
  await reset();
  report.checks.push({ kind: 'simulator', checks: ['exact formula/default/limits', 'keyboard range', 'live values', 'reset'] });

  await open('network');
  await capture('network', 'default');
  await page.getByRole('button', { name: '認証', exact: true }).click();
  await expect(page.getByTestId('node-detail')).toContainText('認証');
  const impactBefore = await page.getByTestId('network-impact').textContent();
  await page.getByRole('button', { name: '障害を切り替える', exact: true }).click();
  await expect.poll(() => page.getByTestId('network-impact').textContent()).not.toBe(impactBefore);
  await expect(page.getByTestId('network-impact')).toContainText('決済');
  await expect(page.getByTestId('network-impact')).toContainText('通知');
  await capture('network', 'changed');
  await reset();
  report.checks.push({ kind: 'network', checks: ['direct node selection', 'failure impact update', 'reset'] });

  await open('priority');
  await capture('priority', 'default');
  const tile = page.getByRole('button', { name: '予約画面', exact: true });
  await tile.focus();
  const start = await page.getByTestId('priority-position').textContent();
  await page.keyboard.press('ArrowRight');
  await expect.poll(() => page.getByTestId('priority-position').textContent()).not.toBe(start);
  const box = await tile.boundingBox();
  assert.ok(box);
  const keyboardPosition = await page.getByTestId('priority-position').textContent();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down(); await page.mouse.move(box.x + box.width / 2 + 100, box.y + box.height / 2 - 60, { steps: 8 }); await page.mouse.up();
  await expect.poll(() => page.getByTestId('priority-position').textContent()).not.toBe(keyboardPosition);
  await range('実行負担', 95); await range('期待する影響', 15);
  await expect(page.getByTestId('priority-position')).toContainText('95');
  await capture('priority', 'changed');
  await reset();
  report.checks.push({ kind: 'priority', checks: ['pointer drag', 'arrow-key alternative', 'range alternative', 'bounded position', 'reset'] });

  await open('playback');
  await capture('playback', 'default');
  await range('時点', 45);
  await expect(page.getByTestId('time-progress')).toContainText('45');
  await page.getByRole('button', { name: '再生', exact: true }).click();
  await expect.poll(async () => Number(await page.getByLabel('時点', { exact: true }).inputValue())).toBeGreaterThan(45);
  await page.getByRole('button', { name: '一時停止', exact: true }).click();
  const paused = await page.getByLabel('時点', { exact: true }).inputValue();
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  assert.equal(await page.getByLabel('時点', { exact: true }).inputValue(), paused);
  await capture('playback', 'changed');
  await reset();
  assert.equal(Number(await page.getByLabel('時点', { exact: true }).inputValue()), 0);
  report.checks.push({ kind: 'playback', checks: ['scrub', 'rAF progress', 'pause stable', 'reset'] });

  await open('comparison');
  await capture('comparison', 'default');
  const comparisonBefore = await page.getByTestId('comparison-after').getAttribute('style');
  await range('境界位置', 80);
  await expect.poll(() => page.getByTestId('comparison-after').getAttribute('style')).not.toBe(comparisonBefore);
  const divider = page.getByRole('slider', { name: '比較の境界', exact: true });
  const dividerBox = await divider.boundingBox();
  assert.ok(dividerBox);
  await page.mouse.move(dividerBox.x + dividerBox.width / 2, dividerBox.y + dividerBox.height / 2);
  await page.mouse.down(); await page.mouse.move(dividerBox.x - 80, dividerBox.y + dividerBox.height / 2, { steps: 6 }); await page.mouse.up();
  assert.notEqual(Number(await page.getByLabel('境界位置', { exact: true }).inputValue()), 80);
  await capture('comparison', 'changed');
  await reset();
  report.checks.push({ kind: 'comparison', checks: ['live reveal boundary', 'reset'] });

  await open('quiz');
  await capture('quiz', 'default');
  await page.getByRole('button', { name: '小さく試す', exact: true }).click();
  await page.getByRole('button', { name: '観測して判断', exact: true }).click();
  await expect(page.getByTestId('quiz-result')).not.toBeEmpty();
  await capture('quiz', 'changed');
  await page.getByRole('button', { name: '一つ戻る', exact: true }).click();
  await expect(page.getByRole('button', { name: '前提を再確認', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '前提を再確認', exact: true }).click();
  await expect(page.getByTestId('quiz-result')).not.toBeEmpty();
  await reset();
  await expect(page.getByRole('button', { name: '一度に広げる', exact: true })).toBeVisible();
  const endings = new Set();
  for (const first of ['小さく試す', '一度に広げる']) {
    for (const second of ['観測して判断', '前提を再確認']) {
      await page.getByRole('button', { name: first, exact: true }).click();
      await page.getByRole('button', { name: second, exact: true }).click();
      endings.add(await page.getByTestId('quiz-result').textContent());
      await reset();
    }
  }
  assert.equal(endings.size, 4, 'Both decisions must affect the explanation');
  report.checks.push({ kind: 'quiz', checks: ['two branches', 'backtracking', 'reset'] });

  await open('explorer');
  await capture('explorer', 'default');
  await expect(page.getByTestId('result-count')).toContainText('8');
  await page.getByLabel('地域を検索', { exact: true }).fill('中央');
  await expect(page.getByTestId('result-count')).toContainText('1');
  await page.getByRole('button', { name: /中央/, exact: false }).click();
  await expect(page.getByTestId('record-detail')).toContainText('中央');
  await page.getByLabel('地域を検索', { exact: true }).fill('該当しない地域');
  await expect(page.getByTestId('result-count')).toContainText('0');
  await expect(page.locator('.lab-empty')).toBeVisible();
  await reset();
  await page.getByLabel('分類', { exact: true }).selectOption({ label: '都市' });
  const amounts = async () => {
    const all = [];
    while (await page.getByRole('button', { name: '前の行', exact: true }).isEnabled()) await page.getByRole('button', { name: '前の行', exact: true }).click();
    do {
      all.push(...await page.locator('tbody tr').evaluateAll(rows => rows.map(row => Number(row.cells[2].textContent.replace(/[^\d]/g, '')))));
      await checkCanvas();
      if (await page.getByRole('button', { name: '次の行', exact: true }).isDisabled()) break;
      await page.getByRole('button', { name: '次の行', exact: true }).click();
    } while (true);
    while (await page.getByRole('button', { name: '前の行', exact: true }).isEnabled()) await page.getByRole('button', { name: '前の行', exact: true }).click();
    return all;
  };
  const firstOrder = await amounts();
  await page.getByRole('button', { name: '利用件数で並べ替え', exact: true }).click();
  const secondOrder = await amounts();
  assert.deepEqual(secondOrder, [...firstOrder].reverse(), 'Sort must reverse the actual displayed record order');
  await capture('explorer', 'changed');
  await reset();
  assert.equal((await amounts()).length, 8, 'Every record must remain reachable through pagination, not be clipped');
  report.checks.push({ kind: 'explorer', checks: ['search', 'empty state', 'category', 'sort', 'selected detail', 'reset'] });

  await open('canvas');
  await capture('canvas', 'default');
  await range('点の数', 100);
  await expect(page.getByLabel('点を選択', { exact: true }).locator('option')).toHaveCount(100);
  await page.getByLabel('点を選択', { exact: true }).selectOption({ index: 4 });
  const description = await page.getByTestId('canvas-detail').textContent();
  await page.locator('canvas').click({ position: { x: 50, y: 50 } });
  await expect.poll(() => page.getByTestId('canvas-detail').textContent()).not.toBe(description);
  await page.getByRole('button', { name: '動かす', exact: true }).click();
  await expect(page.getByRole('button', { name: '止める', exact: true })).toBeVisible();
  await page.getByRole('button', { name: '止める', exact: true }).click();
  await capture('canvas', 'changed');
  await reset();
  await expect(page.getByLabel('点を選択', { exact: true }).locator('option')).toHaveCount(60);
  await page.setViewportSize({ width: 640, height: 360 });
  await checkCanvas();
  await expect.poll(() => page.locator('canvas').evaluate(node => Math.abs(node.width - Math.round(node.getBoundingClientRect().width * Math.min(devicePixelRatio, 2))))).toBeLessThan(2);
  const smallBacking = await page.locator('canvas').evaluate(node => node.width);
  await page.setViewportSize({ width: 1440, height: 900 });
  await checkCanvas();
  await expect.poll(() => page.locator('canvas').evaluate(node => node.width)).toBeGreaterThan(smallBacking);
  await expect.poll(() => page.locator('canvas').evaluate(node => Math.abs(node.width - Math.round(node.getBoundingClientRect().width * Math.min(devicePixelRatio, 2))))).toBeLessThan(2);
  report.checks.push({ kind: 'canvas', checks: ['point count', 'selection alternative', 'direct point selection', 'start/stop', 'reset'] });

  for (const definition of manifest.pages) {
    await open(definition.kind);
    for (const viewport of [{ width: 1324, height: 746 }, { width: 864, height: 408 }, { width: 640, height: 300 }, { width: 390, height: 900 }, { width: 440, height: 540 }]) {
      await page.setViewportSize(viewport);
      await checkCanvas();
      await expect(page.getByRole('button', { name: 'リセット', exact: true })).toBeInViewport();
      await expect(page.getByRole('alert')).toHaveCount(0);
      for (const control of await page.locator('.lab-main input, .lab-main select').all()) {
        await expect(control).toBeInViewport();
      }
    }
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.getByRole('button', { name: '解説', exact: true }).click();
    const explanationPages = [];
    do {
      await checkCanvas();
      explanationPages.push(await page.locator('.lab-explanation-page').textContent());
      if (await page.getByRole('button', { name: '次の解説', exact: true }).isDisabled()) break;
      await page.getByRole('button', { name: '次の解説', exact: true }).click();
    } while (true);
    assert.ok(explanationPages.some(text => text.includes(manifest.data_source)), 'Full source must be available in the explanation pages');
    await page.getByRole('button', { name: 'スライドに戻る', exact: true }).click();
    await checkCanvas();
    const audit = await new AxeBuilder({ page }).analyze();
    assert.deepEqual(audit.violations.filter(item => ['serious', 'critical'].includes(item.impact ?? '')), [], definition.kind);
  }
  const nativeCanvases = [{ width: 1200, height: 900 }, { width: 960, height: 540 }, { width: 960, height: 480 }];
  for (const definition of manifest.pages) {
    const html = await readFile(join(directory, definition.html), 'utf8');
    assert.equal((html.match(/"canvas":\{"height":[\d.]+,"width":[\d.]+\}/g) ?? []).length, 1, 'Generated lab must declare exactly one native canvas');
    for (const canvas of nativeCanvases) {
      await page.goto('about:blank');
      await page.setViewportSize({ width: 960, height: 540 });
      await page.setContent(html.replace(/"canvas":\{"height":[\d.]+,"width":[\d.]+\}/, `"canvas":${JSON.stringify(canvas)}`));
      await checkCanvas();
      if (definition.kind === 'quiz') {
        await page.getByRole('button', { name: '一度に広げる', exact: true }).click();
        await page.getByRole('button', { name: '前提を再確認', exact: true }).click();
        await checkCanvas();
      }
      const paneToggle = page.getByRole('button', { name: '操作を表示', exact: true });
      if (await paneToggle.isVisible()) {
        await paneToggle.click();
        await checkCanvas();
        await expect(page.locator('.lab-visual')).toBeHidden();
        await expect(page.locator('.lab-controls')).toBeVisible();
        if (definition.kind === 'simulator') await range('台数', 150);
        await page.getByRole('button', { name: '図を表示', exact: true }).click();
        if (definition.kind === 'simulator') assert.equal(await numeric('revenue'), 180000, 'Visual/control paging must preserve live input');
        await checkCanvas();
      }
      await page.getByRole('button', { name: '解説', exact: true }).click();
      do {
        await checkCanvas();
        if (await page.getByRole('button', { name: '次の解説', exact: true }).isDisabled()) break;
        await page.getByRole('button', { name: '次の解説', exact: true }).click();
      } while (true);
    }
  }
  const reduced = await browser.newContext({ reducedMotion: 'reduce', viewport: { width: 864, height: 408 } });
  const reducedPage = await reduced.newPage();
  await reducedPage.goto(pathToFileURL(join(directory, manifest.pages.find(page => page.kind === 'playback').html)).href);
  await reducedPage.getByLabel('時点', { exact: true }).evaluate(element => { element.value = '100'; element.dispatchEvent(new Event('input', { bubbles: true })); });
  await expect(reducedPage.getByRole('button', { name: '再生', exact: true })).toBeDisabled();
  assert.equal(await reducedPage.getByLabel('時点', { exact: true }).inputValue(), '100');
  await reducedPage.goto(pathToFileURL(join(directory, manifest.pages.find(page => page.kind === 'canvas').html)).href);
  await expect(reducedPage.getByRole('button', { name: '動かす', exact: true })).toBeDisabled();
  await reduced.close();
  await open('canvas');
  const announced = await page.getByTestId('canvas-detail').textContent();
  await page.getByRole('button', { name: '動かす', exact: true }).click();
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 150)));
  assert.equal(await page.getByTestId('canvas-detail').textContent(), announced, 'Canvas must not rewrite a live region every animation frame');
  await page.getByRole('button', { name: '止める', exact: true }).click();
  await open('playback');
  await page.getByRole('button', { name: '再生', exact: true }).click();
  await expect(page.getByRole('button', { name: '一時停止', exact: true })).toBeVisible();
  await page.evaluate(() => {
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => true });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await expect(page.getByRole('button', { name: '再生', exact: true })).toBeVisible();
  await reset();
  assert.equal(await page.getByLabel('時点', { exact: true }).inputValue(), '0');
  const original = manifest.pages.find(page => page.kind === 'simulator');
  const longMetadataHtml = (await readFile(join(directory, original.html), 'utf8'))
    .replace(JSON.stringify(original.title), JSON.stringify('長い見出し'.repeat(16)))
    .replace(JSON.stringify(manifest.data_source), JSON.stringify('長い注記'.repeat(60)));
  await page.setViewportSize({ width: 390, height: 300 });
  await page.setContent(longMetadataHtml);
  await expect(page.getByRole('alert')).toHaveCount(0);
  await checkCanvas();
  await page.getByRole('button', { name: '解説', exact: true }).click();
  await expect(page.locator('.lab-explanation-page')).toContainText('長い見出し'.repeat(16));
  await checkCanvas();
  await page.getByRole('button', { name: '次の解説', exact: true }).click();
  await expect(page.locator('.lab-explanation-page')).toContainText('長い注記'.repeat(60));
  await checkCanvas();
  await expect(page.getByRole('button', { name: 'リセット', exact: true })).toBeInViewport();
  const baseHtml = await readFile(join(directory, original.html), 'utf8');
  for (const canvas of [null, { width: 0, height: 720 }, { width: 1280, height: 479 }, { width: 1280, height: 720, scale: 0.1 }]) {
    await page.goto('about:blank');
    await page.setContent(baseHtml.replace(/"canvas":\{"height":[\d.]+,"width":[\d.]+\}/, `"canvas":${JSON.stringify(canvas)}`));
    await expect(page.getByRole('alert')).toContainText('canvas');
    await expect(page.getByRole('button', { name: 'リセット', exact: true })).toHaveCount(0);
  }
  report.additional_checks = ['reduced motion retains scrubbed endpoint', 'Canvas live region quiet during animation', 'simulated hidden-document event pauses playback', 'long metadata retained through no-scroll explanation pages', 'all eight records reachable through no-scroll table pages', 'no internal or document scrollbars'];
  assert.deepEqual(report.errors, []);
  assert.deepEqual(report.network_requests, []);
  report.pages = manifest.pages.length; report.viewport_count = 5; report.native_canvases = [{ width: 1280, height: 720 }, ...nativeCanvases]; report.serious_accessibility_violations = 0;
  await writeFile(join(directory, 'browser-verification.json'), JSON.stringify(report, null, 2), { flag: 'wx' });
  console.log(JSON.stringify(report, null, 2));
} finally { await browser.close(); }

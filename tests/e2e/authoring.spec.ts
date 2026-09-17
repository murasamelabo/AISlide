import { test, expect } from '@playwright/test';
import { openSample } from './fixtures';

test('dense picture drags measure browser work without changing the image data', async ({ page }, testInfo) => {
  test.setTimeout(90_000);
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  const fixture = await page.evaluate(async () => {
    const request = async (value: unknown) => {
      const response = await fetch('/api/core', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(value) });
      if (!response.ok) throw new Error(await response.text());
      return response.json();
    };
    const document = await request({ op: 'create_presentation', id: 'drag-benchmark', title: 'Synthetic drag fixture' });
    const image = await request({ op: 'create_graph_icon', mime_type: 'image/svg+xml', alt: 'Synthetic image', base64: btoa('<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256"><rect width="256" height="256" fill="#007a4d"/><circle cx="128" cy="128" r="80" fill="#ffffff"/></svg>') });
    document.deck.slides[0].elements = Array.from({ length: 80 }, (_, index) => ({ ...image, type: 'picture', id: `picture-${index}`, x: index === 0 ? 24 : 336 + (index % 10) * 88, y: index === 0 ? 240 : 16 + Math.floor(index / 10) * 84, width: 72, height: 72 }));
    return request({ op: 'export', deck: document.deck });
  });
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'drag-fixture.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(fixture.base64, 'base64') });
  await expect(page.locator('.slide-stage img')).toHaveCount(80);
  await page.evaluate(async () => { await document.fonts.ready; await Promise.all([...document.querySelectorAll<HTMLImageElement>('.slide-stage img')].map((image) => image.decode())); });
  const picture = page.locator('.slide-stage [data-element-id="picture-0"]');
  const hitbox = picture.getByRole('button', { name: 'Edit picture-0', exact: true });
  const originalData = await picture.locator('img').getAttribute('src');
  const protocol = await page.context().newCDPSession(page);
  await protocol.send('Performance.enable');
  const measurements = [];
  for (let sample = 0; sample < 3; sample++) {
    await expect(hitbox).toBeVisible();
    const start = (await hitbox.boundingBox())!;
    await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
    await page.mouse.down();
    const before = new Map((await protocol.send('Performance.getMetrics')).metrics.map((metric) => [metric.name, metric.value]));
    await page.mouse.move(start.x + start.width / 2 + 72, start.y + start.height / 2 + 36, { steps: 120 });
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    const after = new Map((await protocol.send('Performance.getMetrics')).metrics.map((metric) => [metric.name, metric.value]));
    const dragged = (await picture.boundingBox())!;
    expect(Math.abs(dragged.x - start.x - 72)).toBeLessThan(1);
    measurements.push({ taskMs: 1000 * (after.get('TaskDuration')! - before.get('TaskDuration')!), layoutCount: after.get('LayoutCount')! - before.get('LayoutCount')!, styleMs: 1000 * (after.get('RecalcStyleDuration')! - before.get('RecalcStyleDuration')!) });
    await page.mouse.up();
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
    await expect(hitbox).toBeVisible();
    expect(await picture.locator('img').getAttribute('src')).toBe(originalData);
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect.poll(async () => Math.abs((await picture.boundingBox())!.x - start.x)).toBeLessThan(1);
  }
  await testInfo.attach('drag-browser-work', { body: JSON.stringify({ pictures: 80, pointerEventsPerDrag: 120, measurements }), contentType: 'application/json' });
  await protocol.detach();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

for (const operation of ['move', 'resize'] as const) {
  test(`canvas ${operation} keeps its preview until the core commit completes`, async ({ page }, testInfo) => {
    await page.goto('/');
    await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
    const picker = page.getByRole('dialog', { name: 'Icons and assets', exact: true });
    await picker.getByRole('button', { name: 'Insert icon', exact: true }).click();
    const picture = page.locator('.slide-stage [data-element-id]').filter({ has: page.locator('img') });
    const hitbox = picture.getByRole('button', { name: /^Edit / });
    await expect(picture.locator('img')).toHaveJSProperty('complete', true);
    await hitbox.click();
    const before = (await picture.boundingBox())!;
    const control = operation === 'move' ? hitbox : picture.getByRole('button', { name: /^Resize / });
    const pointer = (await control.boundingBox())!;
    let release = () => {};
    let started = () => {};
    let commits = 0;
    const waiting = new Promise<void>((resolve) => { release = resolve; });
    const observed = new Promise<void>((resolve) => { started = resolve; });
    await page.route('**/api/core', async (route) => {
      if (route.request().postDataJSON().op === 'transaction') { commits += 1; started(); await waiting; }
      await route.continue();
    });
    let preview = before;
    try {
      await page.mouse.move(pointer.x + pointer.width / 2, pointer.y + pointer.height / 2);
      await page.mouse.down();
      await page.mouse.move(pointer.x + pointer.width / 2 + 72, pointer.y + pointer.height / 2 + 36, { steps: 12 });
      preview = (await picture.boundingBox())!;
      expect(preview[operation === 'move' ? 'x' : 'width'] - before[operation === 'move' ? 'x' : 'width']).toBeGreaterThan(60);
      expect(commits).toBe(0);
      await page.mouse.up();
      await observed;
      const samples = await picture.evaluate(async (element) => {
        const frames = [];
        for (let index = 0; index < 4; index++) {
          await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
          const bounds = element.getBoundingClientRect();
          frames.push({ x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height });
        }
        return frames;
      });
      const jumps = samples.map((bounds) => Math.max(...(['x', 'y', 'width', 'height'] as const).map((key) => Math.abs(bounds[key] - preview[key]))));
      await testInfo.attach('pending-preview-jump', { body: JSON.stringify({ operation, before, preview, samples, maxJumpPx: Math.max(...jumps), commits }), contentType: 'application/json' });
      expect(Math.max(...jumps)).toBeLessThan(1);
      expect(commits).toBe(1);
    } finally { release(); }
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
    await expect.poll(async () => Math.abs((await picture.boundingBox())![operation === 'move' ? 'x' : 'width'] - preview[operation === 'move' ? 'x' : 'width'])).toBeLessThan(1);
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect.poll(async () => Math.abs((await picture.boundingBox())![operation === 'move' ? 'x' : 'width'] - before[operation === 'move' ? 'x' : 'width'])).toBeLessThan(1);
    await expect(page.getByRole('alert')).toHaveCount(0);
  });
}

test('canvas drag cancellation never commits or leaves a preview on another slide', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  await page.getByRole('dialog', { name: 'Icons and assets', exact: true }).getByRole('button', { name: 'Insert icon', exact: true }).click();
  await page.getByRole('button', { name: 'Duplicate slide', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(2);
  await page.locator('.thumbnail').first().click();
  const picture = page.locator('.slide-stage [data-element-id]').filter({ has: page.locator('img') });
  const hitbox = picture.getByRole('button', { name: /^Edit / });
  const before = (await picture.boundingBox())!;
  const operations: string[] = [];
  page.on('request', (request) => { if (request.url().endsWith('/api/core') && request.method() === 'POST') operations.push(request.postDataJSON().op); });
  for (const cancellation of ['escape', 'pointercancel', 'capture', 'slide']) {
    await hitbox.evaluate((button) => button.addEventListener('pointerdown', (event) => { button.dataset.testPointerId = String((event as PointerEvent).pointerId); }, { once: true }));
    await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
    await page.mouse.down();
    await page.mouse.move(before.x + before.width / 2 + 54, before.y + before.height / 2 + 24, { steps: 6 });
    await expect.poll(async () => (await picture.boundingBox())!.x - before.x).toBeGreaterThan(50);
    if (cancellation === 'escape') await page.keyboard.press('Escape');
    else if (cancellation === 'pointercancel') await hitbox.evaluate((button) => button.dispatchEvent(new PointerEvent('pointercancel', { pointerId: Number(button.dataset.testPointerId), bubbles: true })));
    else if (cancellation === 'capture') await hitbox.evaluate((button) => button.releasePointerCapture(Number(button.dataset.testPointerId)));
    else {
      await page.locator('.thumbnail').last().evaluate((button: HTMLButtonElement) => button.click());
      await expect(page.locator('.thumbnail').last()).toHaveAttribute('aria-current', 'true');
      await page.locator('.thumbnail').first().evaluate((button: HTMLButtonElement) => button.click());
      await expect(page.locator('.thumbnail').first()).toHaveAttribute('aria-current', 'true');
    }
    await page.mouse.up();
    await expect.poll(async () => Math.abs((await picture.boundingBox())!.x - before.x), { message: cancellation }).toBeLessThan(1);
    await expect(page.locator('.slide-stage .moving')).toHaveCount(0);
    expect(operations.filter((operation) => operation === 'transaction')).toHaveLength(0);
  }
});

test('a rejected canvas move restores the committed position and permits a retry', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  await page.getByRole('dialog', { name: 'Icons and assets', exact: true }).getByRole('button', { name: 'Insert icon', exact: true }).click();
  const picture = page.locator('.slide-stage [data-element-id]').filter({ has: page.locator('img') });
  const hitbox = picture.getByRole('button', { name: /^Edit / });
  const before = (await picture.boundingBox())!;
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op === 'transaction') await route.fulfill({ status: 409, contentType: 'application/json', body: JSON.stringify({ error: 'Synthetic move rejection' }) });
    else await route.continue();
  });
  await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
  await page.mouse.down();
  await page.mouse.move(before.x + before.width / 2 + 54, before.y + before.height / 2 + 24, { steps: 6 });
  await page.mouse.up();
  await expect(page.getByRole('alert')).toContainText('Synthetic move rejection');
  await expect.poll(async () => Math.abs((await picture.boundingBox())!.x - before.x)).toBeLessThan(1);
  await expect(page.locator('.slide-stage .moving')).toHaveCount(0);
  await page.unroute('**/api/core');
  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click();
  const originalX = Number(await page.getByLabel('Element x', { exact: true }).inputValue());
  await hitbox.focus();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByLabel('Element x', { exact: true })).toHaveValue(String(originalX + 1));
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('canvas clicks preserve fractional geometry and fast drops use the final pointer position', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  const response = await page.request.post('/api/core', { headers: { Origin: new URL(page.url()).origin }, data: { op: 'export', deck: { version: 1, title: 'Synthetic fractional fixture', width: 1280, height: 720, slides: [{ id: 'slide-1', title: 'Fractional rectangle', background: 'FFFFFF', notes: '', elements: [{ type: 'rect', id: 'fractional', x: 64.25, y: 180.5, width: 200, height: 100, fill: '007A4D' }] }] } } });
  expect(response.ok()).toBe(true);
  const exported = await response.json();
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'fractional.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(exported.base64, 'base64') });
  const element = page.locator('.slide-stage [data-element-id="fractional"]');
  const control = element.getByRole('button', { name: 'Edit fractional', exact: true });
  await expect(control).toBeVisible();
  const before = await element.evaluate((node) => ({ x: parseFloat((node as HTMLElement).style.left), y: parseFloat((node as HTMLElement).style.top) }));
  let commits = 0;
  page.on('request', (request) => { if (request.url().endsWith('/api/core') && request.method() === 'POST' && request.postDataJSON().op === 'transaction') commits += 1; });
  await control.click();
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
  expect(commits).toBe(0);
  expect(await element.evaluate((node) => ({ x: parseFloat((node as HTMLElement).style.left), y: parseFloat((node as HTMLElement).style.top) }))).toEqual(before);
  const bounds = (await control.boundingBox())!;
  const start = { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 };
  const scale = bounds.width / 200;
  await control.evaluate((button) => button.addEventListener('pointerdown', (event) => { button.dataset.testPointerId = String((event as PointerEvent).pointerId); }, { once: true }));
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  await control.evaluate((button, point) => {
    const pointerId = Number(button.dataset.testPointerId);
    for (const [type, dx, dy] of [['pointermove', 24, 12], ['pointermove', 36, 18], ['pointerup', 60, 30], ['pointerup', 60, 30]] as const) {
      button.dispatchEvent(new PointerEvent(type, { pointerId, isPrimary: true, pointerType: 'mouse', button: 0, buttons: type === 'pointerup' ? 0 : 1, clientX: point.x + dx, clientY: point.y + dy, bubbles: true }));
    }
  }, start);
  await page.mouse.up();
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  await expect.poll(() => element.evaluate((node) => parseFloat((node as HTMLElement).style.left))).toBe(Math.round(before.x + 60 / scale));
  expect(commits).toBe(1);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect.poll(() => element.evaluate((node) => parseFloat((node as HTMLElement).style.left))).toBe(before.x);
});

test('another pending edit cancels an unfinished drag without leaving a locked gesture', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  await page.getByRole('dialog', { name: 'Icons and assets', exact: true }).getByRole('button', { name: 'Insert icon', exact: true }).click();
  const picture = page.locator('.slide-stage [data-element-id]').filter({ has: page.locator('img') });
  const hitbox = picture.getByRole('button', { name: /^Edit / });
  const before = (await picture.boundingBox())!;
  let release = () => {};
  let started = () => {};
  let commits = 0;
  const waiting = new Promise<void>((resolve) => { release = resolve; });
  const observed = new Promise<void>((resolve) => { started = resolve; });
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op === 'transaction') { commits += 1; started(); await waiting; }
    await route.continue();
  });
  try {
    await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
    await page.mouse.down();
    await page.mouse.move(before.x + before.width / 2 + 54, before.y + before.height / 2 + 24, { steps: 6 });
    await expect.poll(async () => (await picture.boundingBox())!.x - before.x).toBeGreaterThan(50);
    await page.getByRole('checkbox', { name: 'Show master graphics', exact: true }).evaluate((input: HTMLInputElement) => input.click());
    await observed;
    await expect(hitbox).toHaveCount(0);
    await page.mouse.up();
    await expect.poll(async () => Math.abs((await picture.boundingBox())!.x - before.x)).toBeLessThan(1);
    expect(commits).toBe(1);
  } finally { release(); }
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  await expect(hitbox).toBeVisible();
  await page.mouse.move(before.x + before.width / 2, before.y + before.height / 2);
  await page.mouse.down();
  await page.mouse.move(before.x + before.width / 2 + 54, before.y + before.height / 2 + 24, { steps: 6 });
  await page.mouse.up();
  await expect.poll(async () => (await picture.boundingBox())!.x - before.x).toBeGreaterThan(50);
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  expect(commits).toBe(2);
});

test('text edits directly on the slide, supports multiline input and undo', async ({ page }) => {
  await openSample(page);
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
  await openSample(page);
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
  await openSample(page);
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
  await openSample(page);
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
  await openSample(page);
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
  await openSample(page);
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
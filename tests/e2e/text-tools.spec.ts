import { test, expect, type Page } from '@playwright/test';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

async function mountEditor(page: Page, mode: 'inline' | 'panel' | 'inspector' = 'inline') {
  page.on('pageerror', (error) => console.error(error.message));
  page.on('requestfailed', (request) => console.error(request.url(), request.failure()));
  const clientUrl = `/@fs/${resolve('packages/client/index.mjs').replaceAll('\\', '/')}`;
  await page.route('**/__text-tools-test', (route) => route.fulfill({ contentType: 'text/html; charset=utf-8', body: `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Text controls test</title></head><body><div id="root"></div><script type="module">
    import RefreshRuntime from '/@react-refresh';
    RefreshRuntime.injectIntoGlobalHook(window);
    window.$RefreshReg$ = () => {}; window.$RefreshSig$ = () => (type) => type;
    window.__vite_plugin_react_preamble_installed__ = true;
    const editorModule = await fetch('/src/InlineEditor.tsx').then((response) => response.text());
    const mainModule = await fetch('/src/main.tsx').then((response) => response.text());
    const toolModule = await fetch('/src/Tool.tsx').then((response) => response.text());
    const { default: React } = await import(editorModule.split('"').find((url) => url.includes('/react.js?')));
    const { default: ReactDOM } = await import(mainModule.split('"').find((url) => url.includes('/react-dom_client.js?')));
    const Tooltip = await import(toolModule.split('"').find((url) => url.includes('react-tooltip.js?')));
    await import('/src/studio.css');
    const { InlineEditor } = await import('/src/InlineEditor.tsx');
    const mode = ${JSON.stringify(mode)};
    const { TextToolsPanel } = mode === 'panel' ? await import('/src/TextToolsPanel.tsx') : {};
    const { TextControls } = mode === 'inspector' ? await import('/src/TextControls.tsx') : {};
    const { AislideClient } = await import(${JSON.stringify(clientUrl)});
    const client = new AislideClient(async (request) => {
      const response = await fetch('/api/core', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(request) });
      if (!response.ok) throw new Error(await response.text());
      return response.json();
    });
    const initial = { type: 'text', id: 'body', x: 60, y: 100, width: 600, height: 220, text: 'A😀日本B', font_size: 24, color: '112233', bold: false, format: { paragraphs: [{ runs: [{ text: 'A', style: { italic: true } }, { text: '😀日本', style: {} }, { text: 'B', style: { underline: true } }] }] } };
    if (mode === 'panel') initial.format = { font_family: 'Arial' };
    const source = await client.createPresentation('text-tools-test');
    const log = [];
    async function sessionFor(element) {
      const deck = source.document.deck;
      deck.slides[0].elements = [element];
      return client.createDocument({ id: 'draft-test', deck });
    }
    const editingSession = await sessionFor(initial);
    const adapters = {
      onReplaceText: async (element, text) => {
        log.push({ op: 'replace', text });
        const session = await sessionFor(element);
        const doc = await session.replaceTextContent(session.document.deck.slides[0].id, { id: element.id, text });
        return doc.deck.slides[0].elements[0];
      },
      onFormatRange: async (element, start, end, style) => {
        log.push({ op: 'format', start, end, style });
        const session = await sessionFor(element);
        const doc = await session.formatText(session.document.deck.slides[0].id, { id: element.id, start, end, style });
        return doc.deck.slides[0].elements[0];
      },
    };
    function Harness() {
      const [committed, setCommitted] = React.useState(null);
      const [closed, setClosed] = React.useState(false);
      const [dirty, setDirty] = React.useState(false);
      const [documentSnapshot, setDocumentSnapshot] = React.useState(editingSession.document);
      const [liveSession, setLiveSession] = React.useState(editingSession);
      const [inspected, setInspected] = React.useState(initial);
      const [navigation, setNavigation] = React.useState(null);
      const [busy, setBusy] = React.useState(false);
      if (mode === 'panel') return React.createElement('main', { style: { maxWidth: '380px' } },
        React.createElement(TextToolsPanel, { session: liveSession, onDocument: setDocumentSnapshot, onNavigate: (...args) => setNavigation(args), onBusy: setBusy, disabled: busy }),
        React.createElement('button', { onClick: () => { setLiveSession(source); setDocumentSnapshot(source.document); } }, 'Switch document'),
        React.createElement('button', { disabled: busy || !liveSession.canUndo, onClick: async () => setDocumentSnapshot(await liveSession.undo()) }, 'Undo AI test edit'),
        React.createElement('output', { 'data-testid': 'document' }, JSON.stringify(documentSnapshot)),
        React.createElement('output', { 'data-testid': 'navigation' }, JSON.stringify(navigation)),
        React.createElement('output', { 'data-testid': 'busy' }, String(busy)));
      if (mode === 'inspector') return React.createElement('main', { style: { maxWidth: '380px' } },
        React.createElement(TextControls, { element: inspected, onChange: setInspected, ...adapters }),
        React.createElement('output', { 'data-testid': 'inspected' }, JSON.stringify(inspected)));
      return React.createElement('main', {},
        !closed && React.createElement('div', { style: { position: 'relative', margin: '60px', width: '600px', height: '220px' } }, React.createElement(InlineEditor, {
          element: initial, onDraftChange: setDirty, ...adapters,
          onCommit: async (element) => { await sessionFor(element); setCommitted(element); },
          onCancel: () => setClosed(true),
        })),
        React.createElement('output', { 'data-testid': 'committed' }, JSON.stringify(committed)),
        React.createElement('output', { 'data-testid': 'calls' }, JSON.stringify(log)),
        React.createElement('output', { 'data-testid': 'dirty' }, String(dirty)),
        React.createElement('output', { 'data-testid': 'closed' }, String(closed)));
    }
    ReactDOM.createRoot(document.getElementById('root')).render(React.createElement(Tooltip.Provider, {}, React.createElement(Harness)));
  </script></body></html>` }));
  await page.goto('/__text-tools-test');
  await expect(page.getByRole('textbox', { name: mode === 'panel' ? 'Find text' : mode === 'inspector' ? 'Text content' : 'Slide text editor', exact: true })).toBeVisible();
}

for (const task of ['proofread', 'translate'] as const) test(`G04 local AI ${task} bounded-fixture review apply cancel and Undo`, async ({ page }) => {
  await mountEditor(page, 'panel');
  const before = JSON.parse(await page.getByTestId('document').innerText());
  let hold = false;
  let started = () => {};
  let release = () => {};
  let entered = new Promise<void>(resolve => { started = resolve; });
  const held = new Promise<void>(resolve => { release = resolve; });
  const proposed = task === 'proofread' ? 'A😀日本B corrected' : 'これは翻訳の固定テスト応答です。';
  await page.route('**/api/core', async route => {
    const request = route.request().postDataJSON();
    if (request.op !== 'text_assist') return route.continue();
    expect(request.input.task).toBe(task);
    if (hold) { started(); await held; }
    const hash = createHash('sha256').update(request.input.text).digest('hex');
    await route.fulfill({ json: { candidate: { text: proposed, source_sha256: hash }, provenance: { mode: 'model', model: 'bounded-test-fixture-not-ai', source_sha256: hash, remote: false, elapsed_ms: 1, verified: false, attempts: 1 } } });
  });
  await page.getByRole('tab', { name: 'Local AI', exact: true }).click();
  await page.getByLabel('AI task', { exact: true }).selectOption(task);
  if (task === 'translate') await page.getByLabel('AI target language', { exact: true }).fill('ja-JP');
  await page.getByRole('button', { name: 'Generate text candidate', exact: true }).click();
  const review = page.getByRole('region', { name: 'AI text review' });
  await expect(review).toBeVisible();
  expect(JSON.parse(await page.getByTestId('document').innerText()).hash).toBe(before.hash);
  await expect(review.locator('ins')).toHaveText(proposed);
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: 960 });
    await page.getByTestId('document').evaluate(element => { element.setAttribute('hidden', ''); });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await review.screenshot({ path: `.artifacts/local-ai-${task}-${width}.png` });
  }
  await page.getByRole('button', { name: 'Apply AI text', exact: true }).click();
  await expect.poll(async () => JSON.parse(await page.getByTestId('document').textContent() ?? '{}').deck.slides[0].elements[0].text).toBe(proposed);
  await page.getByRole('button', { name: 'Undo AI test edit', exact: true }).click();
  await expect.poll(async () => JSON.parse(await page.getByTestId('document').textContent() ?? '{}').hash).toBe(before.hash);
  hold = true;
  entered = new Promise<void>(resolve => { started = resolve; });
  await page.getByRole('button', { name: 'Generate text candidate', exact: true }).click();
  await entered;
  await page.getByRole('button', { name: 'Cancel local AI', exact: true }).click();
  release();
  await expect(page.getByRole('button', { name: 'Generate text candidate', exact: true })).toBeEnabled();
  await expect(review).toHaveCount(0);
  expect(JSON.parse(await page.getByTestId('document').textContent() ?? '{}').hash).toBe(before.hash);
  await expect(page.getByRole('button', { name: 'Undo AI test edit', exact: true })).toBeDisabled();
});

test('inline selection formats Unicode scalars and preserves neighboring rich runs', async ({ page }) => {
  await mountEditor(page);
  const editor = page.getByRole('textbox', { name: 'Slide text editor' });
  await editor.focus();
  await page.keyboard.press('Home');
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Shift+ArrowRight');
  await page.keyboard.press('Shift+ArrowRight');
  await page.getByRole('button', { name: 'Bold', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Apply on-slide edit' })).toBeEnabled();
  await page.getByRole('button', { name: 'Apply on-slide edit' }).click();
  await expect(page.getByTestId('closed')).toHaveText('true');
  const result = JSON.parse(await page.getByTestId('committed').innerText());
  expect(result.text).toBe('A😀日本B');
  expect(result.format.paragraphs[0].runs).toEqual([
    { text: 'A', style: { italic: true } },
    { text: '😀日本', style: { bold: true } },
    { text: 'B', style: { underline: true } },
  ]);
  expect(JSON.parse(await page.getByTestId('calls').innerText())).toEqual([{ op: 'format', start: 1, end: 4, style: { bold: true } }]);
});

test('text tools search navigates and replaces selected matches through the session', async ({ page }) => {
  await mountEditor(page, 'panel');
  await page.getByRole('textbox', { name: 'Find text', exact: true }).fill('日本');
  await page.getByRole('button', { name: 'Find', exact: true }).click();
  await expect(page.getByRole('checkbox', { name: 'Select match 1' })).toBeVisible();
  await page.getByRole('button', { name: 'Go to match 1' }).click();
  expect(JSON.parse(await page.getByTestId('navigation').innerText()).slice(0, 2)).toEqual([0, 'body']);
  await page.getByRole('checkbox', { name: 'Select match 1' }).check();
  await page.getByRole('textbox', { name: 'Replace with', exact: true }).fill('東京😀');
  await page.getByRole('button', { name: 'Replace selected', exact: true }).click();
  await expect(page.getByTestId('busy')).toHaveText('false');
  await expect.poll(async () => JSON.parse(await page.getByTestId('document').innerText()).deck.slides[0].elements[0].text).toBe('A😀東京😀B');
  await expect(page.getByRole('checkbox', { name: 'Select match 1' })).toHaveCount(0);
});

test('inspector flushes rich text explicitly before changing paragraph settings', async ({ page }) => {
  await mountEditor(page, 'inspector');
  await page.getByRole('textbox', { name: 'Text content', exact: true }).fill('A😀日本B\nSecond');
  expect(JSON.parse(await page.getByTestId('inspected').innerText()).text).toBe('A😀日本B');
  await page.getByRole('button', { name: 'Apply text content', exact: true }).click();
  await expect.poll(async () => JSON.parse(await page.getByTestId('inspected').innerText()).text).toBe('A😀日本B\nSecond');
  await page.getByRole('combobox', { name: 'Paragraph', exact: true }).selectOption({ value: '1' });
  await page.getByRole('combobox', { name: 'Paragraph list', exact: true }).selectOption('numbered');
  await page.getByRole('spinbutton', { name: 'Number start', exact: true }).fill('4');
  await page.getByRole('spinbutton', { name: 'Line spacing', exact: true }).fill('120');
  const result = JSON.parse(await page.getByTestId('inspected').innerText());
  expect(result.format.paragraphs[0].bullet).toBeUndefined();
  expect(result.format.paragraphs[1]).toMatchObject({ bullet: 'numbered', number_start: 4, line_spacing: { kind: 'percent', value: 120000 } });
});

test('inline formatting flushes once, retains a blurred selection, and applies every run control', async ({ page }) => {
  await mountEditor(page);
  const requests: string[] = [];
  page.on('request', (request) => { if (request.url().endsWith('/api/core')) requests.push(request.postDataJSON().op); });
  const editor = page.getByRole('textbox', { name: 'Slide text editor' });
  await editor.fill('A😀日本B!');
  expect(requests).toEqual([]);
  await editor.press('Home');
  await editor.press('ArrowRight');
  for (let index = 0; index < 3; index++) await editor.press('Shift+ArrowRight');
  for (const name of ['Italic', 'Underline', 'Superscript', 'Subscript']) {
    await page.getByRole('button', { name, exact: true }).click();
    await expect(page.getByRole('button', { name: 'Apply on-slide edit' })).toBeEnabled();
  }
  await page.getByRole('spinbutton', { name: 'Selection font size' }).fill('36');
  await expect(page.getByRole('button', { name: 'Apply on-slide edit' })).toBeEnabled();
  for (const [label, value] of [['Selection color', '#aa1122'], ['Selection highlight', '#ffee33']]) {
    await page.getByLabel(label, { exact: true }).fill(value);
    await expect(page.getByRole('button', { name: 'Apply on-slide edit' })).toBeEnabled();
  }
  await page.getByRole('button', { name: 'Apply on-slide edit' }).click();
  await expect(page.getByTestId('closed')).toHaveText('true');
  const result = JSON.parse(await page.getByTestId('committed').innerText());
  expect(result.text).toBe('A😀日本B!');
  expect(result.format.paragraphs[0].runs.find((run: { text: string }) => run.text === '😀日本').style).toMatchObject({ italic: true, underline: true, baseline: -25000, font_size: 36, color: 'AA1122', highlight: 'FFEE33' });
  expect(requests.filter((operation) => operation === 'replace_text_content')).toHaveLength(1);
});

test('inline IME and pending formatting block competing commit and cancellation', async ({ page }) => {
  await mountEditor(page);
  const editor = page.getByRole('textbox', { name: 'Slide text editor' });
  await editor.dispatchEvent('compositionstart');
  await editor.press('Control+Enter');
  await editor.press('Escape');
  await expect(page.getByTestId('closed')).toHaveText('false');
  await editor.dispatchEvent('compositionend');
  await editor.press('Control+a');
  let release = () => {};
  let entered = () => {};
  const held = new Promise<void>((resolve) => { release = resolve; });
  const started = new Promise<void>((resolve) => { entered = resolve; });
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op === 'format_text') { entered(); await held; }
    await route.continue();
  });
  try {
    await page.getByRole('button', { name: 'Bold', exact: true }).click();
    await started;
    await expect(page.getByRole('button', { name: 'Apply on-slide edit' })).toBeDisabled();
    await expect(page.getByRole('button', { name: 'Cancel on-slide edit' })).toBeDisabled();
    await page.locator('.inline-editor').dispatchEvent('keydown', { key: 'Escape', bubbles: true });
    await page.locator('.inline-editor').dispatchEvent('keydown', { key: 'Enter', ctrlKey: true, bubbles: true });
    await expect(page.getByTestId('closed')).toHaveText('false');
  } finally { release(); }
  await expect(page.getByRole('button', { name: 'Apply on-slide edit' })).toBeEnabled();
  await editor.press('Control+Enter');
  await expect(page.getByTestId('closed')).toHaveText('true');
});

test('failed inline preparation preserves the typed draft for retry or Escape', async ({ page }) => {
  await mountEditor(page);
  const editor = page.getByRole('textbox', { name: 'Slide text editor' });
  await editor.fill('A😀日本B changed');
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op === 'replace_text_content') await route.fulfill({ status: 409, json: { error: 'Synthetic text conflict' } });
    else await route.continue();
  });
  await editor.press('Control+Enter');
  await expect(page.getByRole('alert')).toContainText('Synthetic text conflict');
  await expect(editor).toHaveValue('A😀日本B changed');
  await expect(page.getByTestId('closed')).toHaveText('false');
  await editor.press('Escape');
  await expect(page.getByTestId('closed')).toHaveText('true');
  await expect(page.getByTestId('committed')).toHaveText('null');
});

test('search options invalidate old matches and font replacement commits through the session', async ({ page }) => {
  await mountEditor(page, 'panel');
  await page.getByRole('textbox', { name: 'Find text', exact: true }).fill('日本');
  await page.getByRole('button', { name: 'Find', exact: true }).click();
  await expect(page.getByRole('checkbox', { name: 'Select match 1' })).toBeVisible();
  await page.getByRole('checkbox', { name: 'Match case' }).check();
  await expect(page.getByRole('checkbox', { name: 'Select match 1' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Replace all', exact: true })).toBeDisabled();
  await page.getByRole('textbox', { name: 'From font', exact: true }).fill('Arial');
  await page.getByRole('textbox', { name: 'To font', exact: true }).fill('Georgia');
  await page.getByRole('button', { name: 'Replace font', exact: true }).click();
  await expect.poll(async () => JSON.parse(await page.getByTestId('document').innerText()).deck.slides[0].elements[0].format.font_family).toBe('Georgia');
});

test('inspector whole-text formatting updates rich runs rather than only their frame default', async ({ page }) => {
  await mountEditor(page, 'inspector');
  await page.getByRole('button', { name: 'Bold', exact: true }).click();
  await expect.poll(async () => JSON.parse(await page.getByTestId('inspected').innerText()).format.paragraphs[0].runs.every((run: { style: { bold?: boolean } }) => run.style.bold)).toBe(true);
});

test('inline formatting remains reachable at the narrow viewport edge', async ({ page }, testInfo) => {
  await mountEditor(page);
  await testInfo.attach('inline-desktop', { body: await page.screenshot({ path: '.artifacts/text-tools-desktop.png' }), contentType: 'image/png' });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('.inline-editor').evaluate((form) => {
    Object.assign(form.parentElement!.style, { position: 'fixed', left: '290px', top: '20px', width: '90px', height: '180px', margin: '0' });
    window.dispatchEvent(new Event('resize'));
  });
  const controls = page.locator('.inline-rich-tools');
  await expect(controls).toBeVisible();
  await expect.poll(async () => { const bounds = (await controls.boundingBox())!; return bounds.x >= 0 && bounds.x + bounds.width <= 390; }).toBe(true);
  const toolbar = (await controls.boundingBox())!;
  const actions = (await page.locator('.inline-edit-actions').boundingBox())!;
  expect(toolbar.y >= actions.y + actions.height || toolbar.y + toolbar.height <= actions.y || toolbar.x >= actions.x + actions.width || toolbar.x + toolbar.width <= actions.x).toBe(true);
  await testInfo.attach('inline-narrow', { body: await page.screenshot({ path: '.artifacts/text-tools-narrow.png' }), contentType: 'image/png' });
});

test('switching sessions during search discards results and releases the panel', async ({ page }) => {
  await mountEditor(page, 'panel');
  let release = () => {};
  let entered = () => {};
  const held = new Promise<void>((resolve) => { release = resolve; });
  const started = new Promise<void>((resolve) => { entered = resolve; });
  await page.route('**/api/core', async (route) => {
    if (route.request().postDataJSON().op === 'search_text') { entered(); await held; }
    await route.continue();
  });
  await page.getByRole('textbox', { name: 'Find text', exact: true }).fill('日本');
  try {
    await page.getByRole('button', { name: 'Find', exact: true }).click();
    await started;
    await page.getByRole('button', { name: 'Switch document', exact: true }).click();
  } finally { release(); }
  await expect(page.getByRole('button', { name: 'Find', exact: true })).toBeEnabled();
  await expect(page.getByRole('checkbox', { name: 'Select match 1' })).toHaveCount(0);
});
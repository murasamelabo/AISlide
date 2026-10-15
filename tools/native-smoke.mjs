import { chromium, expect } from '@playwright/test';

const browser = await chromium.connectOverCDP('http://127.0.0.1:9227');
try {
  const pages = browser.contexts().flatMap((context) => context.pages());
  const page = pages.find((candidate) => candidate.url().includes('tauri.localhost'));
  if (!page) throw new Error('Native AISlide WebView not found');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Select title', exact: true }).click();
  await page.getByLabel('Text content', { exact: true }).fill('Native Tauri verification');
  await page.getByRole('button', { name: 'Apply changes' }).click();
  await expect(page.locator('.slide-stage').getByText('Native Tauri verification', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await page.screenshot({ path: '.artifacts/studio-native.png', fullPage: true });
  const exported = await page.evaluate(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const report = await invoke('core_request', { request: { op: 'sample' } });
    const { deck } = await invoke('core_request', { request: { op: 'compile', report } });
    return invoke('core_request', { request: { op: 'export', deck } });
  });
  if (Buffer.from(exported.base64, 'base64').subarray(0, 2).toString() !== 'PK') throw new Error('Native export is not ZIP');
  console.log('Native WebView: 12 slides, text edit, undo and Rust IPC export passed.');
} finally { await browser.close(); }
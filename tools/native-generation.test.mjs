import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { chromium, expect } from '@playwright/test';
import { requestCore } from './core-client.mjs';
import { startProviderFixture, fixtureEnvironment } from './testing/provider-fixture.mjs';

test('native WebView generation cancels, retries, applies and undoes a draft', { timeout: 90_000, skip: process.platform !== 'win32' }, async (context) => {
  const root = resolve(import.meta.dirname, '..');
  const fixture = await startProviderFixture(await requestCore({ op: 'sample' }));
  context.after(() => fixture.close());
  const profile = await mkdtemp(join(tmpdir(), 'aislide-native-generation-'));
  const reservation = createServer();
  reservation.listen(0, '127.0.0.1');
  await once(reservation, 'listening');
  const port = reservation.address().port;
  await new Promise((resolveClose, reject) => reservation.close((error) => error ? reject(error) : resolveClose()));
  const app = spawn(join(root, 'apps/studio/src-tauri/target/debug/aislide-studio.exe'), [], {
    cwd: root,
    stdio: 'ignore',
    shell: false,
    env: {
      ...process.env,
      ...fixtureEnvironment(fixture.endpoint),
      AISLIDE_AI_TIMEOUT_SECONDS: '5',
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
      WEBVIEW2_USER_DATA_FOLDER: profile,
    },
  });
  const exited = once(app, 'close');
  let browser;
  context.after(async () => {
    await browser?.close();
    if (app.exitCode === null && app.signalCode === null) app.kill();
    await exited;
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  });
  await once(app, 'spawn');
  await expect(async () => {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
  }).toPass({ timeout: 20_000 });
  const page = browser.contexts().flatMap((browserContext) => browserContext.pages()).find((candidate) => candidate.url().includes('tauri.localhost'));
  assert.ok(page, 'Expected the owned native AISlide WebView');
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.getByRole('button', { name: 'Sources', exact: true }).click();
  await page.getByLabel('Source file', { exact: true }).setInputFiles({ name: 'native-source.csv', mimeType: 'text/csv', buffer: Buffer.from('Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n') });
  await expect(page.locator('.source-preview')).toContainText('Q1');
  await page.getByLabel('Report title', { exact: true }).fill('Native source report');
  await page.getByRole('button', { name: 'Compile data report', exact: true }).click();
  await expect(page.locator('.document-name strong')).toHaveText('Native source report');
  await page.getByRole('button', { name: /^Slide 4:/ }).click();
  await expect(page.locator('.slide-stage .recharts-wrapper')).toHaveCount(1);
  await page.getByRole('button', { name: 'Validate layout', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Layout validation' })).toContainText('cosmic-text');
  await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.document-name strong')).toHaveText('Quarterly performance');
  await page.getByRole('button', { name: 'Generate with AI', exact: true }).click();
  await expect(page.getByText('local-fixture-model', { exact: true })).toBeVisible();
  await page.getByLabel('Slide count', { exact: true }).fill('3');
  await page.getByLabel('Brief', { exact: true }).fill('WAIT_FOR_CANCELLATION');
  const observed = fixture.nextRequest();
  await page.getByRole('button', { name: 'Generate draft', exact: true }).click();
  const waiting = await observed;
  await page.getByRole('button', { name: 'Cancel generation', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Generation cancelled');
  await waiting.disconnected;
  await expect(page.locator('.document-name strong')).toHaveText('Quarterly performance');
  await page.getByLabel('Brief', { exact: true }).fill('Retry native model generation');
  await page.getByRole('button', { name: 'Generate draft', exact: true }).click();
  await expect(page.locator('.generation-review')).toBeVisible();
  await expect(page.getByText('AI content unverified', { exact: true })).toBeVisible();
  await mkdir(join(root, '.artifacts'), { recursive: true });
  await page.screenshot({ path: join(root, '.artifacts/generation-native.png') });
  await page.getByRole('button', { name: 'Apply draft', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(3);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  const cancelled = await page.evaluate(async () => {
    const invoke = window.__TAURI_INTERNALS__.invoke;
    const operationId = crypto.randomUUID();
    const pending = invoke('core_request', { operationId, request: { op: 'generate', input: { prompt: 'WAIT_FOR_CANCELLATION immediate abort', source_text: '', slide_count: 3 } } }).then(() => 'unexpected success', (error) => String(error));
    const acknowledged = await invoke('cancel_core_request', { operationId });
    return { acknowledged, result: await pending };
  });
  assert.equal(cancelled.acknowledged, true, 'Immediate native cancellation must reach its request');
  assert.match(cancelled.result, /cancelled/i);
});
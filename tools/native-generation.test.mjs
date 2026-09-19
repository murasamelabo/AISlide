import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readdir, rm, writeFile } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { promisify } from 'node:util';
import { chromium, expect } from '@playwright/test';
import { requestCore } from './core-client.mjs';
import { startProviderFixture, fixtureEnvironment } from './testing/provider-fixture.mjs';

async function nativeTransaction(page, action) {
  const endpoint = await page.evaluate(() => window.__TAURI_INTERNALS__.convertFileSrc('core_request', 'ipc'));
  const completed = (async () => {
    const response = await page.waitForResponse(response => response.url() === endpoint
      && response.request().method() === 'POST' && response.request().postDataJSON().request?.op === 'transaction');
    assert.equal(response.ok(), true, 'Native transaction transport must succeed');
    assert.equal(await response.finished(), null, 'Native transaction response body must finish');
    console.log('NATIVE_TRANSACTION_COMPLETE', JSON.stringify({ status: response.status(), op: 'transaction', finished: true }));
  })();
  await Promise.all([completed, action()]);
}

async function ownedRecovery(context) {
  const directory = await mkdtemp(join(tmpdir(), 'aislide-native-owned-'));
  const owner = randomUUID();
  await writeFile(join(directory, '.aislide-test-owner'), owner, { flag: 'wx' });
  const cleanup = [];
  context.after(async () => {
    for (const stop of cleanup) await stop();
    await rm(directory, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  });
  return { directory, cleanup, env: { AISLIDE_TEST_RECOVERY_ROOT: directory, AISLIDE_TEST_RECOVERY_OWNER: owner } };
}

async function launchOwnedNative(context, recovery, endpoint) {
  const root = resolve(import.meta.dirname, '..');
  const profile = await mkdtemp(join(tmpdir(), 'aislide-native-webview-'));
  const reservation = createServer();
  reservation.listen(0, '127.0.0.1');
  await once(reservation, 'listening');
  const port = reservation.address().port;
  await new Promise((resolveClose, reject) => reservation.close(error => error ? reject(error) : resolveClose()));
  const executable = process.env.AISLIDE_NATIVE_TEST_EXE ?? join(root, 'apps/studio/src-tauri/target/debug/aislide-studio.exe');
  const app = spawn(executable, [`--aislide-owned-test=${recovery.env.AISLIDE_TEST_RECOVERY_OWNER}`], {
    cwd: root, stdio: 'ignore', shell: false,
    env: { ...process.env, ...fixtureEnvironment(endpoint), ...recovery.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`, WEBVIEW2_USER_DATA_FOLDER: profile },
  });
  const exited = new Promise(resolveClose => app.once('close', resolveClose));
  let browser;
  let stopped = false;
  async function stop() {
    if (stopped) return;
    stopped = true;
    try { await browser?.close(); }
    finally {
      if (app.exitCode === null && app.signalCode === null) app.kill();
      await exited;
      await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
    }
  }
  recovery.cleanup.push(stop);
  await once(app, 'spawn');
  await expect(async () => {
    assert.equal(app.exitCode, null, 'Owned native host exited before CDP connection');
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
  }).toPass({ timeout: 25_000 });
  let page;
  await expect(() => {
    page = browser.contexts().flatMap(current => current.pages()).find(candidate => candidate.url().includes('tauri.localhost'));
    assert.ok(page, 'Expected the owned native WebView');
  }).toPass({ timeout: 15_000 });
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  return { app, page, browser, profile, stop };
}

async function nativeSaveDialog(processId, action, destination = '') {
  const { stdout } = await promisify(execFile)('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `
    $ErrorActionPreference = 'Stop'
    Set-StrictMode -Version Latest
    Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; using System.Text; public static class AISlideDialogTest {
      public delegate bool Callback(IntPtr handle, IntPtr data);
      [DllImport("user32.dll")] public static extern bool EnumWindows(Callback callback, IntPtr data);
      [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, Callback callback, IntPtr data);
      [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr handle, out uint processId);
      [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr handle, StringBuilder name, int length);
      [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr handle);
      [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr handle, int index);
      [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr handle);
      [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "SendMessageTimeoutW")] public static extern IntPtr ReadText(IntPtr handle, uint message, UIntPtr count, StringBuilder text, uint flags, uint timeout, out UIntPtr result);
      [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "SendMessageTimeoutW")] public static extern IntPtr SetText(IntPtr handle, uint message, UIntPtr word, string text, uint flags, uint timeout, out UIntPtr result);
      [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr handle, uint message, IntPtr word, IntPtr data);
    }'
    $handles = [System.Collections.Generic.List[System.IntPtr]]::new()
    $enumerate = [AISlideDialogTest+Callback]{ param($handle, $data)
      $ownerProcess = [uint32]0
      [void][AISlideDialogTest]::GetWindowThreadProcessId($handle, [ref]$ownerProcess)
      if ($ownerProcess -eq [uint32]$env:AISLIDE_TEST_PID) {
        $className = [System.Text.StringBuilder]::new(256)
        [void][AISlideDialogTest]::GetClassName($handle, $className, 256)
        if ($className.ToString() -eq '#32770') { $handles.Add($handle) }
      }
      return $true
    }
    [void][AISlideDialogTest]::EnumWindows($enumerate, [IntPtr]::Zero)
    if ($handles.Count -ne 1) { throw "Expected one native save dialog owned by the test process, found $($handles.Count)" }
    $controls = @{}
    $children = [AISlideDialogTest+Callback]{ param($handle, $data)
      $className = [System.Text.StringBuilder]::new(256)
      [void][AISlideDialogTest]::GetClassName($handle, $className, 256)
      $id = [AISlideDialogTest]::GetDlgCtrlID($handle)
      if (($id -eq 1001 -and $className.ToString() -eq 'Edit') -or ($id -in @(1, 2) -and $className.ToString() -eq 'Button')) { $controls[$id] = $handle }
      return $true
    }
    [void][AISlideDialogTest]::EnumChildWindows($handles[0], $children, [IntPtr]::Zero)
    if (-not $controls.ContainsKey(1001) -or ([AISlideDialogTest]::GetWindowLong($controls[1001], -16) -band 0x20)) { throw 'Expected a non-password filename field' }
    $text = [System.Text.StringBuilder]::new(4096)
    $result = [UIntPtr]::Zero
    if ([AISlideDialogTest]::ReadText($controls[1001], 0x000D, [UIntPtr]::new(4096), $text, 2, 2000, [ref]$result) -eq [IntPtr]::Zero) { throw 'Cannot read the test filename' }
    $filename = $text.ToString()
    $buttonId = 2
    if ($env:AISLIDE_TEST_ACTION -eq 'save') {
      if (Test-Path -LiteralPath $env:AISLIDE_TEST_DESTINATION) { throw 'Refusing an existing test destination' }
      if ([AISlideDialogTest]::SetText($controls[1001], 0x000C, [UIntPtr]::Zero, $env:AISLIDE_TEST_DESTINATION, 2, 2000, [ref]$result) -eq [IntPtr]::Zero) { throw 'Cannot set the test filename' }
      $buttonId = 1
    } elseif ($env:AISLIDE_TEST_ACTION -ne 'cancel') { throw 'Unknown dialog action' }
    if (-not $controls.ContainsKey($buttonId) -or -not [AISlideDialogTest]::IsWindowEnabled($controls[$buttonId])) { throw 'Expected an enabled native dialog button' }
    if (-not [AISlideDialogTest]::PostMessage($controls[$buttonId], 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero)) { throw 'Cannot activate the native dialog button' }
    @{ filename = $filename } | ConvertTo-Json -Compress
  `], { windowsHide: true, env: { ...process.env, AISLIDE_TEST_PID: String(processId), AISLIDE_TEST_ACTION: action, AISLIDE_TEST_DESTINATION: destination } });
  return JSON.parse(stdout);
}

test('native owned direct print shows a real dialog and cancels without a job', { timeout: 90_000, skip: process.platform !== 'win32' }, async (context) => {
  const recovery = await ownedRecovery(context);
  const owned = await launchOwnedNative(context, recovery, 'http://127.0.0.1:1/v1');
  const { page, app } = owned;
  const executable = resolve(process.env.AISLIDE_NATIVE_TEST_EXE ?? 'apps/studio/src-tauri/target/debug/aislide-studio.exe');
  let armed;
  async function printDialog(action) {
    const started = performance.now();
    const diagnostic = stderr => [process.cwd(), process.env.USERPROFILE, recovery.env.AISLIDE_TEST_RECOVERY_OWNER]
      .filter(Boolean).reduce((text, value) => text.replaceAll(value, '[redacted]'), String(stderr ?? '')).slice(0, 8192);
    try {
      const { stdout, stderr } = await promisify(execFile)('powershell.exe', ['-NoProfile', '-NonInteractive', '-File',
        resolve('tools/testing/native-print-dialog.ps1'), '-ProcessId', String(app.pid), '-Executable', executable,
        '-Owner', recovery.env.AISLIDE_TEST_RECOVERY_OWNER, '-Action', action, '-ExpectedMainHandle', String(armed?.mainHandle ?? 0)], { windowsHide: true, timeout: 15_000 });
      console.log('NATIVE_PRINT_HELPER', JSON.stringify({ action, elapsedMs: performance.now() - started, stderr: diagnostic(stderr) }));
      return JSON.parse(stdout);
    } catch (error) {
      throw new Error(`Native print helper failed: ${JSON.stringify({ action, elapsedMs: performance.now() - started,
        code: error.code ?? null, killed: error.killed ?? false, signal: error.signal ?? null,
        stderr: diagnostic(error.stderr), stderrBytes: Buffer.byteLength(String(error.stderr ?? '')) })}`);
    }
  }
  armed = await printDialog('probe');
  assert.equal(armed.cancelOnly, true);
  await page.getByRole('button', { name: 'Add text', exact: true }).click();
  await page.getByRole('button', { name: 'Export PDF and images', exact: true }).click();
  const panel = page.getByRole('dialog', { name: 'Export PDF and images', exact: true });
  await panel.getByRole('button', { name: 'Prepare export', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Print PDF', exact: true })).toBeEnabled();
  await panel.getByRole('button', { name: 'Print PDF', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Open print dialog', exact: true })).toBeEnabled();
  await expect(page.locator('iframe')).toHaveCount(0);
  const printRoot = page.locator('body > [data-aislide-print]');
  const prepared = await printRoot.evaluate(element => ({ id: element.className, children: Array.from(element.children).map(child => ({ tag: child.tagName, blob: child.src.startsWith('blob:'), width: child.naturalWidth, height: child.naturalHeight })) }));
  assert.equal(prepared.children.length, 1);
  assert.ok(prepared.children.every(child => child.tag === 'IMG' && child.blob && child.width > 0 && child.height > 0));
  const events = [];
  await page.exposeFunction('recordOwnedPrintEvent', value => events.push(value));
  await page.evaluate(() => {
    window.addEventListener('beforeprint', () => window.recordOwnedPrintEvent('beforeprint'));
    window.addEventListener('afterprint', () => window.recordOwnedPrintEvent('afterprint'));
  });
  assert.deepEqual(events, []);
  const request = panel.getByRole('button', { name: 'Open print dialog', exact: true }).click({ timeout: 30_000 });
  const requested = request.then(() => null, error => error);
  let shown;
  try {
    await expect(async () => {
      const previews = owned.browser.contexts().flatMap(current => current.pages()).filter(candidate => /^(chrome|edge):\/\/print/.test(candidate.url()));
      assert.ok(previews.length <= 1, 'Ambiguous owned print preview');
      if (previews.length) {
        const preview = previews[0];
        const cancel = preview.getByRole('button', { name: /^(Cancel|キャンセル)$/ });
        await expect(cancel).toBeVisible();
        shown = { kind: 'owned CDP print preview', url: preview.url(), title: await preview.title(), action: 'Cancel only' };
        await cancel.click();
      } else shown = await printDialog('cancel');
    }).toPass({ timeout: 30_000, intervals: [250, 500, 1000] });
  } catch (error) {
    console.log('NATIVE_PRINT_DETECTION_ERROR', String(error));
    console.log('NATIVE_PRINT_DIAGNOSTIC', JSON.stringify({ armed, events, pages: owned.browser.contexts().flatMap(current => current.pages()).map(candidate => candidate.url()), status: await page.locator('.static-export [role="status"]').textContent({ timeout: 2000 }), errors: await page.locator('.static-export [role="alert"]').allTextContents() }));
    await page.screenshot({ path: '.artifacts/g35-directprint-20260919/native-diagnostic.png' });
    throw error;
  }
  const clickError = await requested;
  if (clickError) throw clickError;
  await expect(panel.getByRole('status')).toContainText('Print dialog closed');
  await expect(printRoot).toHaveCount(0);
  await expect(page.locator('style[data-aislide-print-style]')).toHaveCount(0);
  await expect(panel.getByRole('button', { name: 'Close dialog', exact: true })).toBeEnabled();
  assert.deepEqual(events, ['beforeprint', 'afterprint']);
  const proof = { armed, prepared, shown, events, browser: owned.browser.version(), status: await panel.getByRole('status').textContent(), actualPrintJob: false };
  await mkdir('.artifacts/g35-directprint-20260919', { recursive: true });
  await writeFile('.artifacts/g35-directprint-20260919/native-print-proof.json', JSON.stringify(proof, null, 2));
  console.log('NATIVE_PRINT_PROOF', JSON.stringify(proof));
  await panel.getByRole('button', { name: 'Close dialog', exact: true }).click();
  await owned.stop();
});

test('native WebView authoring and generation use shared edits, inheritance and undo', { timeout: 90_000, skip: process.platform !== 'win32' }, async (context) => {
  const root = resolve(import.meta.dirname, '..');
  const recovery = await ownedRecovery(context);
  const fixture = await startProviderFixture(await requestCore({ op: 'sample' }));
  context.after(() => fixture.close());
  const output = await mkdtemp(join(tmpdir(), 'aislide-native-workspace-output-'));
  context.after(() => rm(output, { recursive: true, force: true }));
  const profile = await mkdtemp(join(tmpdir(), 'aislide-native-generation-'));
  const reservation = createServer();
  reservation.listen(0, '127.0.0.1');
  await once(reservation, 'listening');
  const port = reservation.address().port;
  await new Promise((resolveClose, reject) => reservation.close((error) => error ? reject(error) : resolveClose()));
  const app = spawn(process.env.AISLIDE_NATIVE_TEST_EXE ?? join(root, 'apps/studio/src-tauri/target/debug/aislide-studio.exe'), [], {
    cwd: root,
    stdio: 'ignore',
    shell: false,
    env: {
      ...process.env,
      ...fixtureEnvironment(fixture.endpoint),
      ...recovery.env,
      AISLIDE_AI_TIMEOUT_SECONDS: '5',
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
      WEBVIEW2_USER_DATA_FOLDER: profile,
    },
  });
  const exited = once(app, 'close');
  let browser;
  recovery.cleanup.push(async () => {
    await browser?.close();
    if (app.exitCode === null && app.signalCode === null) app.kill();
    await exited;
    await rm(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  });
  await once(app, 'spawn');
  await expect(async () => {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
  }).toPass({ timeout: 20_000 });
  let page;
  await expect(() => {
    page = browser.contexts().flatMap((browserContext) => browserContext.pages()).find((candidate) => candidate.url().includes('tauri.localhost'));
    assert.ok(page, 'Expected the owned native AISlide WebView');
  }).toPass({ timeout: 15_000 });
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(1);
  await expect(page.locator('.slide-stage .element-hitbox')).toHaveCount(0);
  await page.getByRole('button', { name: 'New report', exact: true }).click();
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
  await page.evaluate(() => document.fonts.ready);
  const chartTool = page.getByRole('button', { name: 'Add chart', exact: true });
  await chartTool.hover();
  const tooltip = page.locator('.tool-tooltip');
  await expect(tooltip).toBeVisible();
  await expect(chartTool).not.toHaveAttribute('title');
  await expect(page.getByRole('tooltip', { name: 'Add chart', exact: true })).toHaveText('Add chart');
  const tooltipBounds = await tooltip.boundingBox();
  const toolBounds = await chartTool.boundingBox();
  assert.ok(tooltipBounds.y >= toolBounds.y + toolBounds.height || tooltipBounds.y + tooltipBounds.height <= toolBounds.y, 'The tooltip must not cover its triggering button');
  assert.equal(await tooltip.evaluate((element) => Boolean(element.closest('.ribbon'))), false);
  assert.equal(await tooltip.evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    const hit = document.elementFromPoint(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
    return bounds.left >= 0 && bounds.top >= 0 && bounds.right <= innerWidth && bounds.bottom <= innerHeight && (hit === element || element.contains(hit));
  }), true, 'The tooltip must be visible in the viewport without ribbon clipping');
  await expect(chartTool).toHaveCSS('width', '44px');
  await expect(chartTool).toHaveCSS('height', '44px');
  await expect(chartTool.locator('svg')).toHaveCSS('width', '20px');
  await page.screenshot({ path: join(root, '.artifacts/ui-polish-native.png') });
  await page.keyboard.press('Escape');
  await expect(tooltip).toHaveCount(0);
  const inspectorToggle = page.getByRole('button', { name: 'Toggle inspector', exact: true });
  await expect(inspectorToggle).toHaveAttribute('aria-pressed', 'true');
  await inspectorToggle.click();
  await expect(inspectorToggle).toHaveAttribute('aria-pressed', 'false');
  await inspectorToggle.click();
  await expect(inspectorToggle).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Edit title', exact: true }).dblclick();
  const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
  await expect(editor).toBeVisible();
  await editor.fill('Native direct editing\n\u65e5\u672c\u8a9e');
  await editor.dispatchEvent('compositionstart');
  await editor.press('Control+Enter');
  await expect(editor).toBeVisible();
  await editor.dispatchEvent('compositionend');
  await page.getByRole('button', { name: 'Apply on-slide edit', exact: true }).click();
  await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Native direct editing' })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('body')).toHaveCSS('font-family', /Noto Sans JP/);
  await page.getByRole('button', { name: 'Architecture diagram', exact: true }).click();
  const graphDialog = page.getByRole('dialog', { name: 'Architecture diagram', exact: true });
  await graphDialog.getByLabel('Graph title', { exact: true }).fill('Native architecture');
  await graphDialog.getByRole('button', { name: 'Select node api', exact: true }).click();
  await graphDialog.getByLabel('Node label', { exact: true }).fill('Native service');
  await graphDialog.getByRole('button', { name: 'Apply node', exact: true }).click();
  await expect(graphDialog.getByRole('button', { name: 'Choose node icon', exact: true })).toBeVisible();
  await graphDialog.getByRole('button', { name: 'Choose node icon', exact: true }).click();
  const graphIconPicker = graphDialog.getByRole('region', { name: 'Node icon', exact: true });
  await graphIconPicker.getByLabel('Search icons', { exact: true }).fill('server');
  await graphIconPicker.getByRole('button', { name: 'Server icon', exact: true }).click();
  await graphIconPicker.getByRole('button', { name: 'Insert icon', exact: true }).click();
  const graphIcon = graphDialog.locator('.react-flow__node[data-id="api"] .graph-node-icon img');
  await expect(graphIcon).toHaveAttribute('alt', 'Server (Lucide)');
  await expect(graphIcon).toHaveJSProperty('naturalWidth', 256);
  await graphDialog.getByRole('button', { name: 'Undo diagram edit', exact: true }).click();
  await expect(graphIcon).toHaveCount(0);
  await graphDialog.getByRole('button', { name: 'Redo diagram edit', exact: true }).click();
  await expect(graphIcon).toHaveCount(1);
  await graphDialog.getByRole('button', { name: 'Insert graph', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native service', { exact: true })).toBeVisible();
  await expect(page.locator('.slide-stage img')).toHaveAttribute('alt', 'Server (Lucide)');
  await page.getByRole('button', { name: 'Edit graph', exact: true }).click();
  await graphDialog.getByRole('button', { name: 'Select node api', exact: true }).click();
  await graphDialog.getByRole('button', { name: 'Change node icon', exact: true }).click();
  await graphIconPicker.getByLabel('Search icons', { exact: true }).fill('cloud');
  await graphIconPicker.getByRole('button', { name: 'Cloud icon', exact: true }).click();
  await graphIconPicker.getByRole('button', { name: 'Insert icon', exact: true }).click();
  await expect(graphIcon).toHaveAttribute('alt', 'Cloud (Lucide)');
  await graphDialog.getByLabel('Graph title', { exact: true }).fill('Updated native architecture');
  await graphDialog.getByRole('button', { name: 'Update graph', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Updated native architecture', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native architecture', { exact: true })).toBeVisible();
  await expect(page.locator('.slide-stage img')).toHaveAttribute('alt', 'Server (Lucide)');
  await page.screenshot({ path: join(root, '.artifacts/graph-native.png') });
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await page.getByRole('button', { name: 'Parts library', exact: true }).click();
  const partsDialog=page.getByRole('dialog', { name: 'Parts library', exact: true });
  await partsDialog.getByLabel('Part category', { exact: true }).selectOption('pyramid');
  await partsDialog.getByLabel('Part title', { exact: true }).fill('Native metadata part');
  await partsDialog.getByRole('button', { name: 'Insert part', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native metadata part', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Edit part data', exact: true }).click();
  await partsDialog.getByLabel('Part title', { exact: true }).fill('Native part updated');
  await partsDialog.getByRole('button', { name: 'Update part', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native part updated', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native metadata part', { exact: true })).toBeVisible();
  await page.screenshot({ path: join(root, '.artifacts/parts-native.png') });
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await page.getByRole('button', { name: 'Insert objects', exact: true }).click();
  await page.getByRole('button', { name: 'Insert Ellipse', exact: true }).click();
  await expect(page.locator('.canvas-workspace .preset-shape')).toHaveCount(1);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click();
  await page.getByRole('button', { name: 'Add common text', exact: true }).click();
  await page.getByLabel('Design element text', { exact: true }).fill('Native common footer');
  await page.getByRole('button', { name: 'Save design', exact: true }).click();
  await expect(page.locator('.canvas-workspace .master-graphics')).toContainText('Native common footer');
  await page.getByRole('button', { name: 'Edit theme', exact: true }).click();
  await page.getByLabel('Theme accent1', { exact: true }).fill('#b53055');
  await page.getByRole('button', { name: 'Apply theme', exact: true }).click();
  await page.getByLabel('Slide layout', { exact: true }).selectOption('title-content');
  await expect(page.locator('.canvas-workspace .element-hitbox[aria-label="Edit body"]')).toHaveCount(1);
  await page.evaluate(() => document.fonts.ready);
  await mkdir(join(root, '.artifacts'), { recursive: true });
  await page.screenshot({ path: join(root, '.artifacts/authoring-native.png') });
  for (let index = 0; index < 3; index++) await page.getByRole('button', { name: 'Undo', exact: true }).click();
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
  await expect(page.getByRole('dialog', { name: 'Generate report', exact: true }).getByRole('status')).toContainText('Generation cancelled');
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
  await expect(page.getByRole('button', { name: 'Open PPTX', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open project', exact: true })).toHaveCount(0);
  const standalone = await page.evaluate(async () => {
    const request = (request) => window.__TAURI_INTERNALS__.invoke('core_request', { operationId: crypto.randomUUID(), request });
    const report = await request({ op: 'sample' });
    const compiled = await request({ op: 'compile', report });
    compiled.deck.title = 'Native single-file PPTX';
    const document = await request({ op: 'new_document', id: 'native-single-file', deck: compiled.deck });
    const exported = await request({ op: 'export_presentation', document });
    return { base64: exported.base64, hasCheckpoint: 'checkpoint' in exported };
  });
  assert.equal(standalone.hasCheckpoint, false);
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'native-single-file.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from(standalone.base64, 'base64') });
  await page.getByRole('dialog', { name: 'Unsaved changes', exact: true }).getByRole('button', { name: 'Discard changes', exact: true }).click();
  await expect(page.locator('.document-name strong')).toHaveText('Native single-file PPTX');
  await page.getByRole('button', { name: 'Edit title', exact: true }).dblclick();
  const nativeEditor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
  await nativeEditor.fill('One native PPTX\nDirect edit');
  await nativeEditor.press('Control+Enter');
  await expect(nativeEditor).toHaveCount(0);
  await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'One native PPTX' })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.canvas-workspace .slide-text').filter({ hasText: 'Quarterly performance' })).toBeVisible();
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles({ name: 'protected.pptx', mimeType: 'application/vnd.openxmlformats-officedocument.presentationml.presentation', buffer: Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) });
  await expect(page.getByRole('alert')).toContainText('encrypted');
  await expect(page.getByRole('alert')).not.toContainText('checkpoint');
  await expect(page.locator('.document-name strong')).toHaveText('Native single-file PPTX');
  await page.screenshot({ path: join(root, '.artifacts/single-pptx-native.png') });
  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click();
  await page.getByRole('button', { name: 'New presentation', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(1);
  await expect(page.locator('.slide-stage .element-hitbox')).toHaveCount(0);
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  const assets = page.getByRole('dialog', { name: 'Icons and assets', exact: true });
  await assets.getByLabel('Search icons', { exact: true }).fill('database');
  await assets.getByRole('button', { name: 'Database icon', exact: true }).click();
  await nativeTransaction(page, () => assets.getByRole('button', { name: 'Insert icon', exact: true }).click());
  await expect(page.locator('.slide-stage img')).toHaveCount(1);
  await page.locator('.layer-list button').first().click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Duplicate element', exact: true }).click();
  await expect(page.locator('.slide-stage img')).toHaveCount(2);
  await page.getByRole('button', { name: 'New slide', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(2);
  await page.getByRole('button', { name: /^Slide 2:/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Duplicate slide', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(3);
  await page.getByRole('button', { name: /^Slide 3:/ }).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Delete slide', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(2);
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(3);
  await page.getByRole('button', { name: 'Save as', exact: true }).click();
  await page.getByLabel('PPTX filename', { exact: true }).fill('Native workspace.pptx');
  await page.getByRole('dialog').getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.locator('.dirty-indicator')).toBeVisible();
  await promisify(execFile)('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `
    $ErrorActionPreference = 'Stop'
    Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class AISlideCloseTest { [DllImport("user32.dll", SetLastError = true)] [return: MarshalAs(UnmanagedType.Bool)] public static extern bool PostMessage(IntPtr handle, uint message, IntPtr word, IntPtr data); }'
    $owned = Get-Process -Id ${app.pid} -ErrorAction Stop
    if ($owned.MainWindowHandle -eq 0) { throw 'Owned test window not found' }
    if (-not [AISlideCloseTest]::PostMessage($owned.MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) { throw 'Owned close request failed' }
  `], { windowsHide: true });
  const unsaved = page.getByRole('dialog', { name: 'Unsaved changes', exact: true });
  await expect(unsaved).toBeVisible();
  await unsaved.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.locator('.thumbnail')).toHaveCount(3);
  assert.equal(app.exitCode, null, 'Cancel must preserve the native window');
  await page.getByRole('button', { name: /^Slide 1:/ }).click();
  await nativeTransaction(page, () => page.locator('.canvas-workspace').evaluate((element) => {
    const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><circle cx="16" cy="16" r="12" fill="#007a4d"/></svg>';
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([svg], 'native-drop.svg', { type: 'image/svg+xml' }));
    element.dispatchEvent(new DragEvent('drop', { dataTransfer, bubbles: true, cancelable: true, clientX: 500, clientY: 400 }));
  }));
  await expect(page.locator('.slide-stage img')).toHaveCount(3);
  await nativeTransaction(page, () => page.locator('.canvas-workspace').evaluate((element) => {
    const clipboardData = new DataTransfer();
    clipboardData.setData('text/plain', '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="24" height="24" fill="#0017c1"/></svg>');
    element.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
  }));
  await expect(page.locator('.slide-stage img')).toHaveCount(4);
  await page.getByRole('button', { name: 'Save as', exact: true }).click();
  const saveForm = page.getByRole('dialog', { name: 'Save presentation', exact: true });
  await saveForm.getByLabel('PPTX filename', { exact: true }).fill('Native workspace.pptx');
  await saveForm.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  assert.equal((await nativeSaveDialog(app.pid, 'cancel')).filename, 'Native workspace.pptx');
  await expect(saveForm.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  await expect(page.locator('.dirty-indicator')).toBeVisible();
  assert.deepEqual(await readdir(output), []);
  await saveForm.getByRole('button', { name: 'Save PPTX', exact: true }).click();
  const savedPath = join(output, 'Native workspace.pptx');
  assert.equal((await nativeSaveDialog(app.pid, 'save', savedPath)).filename, 'Native workspace.pptx');
  await expect(saveForm).toHaveCount(0);
  await expect(page.locator('.dirty-indicator')).toHaveCount(0);
  assert.deepEqual(await readdir(output), ['Native workspace.pptx']);
  await page.getByLabel('Open PPTX file', { exact: true }).setInputFiles(savedPath);
  await expect(page.locator('.thumbnail')).toHaveCount(3);
  await expect(page.locator('.slide-stage img')).toHaveCount(4);
  await expect(page.locator('.document-name')).toContainText('Native workspace.pptx');
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.screenshot({ path: join(root, '.artifacts/workspace-native.png') });
});

test('native bounded previews, modern controls and private recovery process restart', { timeout: 180_000, skip: process.platform !== 'win32' }, async (context) => {
  const recovery = await ownedRecovery(context);
  const fixture = await startProviderFixture(await requestCore({ op: 'sample' }));
  context.after(() => fixture.close());
  const first = await launchOwnedNative(context, recovery, fixture.endpoint);
  const { page } = first;
  await context.test('eight WordArt presets and statistical chart controls render through native IPC', async (scenario) => {
    scenario.after(async () => {
      const close = page.getByRole('button', { name: 'Close dialog', exact: true });
      if (await close.isVisible()) await close.click();
    });
    await page.getByRole('button', { name: 'Add text', exact: true }).click();
    await page.locator('.slide-stage .element-hitbox').first().dblclick();
    const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
    await editor.fill('Native shaped WordArt');
    await editor.press('Control+Enter');
    await page.getByRole('button', { name: 'Advanced object settings', exact: true }).click();
    const warp = page.getByRole('combobox', { name: 'Text warp', exact: true });
    assert.equal(await warp.count(), 1, await page.getByRole('dialog').ariaSnapshot());
    assert.deepEqual(await warp.locator('option').evaluateAll(options => options.map(option => option.value).filter(Boolean)), ['arch_up','arch_down','wave1','wave2','inflate','deflate','slant_up','slant_down']);
    await warp.selectOption('arch_down');
    await page.getByRole('button', { name: 'Apply visual style', exact: true }).click();
    const preview = page.locator('.slide-stage [data-core-preview]').first();
    await expect(preview).toHaveAttribute('data-core-preview', 'ready');
    const wordartSvg = decodeURIComponent((await preview.locator('img').getAttribute('src')).split(',').slice(1).join(','));
    assert.match(wordartSvg, /<path/);
    assert.match(wordartSvg, /Native shaped WordArt/);
    await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
    await page.getByRole('button', { name: 'Add chart', exact: true }).click();
    await page.getByRole('button', { name: 'Advanced object settings', exact: true }).click();
    await page.getByRole('combobox', { name: 'Chart kind', exact: true }).selectOption('line');
    await page.locator('summary').filter({ hasText: /^Series$/ }).click();
    const trendline = page.getByRole('combobox', { name: 'Trendline', exact: true });
    await trendline.selectOption('linear');
    await page.getByRole('combobox', { name: 'Error bars', exact: true }).selectOption('standard_deviation');
    await page.getByRole('spinbutton', { name: 'Error value', exact: true }).fill('1');
    await page.getByRole('button', { name: 'Apply chart', exact: true }).click();
    try { await expect(page.locator('.slide-stage [data-core-preview="ready"]')).toHaveCount(2); }
    catch (error) {
      console.log('NATIVE_CHART_DIAGNOSTIC', JSON.stringify({ dialog: await page.getByRole('dialog').ariaSnapshot(),
        previews: await page.locator('.slide-stage [data-core-preview]').evaluateAll(elements => elements.map(element => ({ state: element.dataset.corePreview, text: element.textContent }))) }));
      throw error;
    }
    await page.getByRole('button', { name: 'Close dialog', exact: true }).click();
    await expect(page.locator('.slide-stage [data-core-preview="error"]')).toHaveCount(0);
    for (const image of await page.locator('.slide-stage [data-core-preview] img').all()) await image.evaluate(image => image.decode());
  });
  await context.test('foreground network lifecycle, preview and transaction lanes use real IPC', async () => {
    const observed = fixture.nextRequest();
    await page.evaluate(() => {
      window.nativeGenerationId = crypto.randomUUID();
      window.nativeGeneration = window.__TAURI_INTERNALS__.invoke('core_request', { operationId: window.nativeGenerationId,
        request: { op: 'generate', input: { prompt: 'WAIT_FOR_CANCELLATION bounded native proof', source_text: '', slide_count: 3 } } }).then(() => 'unexpected success', error => String(error));
    });
    const network = await observed;
    const projection = await page.evaluate(async () => window.__TAURI_INTERNALS__.invoke('core_request', { operationId: crypto.randomUUID(), request: {
      op: 'compute_chart_presentation', kind: 'line', categories: ['1','2','3'], series: [{ name: 'Synthetic', values: [3,5,7], color: '087F73', trendline: { kind: 'linear', display_equation: true, display_r_squared: true } }], options: { legend: 'hidden' },
    } }));
    assert.ok(projection.series[0].trend);
    const cancellation = await page.evaluate(async () => ({ acknowledged: await window.__TAURI_INTERNALS__.invoke('cancel_core_request', { operationId: window.nativeGenerationId }), result: await window.nativeGeneration }));
    assert.equal(cancellation.acknowledged, true);
    assert.match(cancellation.result, /cancelled/i);
    await network.disconnected;
    const overlap = await page.evaluate(async () => {
      const request = request => window.__TAURI_INTERNALS__.invoke('core_request', { operationId: crypto.randomUUID(), request });
      const document = await request({ op: 'create_presentation', id: 'native-overlap', title: 'Before' });
      window.nativeOverlapDocument = document;
      let previewCompleted = false;
      const pending = request({ op: 'render_element_preview', element: { type: 'text', id: 'heavy-owned-preview', x: 0, y: 0, width: 1280, height: 720,
        text: 'Native glyph outlines '.repeat(40), font_size: 24, color: '087F73', bold: true, visual: { text_warp: 'arch_down' } } }).then(result => { previewCompleted = true; return result; });
      const transaction = await request({ op: 'transaction', document, transaction: { expected_revision: document.revision, expected_hash: document.hash, operations: [{ op: 'replace', path: '/deck/title', value: 'Foreground changed' }] } });
      const transactionWhilePreviewPending = !previewCompleted;
      const result = await pending;
      return { transactionWhilePreviewPending, title: transaction.document.deck.title, svg: result.svg.length };
    });
    assert.equal(overlap.title, 'Foreground changed');
    assert.ok(overlap.svg > 100);
    assert.equal(overlap.transactionWhilePreviewPending, true, 'Native foreground transaction must finish while the owned SVG preview remains pending');
    await page.evaluate(() => {
      window.nativeSaveResult = window.__TAURI_INTERNALS__.invoke('save_presentation', { document: window.nativeOverlapDocument, operationId: crypto.randomUUID(), filename: 'Owned native cancel.pptx' });
      window.nativeSavePreview = window.__TAURI_INTERNALS__.invoke('core_request', { operationId: crypto.randomUUID(), request: {
        op: 'compute_chart_presentation', kind: 'line', categories: ['1','2'], series: [{ name: 'Synthetic', values: [3,5], color: '087F73' }], options: {},
      } });
    });
    assert.equal((await nativeSaveDialog(first.app.pid, 'cancel')).filename, 'Owned native cancel.pptx');
    assert.equal(await page.evaluate(() => window.nativeSaveResult), null);
    assert.ok((await page.evaluate(() => window.nativeSavePreview)).series.length);
    console.log('NATIVE_PREVIEW_PROOF', JSON.stringify(overlap));
  });
  await context.test('committed text Undo and Redo survive an owned process restart with a fresh WebView profile', async () => {
    await page.getByRole('button', { name: 'New presentation', exact: true }).click();
    await page.getByRole('button', { name: 'Discard changes', exact: true }).click();
    await page.getByRole('button', { name: 'Add text', exact: true }).click();
    await page.locator('.slide-stage .element-hitbox').first().dblclick();
    const editor = page.getByRole('textbox', { name: 'Slide text editor', exact: true });
    await editor.fill('Owned recovery second state');
    await editor.press('Control+Enter');
    await expect(editor).toHaveCount(0);
    await expect(page.locator('.slide-stage .slide-text')).toHaveText('Owned recovery second state');
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(page.locator('.slide-stage .slide-text')).toHaveText('New text');
    const original = await page.locator('.slide-stage .slide-text').innerText();
    await page.getByRole('button', { name: 'Local recovery', exact: true }).click();
    const panel = page.getByRole('dialog', { name: 'Local recovery', exact: true });
    await expect(panel.getByRole('checkbox')).not.toBeChecked();
    await panel.getByRole('checkbox').click();
    await expect(panel.getByRole('checkbox')).toBeChecked();
    await expect(page.getByRole('status').filter({ hasText: 'Recovery copy saved' })).toBeVisible();
    await panel.getByRole('button', { name: 'Refresh recovery', exact: true }).click();
    await expect(panel.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true })).toBeVisible();
    const committed = await page.evaluate(async () => {
      const invoke = request => window.__TAURI_INTERNALS__.invoke('recovery_request', { request, operationId: crypto.randomUUID() });
      const state = await invoke({ op: 'state' });
      const envelope = await invoke({ op: 'load', id: state.entries[0].id, expected_generation: state.generation });
      return { past: envelope.past.length, future: envelope.future.length, generation: state.generation, id: state.entries[0].id };
    });
    assert.equal(committed.past, 1);
    assert.equal(committed.future, 1);
    assert.ok((await readdir(join(recovery.directory, 'recovery-v2'))).length > 0);
    await first.stop();
    const second = await launchOwnedNative(context, recovery, fixture.endpoint);
    assert.notEqual(first.profile, second.profile);
    assert.notEqual(first.app.pid, second.app.pid);
    await second.page.getByRole('button', { name: 'Local recovery', exact: true }).click();
    const restoredPanel = second.page.getByRole('dialog', { name: 'Local recovery', exact: true });
    await expect(restoredPanel.getByRole('checkbox')).toBeChecked();
    await restoredPanel.getByRole('button', { name: 'Restore Untitled presentation.pptx', exact: true }).click();
    await expect(restoredPanel).toHaveCount(0);
    await expect(second.page.getByText('Recovered presentation / Unsaved', { exact: true })).toBeVisible();
    await expect(second.page.locator('.slide-stage .slide-text')).toHaveText(original);
    await expect(second.page.getByRole('button', { name: 'Undo', exact: true })).toBeEnabled();
    await expect(second.page.getByRole('button', { name: 'Redo', exact: true })).toBeEnabled();
    await second.page.getByRole('button', { name: 'Redo', exact: true }).click();
    await expect(second.page.locator('.slide-stage .slide-text')).toHaveText('Owned recovery second state');
    await second.page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(second.page.locator('.slide-stage .slide-text')).toHaveText(original);
    await second.page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect(second.page.locator('.slide-stage .element-hitbox')).toHaveCount(0);
    console.log('NATIVE_RESTART_PROOF', JSON.stringify({ ...committed, firstPid: first.app.pid, secondPid: second.app.pid, differentWebViewProfiles: true, privateRecovery: true }));
    await second.stop();
  });
});
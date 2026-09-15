import test from 'node:test';
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readdir, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { promisify } from 'node:util';
import { chromium, expect } from '@playwright/test';
import { requestCore } from './core-client.mjs';
import { startProviderFixture, fixtureEnvironment } from './testing/provider-fixture.mjs';

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

test('native WebView authoring and generation use shared edits, inheritance and undo', { timeout: 90_000, skip: process.platform !== 'win32' }, async (context) => {
  const root = resolve(import.meta.dirname, '..');
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
  let page;
  await expect(() => {
    page = browser.contexts().flatMap((browserContext) => browserContext.pages()).find((candidate) => candidate.url().includes('tauri.localhost'));
    assert.ok(page, 'Expected the owned native AISlide WebView');
  }).toPass({ timeout: 15_000 });
  await expect(page.getByRole('button', { name: /^Slide \d+:/ })).toHaveCount(12);
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
  await graphDialog.getByRole('button', { name: 'Insert graph', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native service', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Edit graph', exact: true }).click();
  await graphDialog.getByLabel('Graph title', { exact: true }).fill('Updated native architecture');
  await graphDialog.getByRole('button', { name: 'Update graph', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Updated native architecture', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native architecture', { exact: true })).toBeVisible();
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
  await assets.getByRole('button', { name: 'Insert icon', exact: true }).click();
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
  await page.locator('.canvas-workspace').evaluate((element) => {
    const svg = '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><circle cx="16" cy="16" r="12" fill="#007a4d"/></svg>';
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(new File([svg], 'native-drop.svg', { type: 'image/svg+xml' }));
    element.dispatchEvent(new DragEvent('drop', { dataTransfer, bubbles: true, cancelable: true, clientX: 500, clientY: 400 }));
  });
  await expect(page.locator('.slide-stage img')).toHaveCount(3);
  await page.locator('.canvas-workspace').evaluate((element) => {
    const clipboardData = new DataTransfer();
    clipboardData.setData('text/plain', '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="24" height="24" fill="#0017c1"/></svg>');
    element.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
  });
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
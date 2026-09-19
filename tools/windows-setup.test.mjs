import test from 'node:test';
import assert from 'node:assert/strict';
import { copyFile, mkdir, mkdtemp, open, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { constants } from 'node:fs';
import { execFile } from 'node:child_process';
import { createServer } from 'node:net';
import { once } from 'node:events';
import { randomUUID } from 'node:crypto';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { promisify } from 'node:util';

async function assertGuiSubsystem(path) {
  const binary = await open(path, 'r');
  try {
    const dos = Buffer.alloc(64);
    assert.equal((await binary.read(dos, 0, dos.length, 0)).bytesRead, dos.length);
    assert.equal(dos.toString('ascii', 0, 2), 'MZ');
    const header = Buffer.alloc(94);
    assert.equal((await binary.read(header, 0, header.length, dos.readUInt32LE(60))).bytesRead, header.length);
    assert.equal(header.toString('ascii', 0, 4), 'PE\0\0');
    assert.ok([0x10b, 0x20b].includes(header.readUInt16LE(24)));
    assert.equal(header.readUInt16LE(92), 2, 'Studio must use the Windows GUI subsystem, including debug setup builds');
  } finally {
    await binary.close();
  }
}

test('Windows setup registers a current-user Start Menu app without changing file associations', async () => {
  const root = resolve(import.meta.dirname, '../apps/studio/src-tauri');
  const config = JSON.parse(await readFile(resolve(root, 'tauri.windows.conf.json'), 'utf8'));
  assert.equal(config.bundle.active, true);
  assert.deepEqual(config.bundle.targets, ['nsis']);
  assert.equal(config.bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(config.bundle.windows.nsis.startMenuFolder, 'AISlide');
  assert.deepEqual(config.bundle.windows.nsis.languages, ['English', 'Japanese']);
  assert.deepEqual(config.bundle.icon, ['icons/icon.ico']);
  assert.ok((await stat(resolve(root, config.bundle.icon[0]))).size > 0);
  assert.equal(config.bundle.fileAssociations, undefined);
  assert.equal(config.bundle.createUpdaterArtifacts, undefined);
  assert.equal(config.bundle.windows.nsis.template, undefined);
  assert.equal(config.bundle.windows.nsis.installerHooks, 'windows/installer-hooks.nsh');
  const hooks = await readFile(resolve(root, config.bundle.windows.nsis.installerHooks), 'utf8');
  assert.match(hooks, /FindProcessCurrentUser/);
  assert.match(hooks, /NSIS_HOOK_PREINSTALL/);
  assert.match(hooks, /NSIS_HOOK_PREUNINSTALL/);
  assert.match(hooks, /SetErrorLevel 2/);
  assert.doesNotMatch(hooks, /KillProcess|taskkill/i);
});

test('setup commands reuse the isolated toolchain and installed Tauri CLI', async () => {
  const root = resolve(import.meta.dirname, '..');
  const manifest = JSON.parse(await readFile(resolve(root, 'package.json'), 'utf8'));
  assert.equal(manifest.scripts['setup:build'], 'node tools/windows-setup.mjs');
  assert.equal(manifest.scripts['setup:bundle'], 'node tools/windows-setup.mjs --bundle');
  const { stdout } = await promisify(execFile)(process.execPath, [resolve(root, 'tools/cargo.mjs'), 'tauri', 'bundle', '--help'], { cwd: root, windowsHide: true });
  assert.match(stdout, /Generate bundles and installers/);
  assert.match(stdout, /--debug/);
});

test('setup builds host artifacts in a separate target directory before bundling', async () => {
  const { setupCommands } = await import('./windows-setup.mjs');
  const root = resolve(import.meta.dirname, '..');
  const commands = setupCommands({ root, host: 'x86_64-pc-windows-gnu', npmCli: 'npm-cli.js', debug: true, noSign: true });
  assert.equal(commands.length, 3);
  assert.deepEqual(commands[0].args.slice(-2), ['run', 'build']);
  assert.ok(commands[1].args.includes('--locked'));
  assert.ok(!commands[1].args.includes('--target'));
  const location = commands[1].args[commands[1].args.indexOf('--target-dir') + 1];
  assert.equal(location, resolve(root, 'apps/studio/src-tauri/target/x86_64-pc-windows-gnu'));
  assert.ok(commands[2].args.includes('--debug'));
  assert.ok(commands[2].args.includes('--no-sign'));
  assert.equal(commands[2].args[commands[2].args.indexOf('--target') + 1], 'x86_64-pc-windows-gnu');
  const release = setupCommands({ root, host: 'x86_64-pc-windows-msvc', npmCli: 'npm-cli.js' });
  assert.ok(release[1].args.includes('--release'));
  assert.ok(!release[2].args.includes('--debug'));
  assert.ok(!release[2].args.includes('--no-sign'));
  const bundle = setupCommands({ root, host: 'x86_64-pc-windows-msvc', npmCli: 'npm-cli.js', bundleOnly: true });
  assert.deepEqual(bundle, [release[2]]);
  assert.throws(() => setupCommands({ root, host: '../unknown', npmCli: 'npm-cli.js' }), /Windows Rust target/);
});

test('setup rejects target overrides before building or packaging stale artifacts', { skip: process.platform !== 'win32' }, async () => {
  for (const name of ['CARGO_BUILD_TARGET', 'CARGO_TARGET_DIR']) {
    await assert.rejects(() => promisify(execFile)(process.execPath, [resolve(import.meta.dirname, 'windows-setup.mjs'), '--dry-run'], { env: { ...process.env, npm_execpath: process.env.npm_execpath ?? 'npm-cli.js', [name]: 'redirected-target' }, windowsHide: true }), (error) => error.code === 1 && error.stderr.includes(name));
  }
});

test('NSIS installs a working Start Menu shortcut and removes only its own files', { skip: process.platform !== 'win32' || !process.argv.includes('--installed'), timeout: 300_000 }, async (context) => {
  const started = performance.now();
  const checkpoint = (stage) => context.diagnostic(`Setup ${stage}: ${Math.round(performance.now() - started)} ms elapsed.`);
  const { chromium, expect } = await import('@playwright/test');
  const { icons: lucideIcons } = await import('lucide-react');
  const root = resolve(import.meta.dirname, '..');
  const execute = promisify(execFile);
  const powershell = async (script, environment = {}) => (await execute('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', `$ErrorActionPreference = 'Stop'\n${script}`], { windowsHide: true, env: { ...process.env, ...environment } })).stdout;
  const version = (await execute(process.execPath, [resolve(root, 'tools/cargo.mjs'), 'version', '--verbose'], { cwd: root, windowsHide: true })).stdout;
  const host = version.match(/^host:\s+(\S+)$/m)?.[1];
  assert.match(host, /^(x86_64|i686|aarch64)-pc-windows-(gnu|msvc)$/);
  const profile = process.argv.includes('--release') ? 'release' : 'debug';
  const binaryDirectory = resolve(root, 'apps/studio/src-tauri/target', host, profile);
  await assertGuiSubsystem(join(binaryDirectory, 'aislide-studio.exe'));
  const tag = randomUUID().replaceAll('-', '').slice(0, 10);
  const name = `AISlide Setup Verification ${tag}`;
  const binaryName = `aislide-setup-check-${tag}`;
  const temporary = await mkdtemp(join(tmpdir(), 'aislide-setup-'));
  const destination = join(temporary, 'Application with spaces');
  const alias = join(binaryDirectory, `${binaryName}.exe`);
  const setup = join(binaryDirectory, 'bundle/nsis', `${name}_0.1.0_${host.startsWith('aarch64') ? 'arm64' : host.startsWith('i686') ? 'x86' : 'x64'}-setup.exe`);
  const folders = JSON.parse(await powershell("@{ Programs = [Environment]::GetFolderPath('Programs'); Desktop = [Environment]::GetFolderPath('DesktopDirectory') } | ConvertTo-Json -Compress"));
  const shortcut = join(folders.Programs, name, `${name}.lnk`);
  const desktopShortcut = join(folders.Desktop, `${name}.lnk`);
  const uninstaller = join(destination, 'uninstall.exe');
  let browser;
  let applicationId;
  const stopOwned = async () => {
    await browser?.close(); browser = undefined;
    if (applicationId) {
      await powershell("$owned = Get-Process -Id $env:AISLIDE_SETUP_PID -ErrorAction SilentlyContinue; if ($owned) { try { $null = $owned.Handle; if ($owned.Path -ne $env:AISLIDE_SETUP_TARGET) { throw 'Test process identity changed' }; Stop-Process -InputObject $owned -ErrorAction Stop; if (-not $owned.WaitForExit(15000)) { throw 'Test-owned application did not exit within 15000 ms' } } finally { $owned.Dispose() } }", { AISLIDE_SETUP_PID: String(applicationId), AISLIDE_SETUP_TARGET: join(destination, `${binaryName}.exe`) });
      checkpoint('owned process exited');
    }
    applicationId = undefined;
  };
  const uninstall = async () => {
    if (await stat(uninstaller).catch(() => null)) {
      await execute(uninstaller, ['/S', `_?=${destination}`], { windowsHide: true, windowsVerbatimArguments: true });
      checkpoint('uninstaller completed');
    }
    await expect(async () => { assert.equal(await stat(shortcut).catch(() => null), null); assert.equal(await stat(desktopShortcut).catch(() => null), null); }).toPass({ timeout: 15000 });
  };
  context.after(async () => {
    await stopOwned();
    await uninstall();
    await rm(alias, { force: true });
    await rm(setup, { force: true });
    await rm(temporary, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  });
  await copyFile(join(binaryDirectory, 'aislide-studio.exe'), alias, constants.COPYFILE_EXCL);
  const config = { productName: name, mainBinaryName: binaryName, identifier: `dev.aislide.setupcheck.${tag}`, bundle: { windows: { nsis: { startMenuFolder: name, compression: 'zlib' } } } };
  await execute(process.execPath, [resolve(root, 'tools/cargo.mjs'), 'tauri', 'bundle', '--bundles', 'nsis', '--target', host, ...profile === 'debug' ? ['--debug'] : [], '--no-sign', '--config', JSON.stringify(config)], { cwd: root, windowsHide: true, maxBuffer: 4 * 1024 * 1024 });
  checkpoint('bundled');
  await execute(setup, ['/S', `/D=${destination}`], { windowsHide: true, windowsVerbatimArguments: true });
  checkpoint('installed');
  const installedBinary = join(destination, `${binaryName}.exe`);
  assert.ok((await stat(installedBinary)).size > 0);
  await assertGuiSubsystem(installedBinary);
  assert.ok((await stat(shortcut)).size > 0);
  const link = JSON.parse(await powershell("$shell = New-Object -ComObject WScript.Shell; $link = $shell.CreateShortcut($env:AISLIDE_SETUP_SHORTCUT); @{ Target = $link.TargetPath; Arguments = $link.Arguments } | ConvertTo-Json -Compress", { AISLIDE_SETUP_SHORTCUT: shortcut }));
  assert.equal(link.Target.toLowerCase(), installedBinary.toLowerCase());
  assert.equal(link.Arguments, '');
  const reservation = createServer().listen(0, '127.0.0.1');
  await once(reservation, 'listening');
  const port = reservation.address().port;
  await new Promise((done) => reservation.close(done));
  await powershell('Start-Process -FilePath $env:AISLIDE_SETUP_SHORTCUT', { AISLIDE_SETUP_SHORTCUT: shortcut, WEBVIEW2_USER_DATA_FOLDER: join(temporary, 'webview'), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1` });
  await expect(async () => {
    const owned = JSON.parse(await powershell("Get-Process -Name $env:AISLIDE_SETUP_PROCESS -ErrorAction Stop | Select-Object Id,Path | ConvertTo-Json -Compress", { AISLIDE_SETUP_PROCESS: binaryName }));
    assert.equal(owned.Path.toLowerCase(), installedBinary.toLowerCase()); applicationId = owned.Id;
  }).toPass({ timeout: 15000 });
  await expect(async () => { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); }).toPass({ timeout: 15000 });
  let page;
  await expect(() => { page = browser.contexts().flatMap((entry) => entry.pages()).find((entry) => entry.url().includes('tauri.localhost')); assert.ok(page); }).toPass({ timeout: 15000 });
  await expect(page.locator('.thumbnail')).toHaveCount(1);
  await expect(page.locator('.slide-stage .element-hitbox')).toHaveCount(0);
  await expect(page.locator('.document-name strong')).toHaveText('Untitled presentation');
  await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
  await expect(page.getByRole('alert')).toHaveCount(0);
  checkpoint('launched');
  await mkdir(resolve(root, '.artifacts'), { recursive: true });
  await page.screenshot({ path: resolve(root, '.artifacts/start-menu-installed.png') });
  await page.getByRole('button', { name: 'Insert icons', exact: true }).click();
  const assets = page.getByRole('dialog', { name: 'Icons and assets', exact: true });
  await expect(assets.locator('.icon-grid button')).toHaveCount(60);
  await expect(assets.getByRole('status')).toHaveText(`${Object.keys(lucideIcons).length} icons`);
  await assets.getByLabel('Icon category', { exact: true }).selectOption('development');
  await assets.getByLabel('Search icons', { exact: true }).fill('database');
  await assets.getByRole('button', { name: 'Database icon', exact: true }).click();
  await assets.screenshot({ path: resolve(root, '.artifacts/icons-expanded-native.png') });
  await assets.getByRole('button', { name: 'Insert icon', exact: true }).click();
  await expect(page.locator('.slide-stage img')).toHaveAttribute('alt', 'Database (Lucide)');
  await expect(page.locator('.slide-stage img')).toHaveJSProperty('complete', true);
  assert.ok(await page.locator('.slide-stage img').evaluate((image) => image.naturalWidth > 0));
  const picture = page.locator('.slide-stage [data-element-id]').filter({ has: page.locator('img') });
  for (const operation of ['move', 'resize']) {
    const hitbox = picture.getByRole('button', { name: /^Edit / });
    await hitbox.click();
    const beforeBounds = await picture.boundingBox();
    const control = operation === 'move' ? hitbox : picture.getByRole('button', { name: /^Resize / });
    const pointer = await control.boundingBox();
    await page.evaluate(() => {
      const endpoint = window.__TAURI_INTERNALS__.convertFileSrc('core_request', 'ipc');
      const original = window.fetch;
      let release;
      const waiting = new Promise((resolve) => { release = resolve; });
      const gate = { started: false, pending: Promise.resolve(), release, restore: () => { window.fetch = original; delete window.__aislideDragGate; } };
      window.__aislideDragGate = gate;
      window.fetch = (input, options) => {
        if (input === endpoint && typeof options?.body === 'string' && JSON.parse(options.body).request?.op === 'transaction' && !gate.started) {
          gate.started = true;
          gate.pending = waiting.then(() => original.call(window, input, options));
          return gate.pending;
        }
        return original.call(window, input, options);
      };
    });
    try {
      await page.mouse.move(pointer.x + pointer.width / 2, pointer.y + pointer.height / 2);
      await page.mouse.down();
      await page.mouse.move(pointer.x + pointer.width / 2 + 72, pointer.y + pointer.height / 2 + 36, { steps: 12 });
      const preview = await picture.boundingBox();
      assert.ok(preview[operation === 'move' ? 'x' : 'width'] - beforeBounds[operation === 'move' ? 'x' : 'width'] > 60);
      await page.mouse.up();
      await expect.poll(() => page.evaluate(() => window.__aislideDragGate.started)).toBe(true);
      await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeDisabled();
      const jumps = await picture.evaluate(async (element, target) => {
        const samples = [];
        for (let index = 0; index < 4; index++) {
          await new Promise((resolve) => requestAnimationFrame(resolve));
          const bounds = element.getBoundingClientRect();
          samples.push(Math.max(...['x', 'y', 'width', 'height'].map((key) => Math.abs(bounds[key] - target[key]))));
        }
        return samples;
      }, preview);
      assert.ok(Math.max(...jumps) < 1, `${operation} preview jumped while the native transaction was pending`);
      await page.screenshot({ path: resolve(root, `.artifacts/drag-native-${operation}.png`) });
      context.diagnostic(`Native ${operation} pending preview: maximum jump ${Math.max(...jumps).toFixed(3)} px.`);
    } finally {
      await page.evaluate(async () => { const gate = window.__aislideDragGate; if (gate) { gate.restore(); gate.release(); await gate.pending; } });
    }
    await expect(page.getByRole('button', { name: 'Save PPTX', exact: true })).toBeEnabled();
    await page.getByRole('button', { name: 'Undo', exact: true }).click();
    await expect.poll(async () => Math.abs((await picture.boundingBox())[operation === 'move' ? 'x' : 'width'] - beforeBounds[operation === 'move' ? 'x' : 'width'])).toBeLessThan(1);
  }
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage img')).toHaveCount(0);
  const profiles = await page.evaluate(() => window.__TAURI_INTERNALS__.invoke('core_request', { operationId: crypto.randomUUID(), request: { op: 'best_practice_profiles' } }));
  checkpoint('drag checks');
  assert.equal(profiles.profiles.length, 4);
  await page.getByRole('button', { name: 'Parts library', exact: true }).click();
  const parts = page.getByRole('dialog', { name: 'Parts library', exact: true });
  await parts.getByLabel('Part category', { exact: true }).selectOption('cycle');
  await parts.getByRole('button', { name: 'Segmented cycle', exact: true }).click();
  await parts.getByLabel('Part title', { exact: true }).fill('Native cycle verification');
  await parts.getByRole('button', { name: 'Insert part', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native cycle verification', { exact: true })).toBeVisible();
  const segments = await page.locator('.slide-stage svg polygon').evaluateAll((nodes) => nodes.filter((node) => node.getAttribute('points').trim().split(/\s+/).length > 40).length);
  assert.equal(segments, 4);
  await page.screenshot({ path: resolve(root, '.artifacts/parts-refresh-native.png') });
  checkpoint('part inserted');
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.locator('.slide-stage').getByText('Native cycle verification', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Edit masters and layouts', exact: true }).click();
  const design = page.getByRole('dialog', { name: 'Masters and layouts', exact: true });
  await design.getByRole('button', { name: 'Browse presets', exact: true }).click();
  await expect(design.locator('.master-preset-choice')).toHaveCount(7);
  await design.getByRole('button', { name: 'Preset Trusted report', exact: true }).click();
  await design.screenshot({ path: resolve(root, '.artifacts/master-presets-native.png') });
  await design.getByRole('button', { name: 'Use preset', exact: true }).click();
  await page.getByLabel('Slide layout', { exact: true }).selectOption('preset-cover');
  await expect(page.locator('.slide-stage').getByText('Presentation title', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expect(page.getByLabel('Slide layout', { exact: true })).toHaveValue('blank');
  checkpoint('preset checks');
  const before = await stat(installedBinary);
  await assert.rejects(() => execute(setup, ['/S', `/D=${destination}`], { windowsHide: true, windowsVerbatimArguments: true }), (error) => error.code === 2);
  await assert.rejects(() => execute(uninstaller, ['/S', `_?=${destination}`], { windowsHide: true, windowsVerbatimArguments: true }), (error) => error.code === 2);
  assert.equal((await stat(installedBinary)).mtimeMs, before.mtimeMs);
  await expect(page.locator('.thumbnail')).toHaveCount(1);
  checkpoint('running guards');
  await writeFile(join(destination, 'keep-user-data.txt'), 'Synthetic user-data preservation check', { flag: 'wx' });
  await stopOwned();
  await uninstall();
  checkpoint('uninstalled');
  assert.equal(await readFile(join(destination, 'keep-user-data.txt'), 'utf8'), 'Synthetic user-data preservation check');
  const registration = JSON.parse(await powershell("@{ Exists = Test-Path -LiteralPath ('HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\' + $env:AISLIDE_SETUP_NAME) } | ConvertTo-Json -Compress", { AISLIDE_SETUP_NAME: name }));
  assert.equal(registration.Exists, false);
  context.diagnostic('Start Menu target, actual shortcut launch, running-app install/uninstall guards, shortcut removal and unrelated-file preservation verified.');
});
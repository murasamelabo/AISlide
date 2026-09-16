import { execFileSync, spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export function setupCommands({ root, host, npmCli, debug = false, noSign = false, bundleOnly = false }) {
  if (!/^(x86_64|i686|aarch64)-pc-windows-(msvc|gnu)$/.test(host)) throw new Error('A Windows Rust target is required');
  const runner = resolve(root, 'tools/cargo.mjs');
  const target = resolve(root, 'apps/studio/src-tauri/target', host);
  const commands = [
    { name: 'Build Studio', args: [npmCli, 'run', 'build'] },
    { name: 'Build desktop', args: [runner, 'build', '--manifest-path', resolve(root, 'apps/studio/src-tauri/Cargo.toml'), '--features', 'custom-protocol', '--locked', '--target-dir', target, ...(debug ? [] : ['--release'])] },
    { name: 'Bundle Windows setup', args: [runner, 'tauri', 'bundle', '--bundles', 'nsis', '--target', host, ...(debug ? ['--debug'] : []), ...(noSign ? ['--no-sign'] : [])] },
  ];
  return bundleOnly ? commands.slice(-1) : commands;
}

function main() {
  const flags = process.argv.slice(2);
  if (flags.includes('--help')) {
    console.log('Usage: npm run setup:build -- [--debug] [--no-sign] [--dry-run]\n       npm run setup:bundle -- [--debug] [--no-sign] [--dry-run]');
    return;
  }
  if (flags.some((flag) => !['--debug', '--no-sign', '--dry-run', '--bundle'].includes(flag))) throw new Error('Unknown setup option; use --help');
  if (process.platform !== 'win32') throw new Error('Windows setup building requires Windows');
  if (process.env.CARGO_BUILD_TARGET) throw new Error('Setup builds the current Rust host; remove the CARGO_BUILD_TARGET override for this command');
  if (process.env.CARGO_TARGET_DIR) throw new Error('Setup uses its dedicated output directory; remove the CARGO_TARGET_DIR override for this command');
  if (!process.env.npm_execpath) throw new Error('Run this command with npm run setup:build');
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
  const version = execFileSync(process.execPath, [resolve(root, 'tools/cargo.mjs'), 'version', '--verbose'], { cwd: root, encoding: 'utf8', windowsHide: true });
  const host = version.match(/^host:\s+(\S+)$/m)?.[1];
  const commands = setupCommands({ root, host, npmCli: process.env.npm_execpath, debug: flags.includes('--debug'), noSign: flags.includes('--no-sign'), bundleOnly: flags.includes('--bundle') });
  if (flags.includes('--dry-run')) {
    console.log(JSON.stringify({ host, commands }, null, 2));
    return;
  }
  for (const command of commands) {
    console.log(`${command.name} (${host})`);
    const result = spawnSync(process.execPath, command.args, { cwd: root, stdio: 'inherit', shell: false });
    if (result.error) throw result.error;
    if (result.status !== 0) { process.exitCode = result.status ?? 1; return; }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); }
  catch (error) { console.error(error instanceof Error ? error.message : String(error)); process.exitCode = 1; }
}
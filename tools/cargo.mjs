import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, delimiter, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const localHome = join(root, '.tools', 'cargo');
const localCargo = join(localHome, 'bin', process.platform === 'win32' ? 'cargo.exe' : 'cargo');
const environment = { ...process.env };
const hasLocal = existsSync(localCargo);
if (hasLocal) {
  environment.CARGO_HOME = localHome;
  environment.RUSTUP_HOME = join(root, '.tools', 'rustup');
  const pathKey = Object.keys(environment).find((key) => key.toLowerCase() === 'path') ?? 'PATH';
  environment[pathKey] = `${join(localHome, 'bin')}${delimiter}${environment[pathKey] ?? ''}`;
  const linkerBin = join(root, '.tools', 'llvm-mingw', 'llvm-mingw-20260908-ucrt-x86_64', 'bin');
  if (existsSync(join(linkerBin, 'dlltool.exe'))) {
    environment[pathKey] = `${linkerBin}${delimiter}${environment[pathKey]}`;
    environment.CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = join(linkerBin, 'x86_64-w64-mingw32-gcc.exe');
    environment.CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = '-C link-self-contained=yes';
  }
}
const argumentsList = process.argv.slice(2);
const tauri = argumentsList[0] === 'tauri';
const tauriArguments = argumentsList.slice(1);
const command = tauri ? process.execPath : hasLocal ? localCargo : 'cargo';
const commandArguments = tauri ? [join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js'), ...tauriArguments] : argumentsList;
const result = spawnSync(command, commandArguments, {
  cwd: tauri ? join(root, 'apps', 'studio') : root,
  env: environment,
  stdio: 'inherit',
  shell: false,
});
if (result.error) {
  console.error(`${tauri ? 'Tauri' : 'Cargo'} unavailable: ${result.error.message}. Install project dependencies and Rust from https://rustup.rs/.`);
}
process.exitCode = result.status ?? 1;
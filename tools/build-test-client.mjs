import { spawnSync } from 'node:child_process';

for (const args of [['tools/cargo.mjs', 'build', '-p', 'aislide-cli'], ['--test', 'tools/client.test.mjs']]) {
  const result = spawnSync(process.execPath, args, { stdio: 'inherit', shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) { process.exitCode = result.status ?? 1; break; }
}
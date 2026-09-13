import * as filesystem from 'node:fs/promises';

export async function publishNewFile(temporary, destination, bytes, signal, io = filesystem) {
  const file = await io.open(temporary, 'wx', 0o600);
  try {
    try {
      await file.writeFile(bytes);
      await file.sync();
    } finally { await file.close(); }
    if (signal?.aborted) throw new Error('Operation cancelled');
    await io.link(temporary, destination);
  } finally {
    await io.unlink(temporary);
  }
}
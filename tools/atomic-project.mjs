import * as filesystem from 'node:fs/promises';
import { join } from 'node:path';
import { randomUUID } from 'node:crypto';

export async function publishProject(directory, filename, result, signal, io = filesystem) {
  const items = [
    { name: filename, bytes: Buffer.from(result.base64, 'base64') },
    { name: filename.replace(/\.pptx$/, '.aislide.json'), bytes: Buffer.from(JSON.stringify(result.checkpoint)) },
  ].map((item) => ({ ...item, temporary: join(directory, `.aislide-${randomUUID()}.tmp`), destination: join(directory, item.name), created: false, published: false }));
  if (signal?.aborted) throw new Error('Operation cancelled');
  for (const item of items) {
    let exists = false;
    try { await io.lstat(item.destination); exists = true; }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    if (exists) throw new Error(`Output already exists: ${item.destination}`);
  }
  try {
    for (const item of items) {
      const file = await io.open(item.temporary, 'wx', 0o600);
      item.created = true;
      try { await file.writeFile(item.bytes); await file.sync(); } finally { await file.close(); }
    }
    for (const item of items) {
      if (signal?.aborted) throw new Error('Operation cancelled');
      await io.link(item.temporary, item.destination); item.published = true;
    }
    return { path: items[0].destination, checkpoint_path: items[1].destination, bytes: items[0].bytes.length };
  } catch (error) {
    const published = items.filter((item) => item.published);
    if (published.length) throw new Error(`Partial project publication: ${published.map((item) => item.destination).join(', ')}. No published paths were removed; verify the pair before use.`, { cause: error });
    throw error;
  } finally {
    await Promise.all(items.filter((item) => item.created).map((item) => io.unlink(item.temporary)));
  }
}
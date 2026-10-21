import { mountInteractive } from './runtime.mjs';

const root = document.getElementById('interactive-root');
try {
  const response = await fetch('./preview.json', { credentials: 'omit', cache: 'no-store' });
  if (!response.ok) throw new Error(`No preview data (${response.status}). Start the server with an explicitly generated --preview JSON file.`);
  const text = await response.text();
  if (text.length > 2 * 1024 * 1024) throw new Error('Preview payload exceeds 2 MiB.');
  mountInteractive(root, JSON.parse(text));
} catch (reason) {
  const error = document.createElement('p');
  error.setAttribute('role', 'alert');
  error.textContent = reason instanceof Error ? reason.message : String(reason);
  root.replaceChildren(error);
}

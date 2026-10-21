import { mountInteractive } from './runtime.mjs';

const root = document.getElementById('interactive-root');
const fail = reason => {
  const message = document.createElement('p');
  message.setAttribute('role', 'alert');
  message.textContent = `AISlide playback could not start: ${reason instanceof Error ? reason.message : String(reason)}`;
  root.replaceChildren(message);
};
const timeout = setTimeout(() => fail('Office did not initialize. Open this content add-in inside a supported PowerPoint host and verify its HTTPS runtime and installation.'), 20_000);
if (!globalThis.Office?.onReady) {
  clearTimeout(timeout);
  fail('The Microsoft Office JavaScript library did not load. Verify the network and Office add-in policy.');
} else {
  Office.onReady().then(info => {
    clearTimeout(timeout);
    if (info.host !== Office.HostType.PowerPoint) throw new Error('This content add-in requires PowerPoint.');
    const settings = Office.context?.document?.settings;
    if (!settings?.get) throw new Error('Document settings are unavailable in this PowerPoint host.');
    const payload = settings.get('aislideInteractive');
    if (!payload) throw new Error('This add-in has no AISlide component data. Export a PowerPoint interactive PPTX from AISlide; inserting an empty add-in is not sufficient.');
    mountInteractive(root, typeof payload === 'string' ? JSON.parse(payload) : payload);
  }).catch(fail);
}

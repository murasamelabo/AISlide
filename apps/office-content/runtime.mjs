const MAX_BYTES = 2 * 1024 * 1024;
const SLOTS = 'dk1 lt1 dk2 lt2 accent1 accent2 accent3 accent4 accent5 accent6 hlink folHlink'.split(' ');
const FRAME = 'id x y width height';
const mounted = new WeakMap();
const own = (object, key) => Object.prototype.hasOwnProperty.call(object, key);
const fail = message => { throw new Error(`AISlide interactive: ${message}`); };

function record(value, required, optional, path) {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      ![Object.prototype, null].includes(Object.getPrototypeOf(value))) fail(`${path} must be a plain record`);
  const allowed = `${required} ${optional}`.split(' ').filter(Boolean);
  for (const key of Reflect.ownKeys(value)) {
    if (!allowed.includes(key) || !own(Object.getOwnPropertyDescriptor(value, key), 'value'))
      fail(`${path} has an unsupported field or accessor: ${String(key)}`);
  }
  for (const key of required.split(' ').filter(Boolean)) {
    if (!own(value, key)) fail(`${path}.${key} is required`);
  }
}

function list(value, minimum, maximum, path) {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype ||
      value.length < minimum || value.length > maximum || Reflect.ownKeys(value).length !== value.length + 1)
    fail(`${path} must be a plain array with ${minimum}-${maximum} entries`);
  for (let index = 0; index < value.length; index++) {
    if (!own(Object.getOwnPropertyDescriptor(value, String(index)) || {}, 'value'))
      fail(`${path} must contain only plain, dense entries`);
  }
  return value;
}

function text(value, maximum, path, required = true) {
  if (typeof value !== 'string' || value.length > maximum * 2 || [...value].length > maximum ||
      /[\u0000-\u0008\u000b-\u001f\u007f]/u.test(value) || (required && !value.trim()))
    fail(`${path} must be ${required ? 'nonempty ' : ''}text of at most ${maximum} characters without control characters`);
  return value;
}

function identity(value, path) {
  if (typeof value !== 'string' || !/^[A-Za-z0-9_-]{1,32}$/.test(value)) fail(`${path} has an invalid ID`);
  return value;
}

function number(value, path, positive = false, maximum = 4096) {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > maximum || (positive && value === 0))
    fail(`${path} must be finite, ${positive ? 'positive' : 'nonnegative'} and at most ${maximum}`);
}

function color(value, colors, path) {
  if (typeof value !== 'string' || (!/^[0-9a-f]{6}$/i.test(value) &&
      !(value.startsWith('@') && SLOTS.includes(value.slice(1))))) fail(`${path} is not an RGB or OOXML theme color`);
  return `#${value.startsWith('@') ? colors[value.slice(1)] : value}`;
}

function canonical(state) {
  return { selected_id: state.selected_id, ...(state.group_id == null ? {} : { group_id: state.group_id }),
    dimension: state.dimension, scale: state.scale };
}
const stateKey = state => JSON.stringify([state.selected_id, state.group_id ?? null, state.dimension, state.scale]);

function validateState(state, payload, path) {
  record(state, 'selected_id dimension scale', 'group_id', path);
  identity(state.selected_id, `${path}.selected_id`);
  const item = payload.items.find(candidate => candidate.id === state.selected_id);
  if (!item) fail(`${path}.selected_id refers to an unknown item`);
  if (state.group_id != null && (!payload.groups.some(group => group.id === state.group_id) || item.group_id !== state.group_id))
    fail(`${path}.group_id must name a group containing the selected item`);
  if (!Number.isInteger(state.dimension) || state.dimension < 0 || state.dimension >= Math.max(1, payload.dimensions.length))
    fail(`${path}.dimension is not an available dimension`);
  if (!['linear', 'logarithmic'].includes(state.scale)) fail(`${path}.scale must be linear or logarithmic`);
}

function validateFormat(format, font, path) {
  if (format == null) return;
  const defaults = { italic: false, underline: false, alignment: 'left', vertical: 'top', bullet: 'none',
    inherit_layout: false, font_family: null, hyperlink: null, placeholder: null, padding: null, paragraphs: null };
  record(format, '', Object.keys(defaults).join(' '), path);
  for (const [key, value] of Object.entries(format)) {
    if (key === 'font_family' && (value == null || value === '@minor' || value === font)) continue;
    if (key === 'hyperlink' && (value == null || value === '')) continue;
    if (key === 'paragraphs' && value != null) { list(value, 0, 0, `${path}.paragraphs`); continue; }
    if (key === 'padding' && value != null) {
      record(value, '', 'left top right bottom', `${path}.padding`);
      if (Object.values(value).some(inset => inset !== 0)) fail(`${path}.padding must be empty or zero`);
      continue;
    }
    if (value !== defaults[key]) fail(`${path}.${key} is not a supported generated default`);
  }
}

function validateElement(group, payload, path) {
  record(group, `type ${FRAME} view_width view_height children`, 'visual', path);
  if (group.type !== 'group') fail(`${path} must be a generated group`);
  const frame = (element, elementPath) => {
    text(element.id, 80, `${elementPath}.id`);
    for (const key of ['x', 'y', 'width', 'height']) number(element[key], `${elementPath}.${key}`, ['width', 'height'].includes(key));
    if (element.visual != null) fail(`${elementPath}.visual styles are not supported`);
  };
  frame(group, path);
  number(group.view_width, `${path}.view_width`, true);
  number(group.view_height, `${path}.view_height`, true);
  const ids = new Set([group.id]);
  for (const [index, child] of list(group.children, 1, 128, `${path}.children`).entries()) {
    const childPath = `${path}.children[${index}]`;
    record(child, `type ${FRAME}`, 'fill text font_size color bold format visual', childPath);
    if (child.type === 'rect') record(child, `type ${FRAME} fill`, 'visual', childPath);
    else if (child.type === 'text') record(child, `type ${FRAME} text font_size color bold`, 'format visual', childPath);
    else fail(`${childPath}.type must be rect or text`);
    frame(child, childPath);
    if (ids.has(child.id)) fail(`${childPath}.id is duplicated`);
    ids.add(child.id);
    if (child.x + child.width > group.view_width + 0.000001 || child.y + child.height > group.view_height + 0.000001)
      fail(`${childPath} lies outside the generated view`);
    if (child.type === 'rect') color(child.fill, payload.colors, `${childPath}.fill`);
    else {
      text(child.text, 4000, `${childPath}.text`, false);
      number(child.font_size, `${childPath}.font_size`, true, 120);
      if (typeof child.bold !== 'boolean') fail(`${childPath}.bold must be boolean`);
      color(child.color, payload.colors, `${childPath}.color`);
      validateFormat(child.format, payload.font_family, `${childPath}.format`);
    }
  }
}

function validate(payload) {
  record(payload, 'version title template source groups dimensions items initial_state colors font_family views', '', 'payload');
  if (payload.version !== 1) fail('payload.version must be 1');
  if (!['summary', 'dashboard', 'timeline', 'comparison', 'map'].includes(payload.template)) fail('payload.template is unsupported');
  text(payload.title, 120, 'payload.title');
  text(payload.source, 240, 'payload.source');
  text(payload.font_family, 100, 'payload.font_family');
  if (/[\t\n]/u.test(payload.font_family)) fail('payload.font_family cannot contain control characters');
  record(payload.colors, SLOTS.join(' '), '', 'payload.colors');
  for (const slot of SLOTS) {
    if (typeof payload.colors[slot] !== 'string' || !/^[0-9a-f]{6}$/i.test(payload.colors[slot]))
      fail(`payload.colors.${slot} must be six hexadecimal digits`);
  }
  const groups = new Set();
  for (const group of list(payload.groups, 0, 4, 'payload.groups')) {
    record(group, 'id label', '', 'group');
    identity(group.id, 'group.id'); text(group.label, 32, 'group.label');
    if (groups.has(group.id)) fail('group.id is duplicated');
    groups.add(group.id);
  }
  for (const dimension of list(payload.dimensions, payload.template === 'comparison' ? 1 : 0, 4, 'payload.dimensions'))
    text(dimension, 32, 'dimension');
  const ids = new Set();
  for (const item of list(payload.items, 2, 8, 'payload.items')) {
    record(item, 'id label', 'group_id', 'item');
    identity(item.id, 'item.id'); text(item.label, 48, 'item.label');
    if (ids.has(item.id)) fail('item.id is duplicated');
    if (item.group_id != null && !groups.has(item.group_id)) fail('item.group_id refers to an unknown group');
    ids.add(item.id);
  }
  validateState(payload.initial_state, payload, 'initial_state');
  const states = new Set();
  for (const [index, view] of list(payload.views, 1, 128, 'payload.views').entries()) {
    record(view, 'state element', 'hotspots controls_frame', `views[${index}]`);
    validateState(view.state, payload, `views[${index}].state`);
    const key = stateKey(view.state);
    if (states.has(key)) fail(`views[${index}] duplicates a compiled state`);
    states.add(key);
    validateElement(view.element, payload, `views[${index}].element`);
    if (view.controls_frame !== undefined) {
      record(view.controls_frame, 'x y width height', '', 'controls_frame');
      for (const key of ['x', 'y', 'width', 'height']) number(view.controls_frame[key], `controls_frame.${key}`, ['width', 'height'].includes(key));
      const frame = view.controls_frame;
      if (frame.x + frame.width > view.element.view_width || frame.y + frame.height > view.element.view_height) fail('controls_frame lies outside the generated view');
    }
    if (view.hotspots !== undefined) {
      const seen = new Set();
      for (const hotspot of list(view.hotspots, 0, 8, `views[${index}].hotspots`)) {
        record(hotspot, 'item_id x y width height', '', 'hotspot');
        const item = payload.items.find(item => item.id === hotspot.item_id);
        if (!item || seen.has(hotspot.item_id) || (view.state.group_id != null && item.group_id !== view.state.group_id)) fail('hotspot must name a unique visible item');
        seen.add(hotspot.item_id);
        for (const key of ['x', 'y', 'width', 'height']) number(hotspot[key], `hotspot.${key}`, ['width', 'height'].includes(key));
        if (hotspot.x + hotspot.width > view.element.view_width || hotspot.y + hotspot.height > view.element.view_height) fail('hotspot lies outside the generated view');
      }
    }
  }
  if (!states.has(stateKey(payload.initial_state))) fail('initial_state has no compiled view');
  const serialized = JSON.stringify(payload);
  if (new TextEncoder().encode(serialized).length > MAX_BYTES) fail('payload exceeds the 2 MiB limit');
  return JSON.parse(serialized);
}

/** Mount trusted generated primitives after validating the entire untrusted payload. */
export function mountInteractive(root, payload) {
  const data = validate(payload);
  if (!root || root.nodeType !== 1 || typeof root.replaceChildren !== 'function') fail('root must be a DOM element');
  const document = root.ownerDocument;
  const Observer = document.defaultView?.ResizeObserver;
  if (!Observer) fail('ResizeObserver is required');
  const views = new Map(data.views.map(view => [stateKey(view.state), view]));
  const embeddedControls = data.views.every(view => view.controls_frame);
  let state = canonical(data.initial_state), disposed = false, activeGroup, activeControls, stage, compactControls;
  const removers = [], itemButtons = new Map();
  const node = (tag, className = '', content) => {
    const element = document.createElement(tag);
    element.className = className;
    if (content !== undefined) element.textContent = content;
    return element;
  };
  const host = node('section', `aislide-interactive${embeddedControls ? ' aislide-interactive--embedded' : ''}`);
  host.setAttribute('aria-label', data.title);
  host.style.fontFamily = `"${data.font_family.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}", sans-serif`;
  const header = node('header', 'aislide-interactive__header');
  header.append(node('h1', '', data.title), node('p', 'aislide-interactive__source', `Source: ${data.source}`));
  const layout = node('div', 'aislide-interactive__layout');
  const menu = node('nav', 'aislide-interactive__menu');
  menu.setAttribute('aria-label', 'Interactive controls');
  const disclosure = node('details', 'aislide-interactive__options');
  const optionsToggle = node('summary', '', '⋯');
  optionsToggle.setAttribute('aria-label', 'Interaction options');
  const controls = embeddedControls ? node('div', 'aislide-interactive__controls') : menu;
  if (embeddedControls) { disclosure.append(optionsToggle, controls); menu.append(disclosure); }
  const preview = node('div', 'aislide-interactive__preview');
  const viewport = node('div', 'aislide-interactive__viewport');
  viewport.setAttribute('role', 'group'); viewport.setAttribute('aria-label', 'Generated view');
  const transcript = node('details', 'aislide-interactive__transcript');
  const readable = node('div');
  const transcriptToggle = node('summary', '', embeddedControls ? 'Text' : 'Readable view text');
  transcriptToggle.setAttribute('aria-label', 'Readable view text');
  transcript.append(transcriptToggle, readable);
  const status = node('p', 'aislide-interactive__status');
  status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite'); status.setAttribute('aria-atomic', 'true');
  const error = node('p', 'aislide-interactive__error');
  error.setAttribute('role', 'alert'); error.hidden = true;
  function listen(element, event, callback) {
    element.addEventListener(event, callback);
    removers.push(() => element.removeEventListener(event, callback));
  }
  function attempt(next) {
    if (disposed) return;
    try { render(next); error.hidden = true; error.textContent = ''; }
    catch (cause) { syncControls(); error.textContent = cause.message; error.hidden = false; }
  }
  function button(label, callback, parent = controls) {
    const element = node('button', '', label); element.type = 'button';
    listen(element, 'click', callback); parent.append(element); return element;
  }
  function select(label, options, callback) {
    const wrapper = node('label', 'aislide-interactive__field'), element = node('select');
    element.title = label;
    wrapper.append(node('span', '', label), element);
    for (const [value, name, disabled = false] of options) {
      const option = node('option', '', name); option.value = value; option.disabled = disabled; element.append(option);
    }
    listen(element, 'change', () => callback(element.value)); controls.append(wrapper); return element;
  }
  const groupSelect = select('Group', [['', 'All groups'], ...data.groups.map(group =>
    [group.id, group.label, !data.items.some(item => item.group_id === group.id)])], value => {
    const candidates = data.items.filter(item => !value || item.group_id === value);
    attempt({ ...state, group_id: value || undefined,
      selected_id: candidates.find(item => item.id === state.selected_id)?.id ?? candidates[0]?.id });
  });
  if (embeddedControls && data.groups.length === 0) groupSelect.parentElement.hidden = true;
  const scaleSelect = data.template === 'dashboard' ? select('Scale',
    [['linear', 'Linear'], ['logarithmic', 'Logarithmic — log(1 + value)']], scale => attempt({ ...state, scale })) : null;
  const dimensionSelect = data.template === 'comparison' ? select('Dimension',
    data.dimensions.map((label, index) => [String(index), label]), value => attempt({ ...state, dimension: Number(value) })) : null;
  const items = node('div', 'aislide-interactive__items');
  items.setAttribute('role', 'group'); items.setAttribute('aria-label', 'Items'); controls.append(items);
  if (embeddedControls) items.hidden = true;
  for (const item of data.items) itemButtons.set(item.id, button(item.label, () => attempt({ ...state, selected_id: item.id }), items));
  const navigation = node('div', 'aislide-interactive__navigation');
  const visibleItems = () => data.items.filter(item => state.group_id == null || item.group_id === state.group_id);
  function move(offset) {
    const candidates = visibleItems(), index = candidates.findIndex(item => item.id === state.selected_id);
    if (candidates[index + offset]) attempt({ ...state, selected_id: candidates[index + offset].id });
  }
  const previous = data.template === 'timeline' ? button('Previous', () => move(-1), navigation) : null;
  const next = data.template === 'timeline' ? button('Next', () => move(1), navigation) : null;
  if (previous) controls.append(navigation);
  button('Reset', () => attempt(canonical(data.initial_state)));
  function syncControls() {
    groupSelect.value = state.group_id ?? '';
    if (scaleSelect) scaleSelect.value = state.scale;
    if (dimensionSelect) dimensionSelect.value = String(state.dimension);
    for (const item of data.items) {
      const selected = item.id === state.selected_id, element = itemButtons.get(item.id);
      element.hidden = state.group_id != null && item.group_id !== state.group_id;
      element.setAttribute('aria-pressed', String(selected));
      element.textContent = `${item.label}${selected ? ' — selected' : ''}`;
    }
    if (previous) {
      const candidates = visibleItems(), index = candidates.findIndex(item => item.id === state.selected_id);
      previous.disabled = index === 0; next.disabled = index === candidates.length - 1;
    }
  }
  function resize() {
    if (disposed || !stage) return;
    const { width, height } = viewport.getBoundingClientRect();
    const scale = Math.min(width / activeGroup.width, height / activeGroup.height);
    const sx = scale * activeGroup.width / activeGroup.view_width;
    const sy = scale * activeGroup.height / activeGroup.view_height;
    if (!Number.isFinite(sx) || !Number.isFinite(sy)) fail('generated view cannot be scaled to the container');
    stage.style.left = `${(width - activeGroup.width * scale) / 2}px`;
    stage.style.top = `${(height - activeGroup.height * scale) / 2}px`;
    stage.style.transform = `scale(${sx}, ${sy})`;
    if (embeddedControls) {
      const compact = scale < 0.6;
      if (compactControls !== compact) {
        compactControls = compact;
        host.classList.toggle('aislide-interactive--compact', compact);
        disclosure.open = !compact;
        items.hidden = !compact;
      }
      menu.style.left = `${compact ? (width + activeGroup.width * scale) / 2 - 36 : (width - activeGroup.width * scale) / 2 + activeControls.x * sx}px`;
      menu.style.top = `${(height - activeGroup.height * scale) / 2 + (compact ? 6 : activeControls.y * sy)}px`;
      menu.style.width = compact ? '30px' : `${activeControls.width * sx}px`;
    }
  }
  function render(nextState) {
    validateState(nextState, data, 'requested state');
    const view = views.get(stateKey(nextState));
    if (!view) fail(`No compiled view for state ${stateKey(nextState)}`);
    const group = view.element;
    const drawing = node('div', 'aislide-interactive__drawing'), paragraphs = [];
    drawing.style.width = `${group.view_width}px`; drawing.style.height = `${group.view_height}px`;
    for (const child of group.children) {
      const element = node('div', `aislide-interactive__${child.type}`);
      for (const [property, value] of Object.entries({ left: child.x, top: child.y, width: child.width, height: child.height }))
        element.style[property] = `${value}px`;
      if (child.type === 'rect') { element.style.backgroundColor = color(child.fill, data.colors, 'fill'); element.setAttribute('aria-hidden', 'true'); }
      else {
        element.textContent = child.text; element.style.color = color(child.color, data.colors, 'color');
        element.style.fontSize = `${child.font_size}px`; element.style.fontWeight = child.bold ? '700' : '400';
        if (child.text) paragraphs.push(node('p', '', child.text));
      }
      drawing.append(element);
    }
    for (const hotspot of view.hotspots ?? []) {
      const item = data.items.find(item => item.id === hotspot.item_id);
      const target = node('button', 'aislide-interactive__hotspot');
      target.type = 'button';
      target.dataset.itemId = item.id;
      target.setAttribute('aria-label', `Select ${item.label}`);
      target.setAttribute('aria-pressed', String(item.id === nextState.selected_id));
      target.title = item.label;
      for (const [property, value] of Object.entries({ left: hotspot.x, top: hotspot.y, width: hotspot.width, height: hotspot.height })) target.style[property] = `${value}px`;
      target.addEventListener('click', () => {
        attempt({ ...state, selected_id: item.id });
        if (!disposed) viewport.querySelectorAll('.aislide-interactive__hotspot').forEach(element => {
          if (element.dataset.itemId === item.id) element.focus({ preventScroll: true });
        });
      });
      drawing.append(target);
    }
    const previousStage = stage;
    state = canonical(nextState); activeGroup = group; activeControls = view.controls_frame; stage = drawing;
    if (embeddedControls) {
      if (previousStage) previousStage.replaceWith(drawing);
      else viewport.prepend(drawing);
    } else viewport.replaceChildren(drawing);
    readable.replaceChildren(...paragraphs); syncControls(); resize();
    status.textContent = `Selected: ${data.items.find(item => item.id === state.selected_id).label}`;
  }
  if (embeddedControls) { controls.append(transcript); viewport.append(menu); preview.append(viewport); layout.append(preview); }
  else { preview.append(viewport, transcript); layout.append(preview, menu); }
  host.append(header, layout, status, error);
  mounted.get(root)?.dispose();
  root.replaceChildren(host);
  const observer = new Observer(resize);
  const api = {
    getState: () => canonical(state),
    dispose() {
      if (disposed) return;
      disposed = true; observer.disconnect(); removers.forEach(remove => remove()); host.remove();
      if (mounted.get(root) === api) mounted.delete(root);
    },
  };
  try { render(data.initial_state); observer.observe(viewport); mounted.set(root, api); }
  catch (cause) { api.dispose(); throw cause; }
  return api;
}

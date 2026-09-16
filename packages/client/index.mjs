export class AislideClient {
  #transport;
  constructor(transport) {
    if (typeof transport !== 'function') throw new TypeError('A core request transport is required');
    this.#transport = transport;
  }
  request(request, options) { return this.#transport(request, options); }
  ingest(input, options) { return this.request({ op: 'ingest', input }, options); }
  dataReport(source, mapping, options) { return this.request({ op: 'data_report', source, mapping }, options); }
  designDefaults(options) { return this.request({ op: 'design_defaults' }, options); }
  designPresets(options) { return this.request({ op: 'design_presets' }, options); }
  objectCatalog(options) { return this.request({ op: 'object_catalog' }, options); }
  partCatalog(options) { return this.request({ op: 'part_catalog' }, options); }
  graphCatalog(options) { return this.request({ op: 'graph_catalog' }, options); }
  createGraphIcon(input, options) { return this.request({ ...input, op: 'create_graph_icon' }, options); }
  createGraph(input, options) { return this.request({ ...input, op: 'create_graph' }, options); }
  transformGraph(spec, operations, options) { return this.request({ op: 'transform_graph', spec, operations }, options); }
  createPart(input, options) { return this.request({ ...input, op: 'create_part' }, options); }
  createObject(input, options) { return this.request({ ...input, op: 'create_object' }, options); }
  createAsset(input, options) { return this.request({ ...input, op: 'create_asset' }, options); }
  async createPresentation(id, title = 'Untitled presentation', options) {
    const document = await this.request({ op: 'create_presentation', id, title }, options);
    return new DocumentSession(this.#transport, document);
  }
  async openPresentation(id, base64, options) {
    const result = await this.request({ op: 'open_presentation', id, base64 }, options);
    return { session: new DocumentSession(this.#transport, result.document), warnings: result.warnings, objects: result.objects, format: result.format };
  }
  async importPresentation(id, base64, options) {
    const result = await this.request({ op: 'import_document', id, base64 }, options);
    return { session: new DocumentSession(this.#transport, result.document), warnings: result.warnings, objects: result.objects };
  }
  async createDocument(input, options) {
    const document = await this.request({ ...input, op: 'new_document' }, options);
    return new DocumentSession(this.#transport, document);
  }
  async openProject({ base64, checkpoint }, options) {
    const document = await this.request({ op: 'open_project', base64, checkpoint }, options);
    return new DocumentSession(this.#transport, document);
  }
}

export class DocumentSession {
  #request;
  #document;
  #past = [];
  #future = [];
  #busy = false;
  constructor(transport, document) { this.#request = transport; this.#document = structuredClone(document); }
  get document() { return structuredClone(this.#document); }
  get revision() { return this.#document.revision; }
  get canUndo() { return this.#past.length > 0; }
  get canRedo() { return this.#future.length > 0; }
  get busy() { return this.#busy; }

  #retain(history, receipt) {
    if (!receipt) return;
    history.push(receipt);
    while (history.length > 30 || (history.length > 1 && JSON.stringify(history).length > 4 * 1024 * 1024)) history.shift();
  }
  async #run(action, { signal } = {}) {
    if (signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
    if (this.#busy) throw new Error('Document is busy');
    this.#busy = true;
    try { return await action(); } finally { this.#busy = false; }
  }
  async transact(operations, options = {}) {
    return this.#run(() => this.#commit(operations, options), options);
  }
  async #commit(operations, options) {
    const result = await this.#request({ op: 'transaction', document: this.#document, transaction: {
      expected_revision: options.expectedRevision ?? this.#document.revision,
      expected_hash: this.#document.hash, operations,
    } }, { signal: options.signal });
    return this.#accept(result, options);
  }
  #accept(result, options) {
    if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
    if (result.receipt) { this.#retain(this.#past, result.receipt); this.#future = []; }
    this.#document = result.document;
    return this.document;
  }
  #part(op, slideId, input, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ ...input, op, document: this.#document, expected_revision: this.revision, slide_id: slideId }, { signal: options.signal });
      return this.#accept(result, options);
    }, options);
  }
  addPart(slideId, input, options) { return this.#part('insert_part', slideId, input, options); }
  updatePart(slideId, input, options) { return this.#part('update_part', slideId, input, options); }
  addGraph(slideId, input, options) { return this.#part('insert_graph', slideId, input, options); }
  updateGraph(slideId, input, options) { return this.#part('update_graph', slideId, input, options); }
  applyGraph(slideId, input, options) { return this.#part('apply_graph', slideId, input, options); }
  editElements(slideId, operations, options) { return this.#part('edit_elements', slideId, { operations }, options); }
  editSlides(operations, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ op: 'edit_slides', document: this.#document, expected_revision: this.revision, operations }, { signal: options.signal });
      return this.#accept(result, options);
    }, options);
  }
  #expectRevision(options) {
    if (options.expectedRevision !== undefined && options.expectedRevision !== this.revision) throw new Error('Revision conflict');
  }
  #transform(op, input, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const deck = await this.#request({ ...input, op, deck: this.#document.deck }, { signal: options.signal });
      if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
      return this.#commit([{ op: 'replace', path: '/deck', value: deck }], options);
    }, options);
  }
  updateDesign(design, options) { return this.#transform('update_design', { design }, options); }
  applyDesignPreset(presetId, options) { return this.#transform('apply_design_preset', { preset_id: presetId }, options); }
  applyTheme(theme, options) { return this.#transform('apply_theme', { theme }, options); }
  assignLayout(slideId, layoutId, options) { return this.#transform('assign_layout', { slide_id: slideId, layout_id: layoutId }, options); }
  addObject(slideId, input, options) { return this.#insert('create_object', slideId, input, options); }
  addAsset(slideId, input, options) { return this.#insert('create_asset', slideId, input, options); }
  #insert(op, slideId, input, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const index = this.#document.deck.slides.findIndex((slide) => slide.id === slideId);
      if (index < 0) throw new Error('Unknown slide ID');
      const element = await this.#request({ ...input, op }, { signal: options.signal });
      if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
      return this.#commit([{ op: 'add', path: `/deck/slides/${index}/elements/-`, value: element }], options);
    }, options);
  }
  replaceDeck(deck, { sources, bindings, report, ...options } = {}) {
    const operations = [{ op: 'replace', path: '/deck', value: deck }];
    if (sources !== undefined) operations.push({ op: 'replace', path: '/sources', value: sources });
    if (bindings !== undefined) operations.push({ op: 'replace', path: '/bindings', value: bindings });
    if (report !== undefined) operations.push({ op: 'add', path: '/report', value: report });
    return this.transact(operations, options);
  }
  async #restore(from, to, options = {}) {
    return this.#run(async () => {
      const receipt = from.at(-1);
      if (!receipt) throw new Error('Nothing to restore');
      const result = await this.#request({ op: 'undo_transaction', document: this.#document, expected_revision: this.#document.revision, receipt }, { signal: options.signal });
      if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
      from.pop(); this.#retain(to, result.receipt); this.#document = result.document;
      return this.document;
    }, options);
  }
  undo(options) { return this.#restore(this.#past, this.#future, options); }
  redo(options) { return this.#restore(this.#future, this.#past, options); }
  exportProject(options = {}) { return this.#run(() => this.#request({ op: 'export_project', document: this.#document }, options), options); }
  exportPresentation(options = {}) { return this.#run(() => this.#request({ op: 'export_presentation', document: this.#document }, options), options); }
}
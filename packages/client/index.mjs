export class AislideClient {
  #transport;
  constructor(transport) {
    if (typeof transport !== 'function') throw new TypeError('A core request transport is required');
    this.#transport = transport;
  }
  request(request, options) { return this.#transport(request, options); }
  ingest(input, options) { return this.request({ op: 'ingest', input }, options); }
  dataReport(source, mapping, options) { return this.request({ op: 'data_report', source, mapping }, options); }
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
    return this.#run(async () => {
      const result = await this.#request({ op: 'transaction', document: this.#document, transaction: {
        expected_revision: options.expectedRevision ?? this.#document.revision,
        expected_hash: this.#document.hash, operations,
      } }, { signal: options.signal });
      if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
      if (result.receipt) { this.#retain(this.#past, result.receipt); this.#future = []; }
      this.#document = result.document;
      return this.document;
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
}
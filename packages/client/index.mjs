export const CAPACITY_PROFILES = Object.freeze({
  legacy: Object.freeze({ slides: 32, elements_per_slide: 256, elements_total: 2048, group_depth: 8, document_bytes: 2097152, request_bytes: 4194304, archive_bytes: 3144704, image_encoded_bytes: 3145728, unique_images: 128, raster_work_bytes: 67108864, json_nodes: 250000, json_depth: 64 }),
  standard: Object.freeze({ slides: 128, elements_per_slide: 256, elements_total: 8192, group_depth: 8, document_bytes: 8388608, request_bytes: 16777216, archive_bytes: 8388608, image_encoded_bytes: 3145728, unique_images: 128, raster_work_bytes: 67108864, json_nodes: 250000, json_depth: 64 }),
  large: Object.freeze({ slides: 256, elements_per_slide: 256, elements_total: 8192, group_depth: 8, document_bytes: 33554432, request_bytes: 100663296, archive_bytes: 16777216, image_encoded_bytes: 3145728, unique_images: 128, raster_work_bytes: 67108864, json_nodes: 250000, json_depth: 64 }),
});

function capacityProfile(profile = 'large') {
  if (!['legacy', 'standard', 'large'].includes(profile)) throw new Error('Unknown capacity profile');
  return profile;
}

function withProfile(request, profile) {
  return profile === 'large' && request.capacity_profile === undefined ? request : { ...request, capacity_profile: profile };
}

export function encodeCoreRequest(request) {
  const profile = capacityProfile(request?.capacity_profile);
  const limits = CAPACITY_PROFILES[profile];
  let nodes = 0;
  let characters = 0;
  const visit = (value, depth) => {
    if (++nodes > limits.json_nodes || depth > limits.json_depth) throw new Error('JSON node/depth limit');
    if (typeof value === 'string') characters += value.length;
    if (characters > limits.request_bytes) throw new Error('Request exceeds capacity byte limit');
    if (value && typeof value === 'object') {
      if (Array.isArray(value) && value.length > limits.json_nodes - nodes) throw new Error('JSON node limit');
      for (const key of Object.keys(value)) { characters += key.length; visit(value[key], depth + 1); }
    }
  };
  visit(request, 0);
  const payload = JSON.stringify(request);
  if (!payload || new TextEncoder().encode(payload).length > limits.request_bytes) throw new Error('Request exceeds capacity byte limit');
  return payload;
}

export class AislideClient {
  #transport;
  #profile;
  constructor(transport, options = {}) {
    if (typeof transport !== 'function') throw new TypeError('A core request transport is required');
    this.#transport = transport;
    this.#profile = capacityProfile(options.capacityProfile);
  }
  #options(options) { return { ...options, capacityProfile: capacityProfile(options?.capacityProfile === undefined ? this.#profile : options.capacityProfile) }; }
  async request(request, options) {
    if (options?.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
    const selected = options?.capacityProfile === undefined ? (request.capacity_profile === undefined ? this.#profile : request.capacity_profile) : options.capacityProfile;
    const result = await this.#transport(withProfile(request, capacityProfile(selected)), options);
    if (options?.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
    return result;
  }
  authoringCapabilities(options) { return this.request({ op: 'authoring_capabilities' }, options); }
  computeChartPresentation(input, options) { return this.request({ ...input, op: 'compute_chart_presentation' }, options); }
  renderElementPreview(element, theme, options) { return this.request({ op: 'render_element_preview', element, theme }, options); }
  textAssist(input, options) { return this.request({ op: 'text_assist', input }, options); }
  segmentationStatus(options) { return this.request({ op: 'segmentation_status' }, options); }
  segmentImage(input, options) { return this.request({ ...input, op: 'segment_image' }, options); }
  importProofingDictionary(input, options) { return this.request({ ...input, op: 'import_proofing_dictionary' }, options); }
  proofText(input, options) { return this.request({ ...input, op: 'proof_text' }, options); }
  inspectFont(base64, options) { return this.request({ op: 'inspect_font', base64 }, options); }
  inspectPptxFonts(base64, options) { return this.request({ op: 'inspect_pptx_fonts', base64 }, options); }
  inspectMasterSource(input, options) { return this.request({ ...input, op: 'inspect_master_source' }, options); }
  capacityProfiles(options) { return this.request({ op: 'capacity_profiles' }, options); }
  async verifyRecovery(document, options) {
    const verified = structuredClone(await this.request({ op: 'verify_recovery', document }, options));
    const pending = [verified];
    while (pending.length) {
      const value = pending.pop();
      if (value !== null && typeof value === 'object') { Object.freeze(value); pending.push(...Object.values(value)); }
    }
    return verified;
  }
  async recoverPresentation(document, options) {
    return new DocumentSession(this.#transport, await this.verifyRecovery(document, options), this.#options(options));
  }
  verifySessionRecovery(envelope, options) { return this.request({ op: 'verify_session_recovery', envelope, capacity_profile: envelope.capacity_profile }, options); }
  recoverSession(envelope, options) { return DocumentSession.recover(this.#transport, envelope, options); }
  formatTextElement(input, options) { return this.request({ ...input, op: 'format_text_element' }, options); }
  replaceElementText(input, options) { return this.request({ ...input, op: 'replace_element_text' }, options); }
  setTableCellText(input, options) { return this.request({ ...input, op: 'set_table_cell_text' }, options); }
  editVector(input, options) { return this.request({ ...input, op: 'edit_vector' }, options); }
  searchText(deck, input, options) { return this.request({ op: 'search_text', deck, options: input }, options); }
  editImage(input, options) { return this.request({ ...input, op: 'edit_image' }, options); }
  async importTemplate(id, input, options) {
    const document = await this.request({ ...input, op: 'import_template', id }, options);
    return new DocumentSession(this.#transport, document, this.#options(options));
  }
  ingest(input, options) { return this.request({ op: 'ingest', input }, options); }
  dataReport(source, mapping, options) { return this.request({ op: 'data_report', source, mapping }, options); }
  designDefaults(options) { return this.request({ op: 'design_defaults' }, options); }
  designCapabilities(options) { return this.request({ op: 'design_capabilities' }, options); }
  designPresets(options) { return this.request({ op: 'design_presets' }, options); }
  objectCatalog(options) { return this.request({ op: 'object_catalog' }, options); }
  partCatalog(options) { return this.request({ op: 'part_catalog' }, options); }
  bestPracticeProfiles(options) { return this.request({ op: 'best_practice_profiles' }, options); }
  bestPracticeGuide(profileId, options) { return this.request({ op: 'best_practice_guide', profile_id: profileId }, options); }
  validateGuidedPresentation(input, options) { return this.request({ op: 'validate_guided_presentation', input }, options); }
  async createGuidedPresentation(id, input, options) {
    if (options?.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
    const result = await this.request({ op: 'create_guided_presentation', id, input }, options);
    if (options?.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
    return { session: new DocumentSession(this.#transport, result.document, this.#options(options)), validation: result.validation, profile_id: result.profile_id, model_inference: result.model_inference };
  }
  graphCatalog(options) { return this.request({ op: 'graph_catalog' }, options); }
  architectureIcons(options) { return this.request({ op: 'architecture_icons' }, options); }
  architectureIconAssets(ids, options) { return this.request({ op: 'architecture_icon_assets', ids }, options); }
  createGraphIcon(input, options) { return this.request({ ...input, op: 'create_graph_icon' }, options); }
  createGraph(input, options) { return this.request({ ...input, op: 'create_graph' }, options); }
  transformGraph(spec, operations, options) { return this.request({ op: 'transform_graph', spec, operations }, options); }
  createPart(input, options) { return this.request({ ...input, op: 'create_part' }, options); }
  createObject(input, options) { return this.request({ ...input, op: 'create_object' }, options); }
  createAsset(input, options) { return this.request({ ...input, op: 'create_asset' }, options); }
  async createPresentation(id, title = 'Untitled presentation', options) {
    const document = await this.request({ op: 'create_presentation', id, title }, options);
    return new DocumentSession(this.#transport, document, this.#options(options));
  }
  async openPresentation(id, base64, options) {
    const result = await this.request({ op: 'open_presentation', id, base64 }, options);
    return { session: new DocumentSession(this.#transport, result.document, this.#options(options)), warnings: result.warnings, objects: result.objects, format: result.format };
  }
  async importPresentation(id, base64, options) {
    const result = await this.request({ op: 'import_document', id, base64 }, options);
    return { session: new DocumentSession(this.#transport, result.document, this.#options(options)), warnings: result.warnings, objects: result.objects };
  }
  async createDocument(input, options) {
    const document = await this.request({ ...input, op: 'new_document' }, options);
    return new DocumentSession(this.#transport, document, this.#options(options));
  }
  async openProject({ base64, checkpoint }, options) {
    const document = await this.request({ op: 'open_project', base64, checkpoint }, options);
    return new DocumentSession(this.#transport, document, this.#options(options));
  }
}

export class DocumentSession {
  #request;
  #document;
  #past = [];
  #future = [];
  #busy = false;
  #fieldWarnings = [];
  #profile;
  #historyBoundary = null;
  constructor(transport, document, options = {}) {
    this.#profile = capacityProfile(options.capacityProfile);
    this.#request = (request, requestOptions) => transport(withProfile(request, request.capacity_profile ?? this.#profile), requestOptions);
    this.#document = structuredClone(document);
  }
  static async recover(transport, envelope, options = {}) {
    const client = new AislideClient(transport);
    const verified = await client.verifySessionRecovery(envelope, options);
    if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
    const session = new DocumentSession(transport, verified.document, { capacityProfile: verified.capacity_profile });
    session.#past = structuredClone(verified.past);
    session.#future = structuredClone(verified.future);
    session.#historyBoundary = verified.history_boundary;
    return session;
  }
  get capacityProfile() { return this.#profile; }
  get historyBoundary() { return this.#historyBoundary; }
  get recoveryEnvelope() {
    return structuredClone({ format: 'aislide.session', version: 1, capacity_profile: this.#profile, document: this.#document,
      past: this.#past, future: this.#future, history_boundary: this.#historyBoundary });
  }
  setCapacityProfile(profile, options = {}) {
    capacityProfile(profile);
    return this.#run(async () => {
      const envelope = { ...this.recoveryEnvelope, capacity_profile: profile };
      await this.#request({ op: 'verify_session_recovery', capacity_profile: profile, envelope }, { signal: options.signal });
      if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
      this.#profile = profile;
      return this.document;
    }, options);
  }
  get document() { return structuredClone(this.#document); }
  get revision() { return this.#document.revision; }
  get canUndo() { return this.#past.length > 0; }
  get canRedo() { return this.#future.length > 0; }
  get busy() { return this.#busy; }
  get fieldWarnings() { return [...this.#fieldWarnings]; }

  #retain(history, receipt) {
    if (!receipt) return;
    history.push(receipt);
    while (history.length > 30 || new TextEncoder().encode(JSON.stringify(history)).byteLength > 4 * 1024 * 1024) {
      history.shift();
      this.#historyBoundary = 'history_limit';
    }
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
  applyOperations(operations, options) { return this.#guardedAuthor('apply_operations', { operations }, options); }
  addElements(slideId, elements, options) { return this.applyOperations([{ op: 'add_elements', slide_id: slideId, elements }], options); }
  setFrames(slideId, frames, options) { return this.applyOperations(frames.map(({ id, frame }) => ({ op: 'set_frame', slide_id: slideId, id, frame })), options); }
  setTextStyle(slideId, input, options) { return this.applyOperations([{ ...input, op: 'set_text_style', slide_id: slideId }], options); }
  setSlideBackground(slideId, color, options) { return this.applyOperations([{ op: 'set_slide_background', slide_id: slideId, color }], options); }
  setConnector(slideId, input, options) { return this.applyOperations([{ ...input, op: 'set_connector', slide_id: slideId }], options); }
  setPictureCrop(slideId, input, options) { return this.applyOperations([{ ...input, op: 'set_picture_crop', slide_id: slideId }], options); }
  setHyperlink(slideId, input, options) { return this.applyOperations([{ ...input, op: 'set_hyperlink', slide_id: slideId }], options); }
  setShapeAdjustment(slideId, input, options) { return this.applyOperations([{ ...input, op: 'set_shape_adjustment', slide_id: slideId }], options); }
  addPicture(slideId, input, options) { return this.applyOperations([{ ...input, op: 'add_picture', slide_id: slideId }], options); }
  importSlides(source, input, options) { return this.#guardedAuthor('import_slides', { ...input, source }, options); }
  #guardedAuthor(op, input, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ ...input, op, document: this.#document, expected_revision: this.revision,
        expected_hash: options.expectedHash ?? this.#document.hash }, { signal: options.signal });
      return this.#accept(result, options);
    }, options);
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
  formatText(slideId, input, options) { return this.#part('format_text', slideId, input, options); }
  applyFormat(slideId, input, options) { return this.#part('apply_format', slideId, input, options); }
  setProofingLanguage(slideId, input, options) { return this.#part('set_proofing_language', slideId, input, options); }
  applyTextAssist(slideId, input, options) { return this.#part('apply_text_assist', slideId, input, options); }
  replaceTextContent(slideId, input, options) { return this.#part('replace_text_content', slideId, input, options); }
  updateParagraphs(slideId, input, options) { return this.#part('update_paragraphs', slideId, input, options); }
  updateRichNotes(slideId, paragraphs, options) { return this.#part('update_rich_notes', slideId, { paragraphs }, options); }
  updateAuxiliaryDesign(design, options) { return this.#author('update_auxiliary_design', { design }, options); }
  editTable(slideId, input, options) { return this.#part('edit_table', slideId, input, options); }
  applyImageEdit(slideId, input, options) { return this.#part('apply_image_edit', slideId, input, options); }
  addComment(slideId, comment, options) { return this.#part('add_comment', slideId, { comment }, options); }
  modernComment(slideId, operation, options) { return this.#part('modern_comment', slideId, { operation }, options); }
  setTableHeaders(slideId, elementId, policy, options) { return this.#part('set_table_headers', slideId, { element_id: elementId, policy }, options); }
  replyComment(slideId, parentId, comment, options) { return this.#part('reply_comment', slideId, { parent_id: parentId, comment }, options); }
  resolveComment(slideId, commentId, resolved, options) { return this.#part('resolve_comment', slideId, { comment_id: commentId, resolved }, options); }
  removeComment(slideId, commentId, options) { return this.#part('remove_comment', slideId, { comment_id: commentId }, options); }
  setAccessibility(slideId, elementId, metadata, options) { return this.#part('set_accessibility', slideId, { element_id: elementId, metadata }, options); }
  setReadingOrder(slideId, order, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ op: 'set_reading_order', document: this.#document, expected_revision: this.revision, slide_id: slideId, order }, { signal: options.signal });
      return { document: this.#accept(result, options), warnings: result.warnings };
    }, options);
  }
  updateNotes(slideId, notes, options = {}) {
    return this.#run(() => {
      this.#expectRevision(options);
      const index = this.#document.deck.slides.findIndex((slide) => slide.id === slideId);
      if (index < 0) throw new Error('Unknown slide ID');
      return this.#commit([{ op: 'replace', path: `/deck/slides/${index}/notes`, value: notes }], options);
    }, options);
  }
  #author(op, input, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ ...input, op, document: this.#document, expected_revision: this.revision }, { signal: options.signal });
      return this.#accept(result, options);
    }, options);
  }
  replaceText(input, options) { return this.#author('replace_text', { options: input }, options); }
  replaceFont(from, to, options) { return this.#author('replace_font', { from, to }, options); }
  embedFont(input, options) { return this.#author('embed_font', input, options); }
  setFontUsage(sha256, licenseAcknowledged, options) { return this.#author('set_font_usage', { sha256, license_acknowledged: licenseAcknowledged }, options); }
  resizeCanvas(input, options) { return this.#author('resize_canvas', input, options); }
  refreshFields(referenceDate, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ op: 'refresh_fields', document: this.#document, expected_revision: this.revision, reference_date: referenceDate, ...(options.referenceTime === undefined ? {} : { reference_time: options.referenceTime }), ...(options.locale === undefined ? {} : { locale: options.locale }) }, { signal: options.signal });
      const document = this.#accept(result, options);
      this.#fieldWarnings = [...(result.warnings ?? [])];
      return document;
    }, options);
  }
  combineShapes(slideId, input, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ ...input, op: 'combine_shapes', document: this.#document, expected_revision: this.revision, slide_id: slideId }, { signal: options.signal });
      const document = this.#accept(result.transaction, options);
      return { document, clipboard: result.clipboard, effects: result.effects, changes: result.transaction.changes };
    }, options);
  }
  editSelection(slideId, operation, { clipboard, ...options } = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ op: 'edit_selection', document: this.#document, expected_revision: this.revision, slide_id: slideId, operation, ...(clipboard === undefined ? {} : { clipboard }) }, { signal: options.signal });
      const document = this.#accept(result.transaction, options);
      return { document, clipboard: result.clipboard, effects: result.effects, changes: result.transaction.changes };
    }, options);
  }
  #read(request, options = {}) {
    return this.#run(async () => {
      const result = await this.#request(request, { signal: options.signal });
      if (options.signal?.aborted) throw new DOMException('Operation cancelled', 'AbortError');
      return result;
    }, options);
  }
  searchText(input, options) { return this.#read({ op: 'search_text', deck: this.#document.deck, options: input }, options); }
  textAssist(input, options) { return this.#read({ op: 'text_assist', input }, options); }
  textAiStatus(options) { return this.#read({ op: 'provider_status' }, options); }
  importProofingDictionary(input, options) { return this.#read({ ...input, op: 'import_proofing_dictionary' }, options); }
  proofText(input, options) { return this.#read({ ...input, op: 'proof_text' }, options); }
  copyFormat(slideId, input, options) { return this.#read({ ...input, op: 'copy_format', slide_id: slideId, document: this.#document }, options); }
  sampleSlidePixel(slideId, x, y, options) { return this.#read({ op: 'sample_slide_pixel', slide_id: slideId, x, y, document: this.#document }, options); }
  exportTemplate(kind, options) { return this.#read({ op: 'export_template', document: this.#document, kind }, options); }
  previewMasterImport(input, options = {}) {
    return this.#read({ op: 'preview_master_import', document: this.#document, expected_revision: options.expectedRevision ?? this.revision,
      expected_hash: options.expectedHash ?? this.#document.hash, input }, options);
  }
  importMasters(input, expectedCandidateHash, options = {}) {
    return this.#run(async () => {
      this.#expectRevision(options);
      const result = await this.#request({ op: 'import_masters', document: this.#document, expected_revision: this.revision,
        expected_hash: options.expectedHash ?? this.#document.hash, input, expected_candidate_hash: expectedCandidateHash }, { signal: options.signal });
      return this.#accept(result, options);
    }, options);
  }
  exportStatic(input = {}, options) { return this.#read({ op: 'export_static', document: this.#document, options: input }, options); }
  previewPresentation(input = {}, options) { return this.#read({ op: 'preview_presentation', document: this.#document, options: input }, options); }
  preflightPresentation(input = {}, options) { return this.#read({ op: 'preflight_presentation', document: this.#document, options: input }, options); }
  prepareDelivery(input = {}, options = {}) {
    return this.#read({ op: 'prepare_delivery', document: this.#document, expected_revision: options.expectedRevision ?? this.revision,
      expected_hash: options.expectedHash ?? this.#document.hash, options: input }, options);
  }
  previewSlideRevision(slideId, edits, { maxDimension, ...options } = {}) {
    return this.#read({ op: 'preview_slide_revision', document: this.#document, expected_revision: options.expectedRevision ?? this.revision,
      expected_hash: options.expectedHash ?? this.#document.hash, slide_id: slideId, edits, ...(maxDimension === undefined ? {} : { max_dimension: maxDimension }) }, options);
  }
  applySlideRevision(slideId, edits, { expectedRevision, expectedHash, candidateHash, ...options }) {
    return this.#run(async () => {
      const result = await this.#request({ op: 'apply_slide_revision', document: this.#document, expected_revision: expectedRevision,
        expected_hash: expectedHash, candidate_hash: candidateHash, slide_id: slideId, edits }, { signal: options.signal });
      return this.#accept(result, options);
    }, options);
  }
  checkAccessibility(options) { return this.#read({ op: 'check_accessibility', document: this.#document }, options); }
  inspectDocument(options) { return this.#read({ op: 'inspect_document', document: this.#document }, options); }
  listFonts(options) { return this.#read({ op: 'list_fonts', deck: this.#document.deck }, options); }
  exportCleanCopy(input, options) { return this.#read({ op: 'export_clean_copy', document: this.#document, options: input }, options); }
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
  setMasterTheme(masterId, theme, options) { return this.#transform('set_master_theme', { master_id: masterId, theme }, options); }
  setDesignField(field, options) { return this.#transform('set_design_field', { field }, options); }
  applyDesignPreset(presetId, options) { return this.#transform('apply_design_preset', { preset_id: presetId }, options); }
  applyTheme(theme, options) { return this.#transform('apply_theme', { theme }, options); }
  assignLayout(slideId, layoutId, options) { return this.#transform('assign_layout', { slide_id: slideId, layout_id: layoutId, ...(options?.preserveFreeform === undefined ? {} : { preserve_freeform: options.preserveFreeform }) }, options); }
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

export const FONT_LIMITS = Object.freeze({ face_bytes: 12 * 1048576, total_bytes: 24 * 1048576, faces: 8 });
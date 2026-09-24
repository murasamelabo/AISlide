import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { AislideClient, DocumentSession } from '../packages/client/index.mjs';
import { requestCore } from './core-client.mjs';
import { guidedExamples } from './guided-demo.mjs';

test('feedback SDK batches use one guarded request and one history receipt', async () => {
  const document = { id: 'feedback', revision: 2, hash: 'a'.repeat(64), deck: { slides: [] } };
  const changed = { ...document, revision: 3, hash: 'b'.repeat(64) };
  const receipt = { inverse: [], after_hash: changed.hash };
  const calls = [];
  const controller = new AbortController();
  const session = new DocumentSession(async (request, options) => {
    calls.push({ request: structuredClone(request), signal: options?.signal });
    return request.op === 'undo_transaction'
      ? { document: { ...document, revision: 4 }, receipt }
      : { document: changed, receipt };
  }, document, { capacityProfile: 'standard' });
  const operations = [{ op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' }];
  assert.deepEqual(await session.applyOperations(operations, { expectedRevision: 2, signal: controller.signal }), changed);
  assert.deepEqual(calls[0], { request: { op: 'apply_operations', document, expected_revision: 2, expected_hash: document.hash, operations, capacity_profile: 'standard' }, signal: controller.signal });
  assert.equal(calls.length, 1);
  assert.equal(session.recoveryEnvelope.past.length, 1);
  await session.undo();
  assert.equal(session.canUndo, false);
  assert.equal(session.canRedo, true);
  const before = session.recoveryEnvelope;
  await assert.rejects(() => session.applyOperations(operations, { expectedRevision: 2 }), /conflict/i);
  assert.equal(calls.length, 2);
  assert.deepEqual(session.recoveryEnvelope, before);
});

test('managed batch SDK forwards all managed variants once and preserves individual routes', async () => {
  const document = { id: 'managed-sdk', revision: 0, hash: 'a'.repeat(64), deck: { slides: [] }, sources: [], bindings: [], parts: [] };
  const layout = { x: 64, y: 120, width: 1152, height: 512, show_title: false };
  const part = { version: 1, preset: 'matrix/balanced', title: 'Synthetic', data: { kind: 'matrix', corner_label: 'Criterion', rows: ['A', 'B'], columns: ['C', 'D'], cells: [['a', 'b'], ['c', 'd']] }, layout };
  const graph = { version: 1, title: 'Synthetic', show_title: false, nodes: [{ id: 'node', label: 'Node', x: 0, y: 0 }] };
  const operations = [
    { op: 'add_part', slide_id: 'slide-1', id: 'part', spec: part },
    { op: 'update_part', slide_id: 'slide-1', id: 'part', spec: { ...part, title: 'Updated part' } },
    { op: 'add_graph', slide_id: 'slide-1', id: 'graph', spec: graph, layout },
    { op: 'update_graph', slide_id: 'slide-1', id: 'graph', spec: { ...graph, title: 'Updated graph' } },
  ];
  const changed = { ...document, revision: 1, hash: 'b'.repeat(64), parts: [
    { slide_id: 'slide-1', element_id: 'part', spec: operations[1].spec, render_sha256: 'c'.repeat(64), stale: false },
    { slide_id: 'slide-1', element_id: 'graph', spec: { version: 1, preset: 'diagram/custom', title: 'Updated graph', data: { kind: 'diagram', graph: operations[3].spec }, layout }, render_sha256: 'd'.repeat(64), stale: false },
  ] };
  const calls = [];
  const controller = new AbortController();
  const transport = async (request, options) => {
    calls.push({ request: structuredClone(request), signal: options?.signal });
    return { document: changed, receipt: { inverse: [], after_hash: changed.hash } };
  };
  const session = new DocumentSession(transport, document, { capacityProfile: 'standard' });
  assert.deepEqual(await session.applyOperations(operations, { expectedRevision: 0, signal: controller.signal }), changed);
  assert.deepEqual(calls, [{ request: { op: 'apply_operations', document, expected_revision: 0, expected_hash: document.hash, operations, capacity_profile: 'standard' }, signal: controller.signal }]);
  assert.equal(session.recoveryEnvelope.past.length, 1);
  for (const [method, op, spec] of [['addPart', 'insert_part', part], ['updatePart', 'update_part', part], ['addGraph', 'insert_graph', graph], ['updateGraph', 'update_graph', graph]]) {
    calls.length = 0;
    const legacy = new DocumentSession(transport, document);
    await legacy[method]('slide-1', { id: 'root', spec }, { expectedRevision: 0, signal: controller.signal });
    assert.deepEqual(calls, [{ request: { op, document, expected_revision: 0, slide_id: 'slide-1', id: 'root', spec }, signal: controller.signal }]);
    assert.equal(legacy.recoveryEnvelope.past.length, 1);
  }
});

test('feedback SDK convenience APIs forward complete operations without extra core validation', async () => {
  const document = { id: 'feedback-helpers', revision: 0, hash: 'a'.repeat(64), deck: { slides: [] } };
  const frame = { x: 10, y: 20, width: 300, height: 120 };
  const element = { type: 'text', id: 'text', ...frame, text: 'Synthetic', font_size: 24, color: '@dk1', bold: false, format: { paragraphs: [{ runs: [{ text: 'Synthetic', style: { italic: true, language: 'en-US' } }] }] } };
  const cases = [
    ['addElements', [[element]], [{ op: 'add_elements', elements: [element] }]],
    ['setFrames', [[{ id: 'text', frame }, { id: 'group', frame }]], [{ op: 'set_frame', id: 'text', frame }, { op: 'set_frame', id: 'group', frame }]],
    ['setTextStyle', [{ ids: ['text'], style: { bold: false, color: '@accent1' } }], [{ op: 'set_text_style', ids: ['text'], style: { bold: false, color: '@accent1' } }]],
    ['setSlideBackground', ['FFFFFF'], [{ op: 'set_slide_background', color: 'FFFFFF' }]],
    ['setConnector', [{ id: 'edge', connector: { color: '@dk1', stroke_width: 2, arrow: true }, frame }], [{ op: 'set_connector', id: 'edge', connector: { color: '@dk1', stroke_width: 2, arrow: true }, frame }]],
    ['setPictureCrop', [{ id: 'image', crop: { left: 0.1 } }], [{ op: 'set_picture_crop', id: 'image', crop: { left: 0.1 } }]],
    ['setHyperlink', [{ id: 'text', link: null }], [{ op: 'set_hyperlink', id: 'text', link: null }]],
    ['setShapeAdjustment', [{ id: 'shape', adjustment: { name: 'adj', value: 25000 } }], [{ op: 'set_shape_adjustment', id: 'shape', adjustment: { name: 'adj', value: 25000 } }]],
    ['addPicture', [{ id: 'image', base64: 'c3ludGhldGlj', mime_type: 'image/png', alt: 'Synthetic', frame, crop: { top: 0.1 } }], [{ op: 'add_picture', id: 'image', base64: 'c3ludGhldGlj', mime_type: 'image/png', alt: 'Synthetic', frame, crop: { top: 0.1 } }]],
  ];
  for (const [method, args, operations] of cases) {
    const calls = [];
    const session = new DocumentSession(async request => {
      calls.push(structuredClone(request));
      return { document: { ...document, revision: 1 }, receipt: { inverse: [] } };
    }, document);
    await session[method]('slide-1', ...args, { expectedRevision: 0, expectedHash: 'b'.repeat(64) });
    assert.deepEqual(calls, [{ op: 'apply_operations', document, expected_revision: 0, expected_hash: 'b'.repeat(64), operations: operations.map(operation => ({ ...operation, slide_id: 'slide-1' })) }], method);
    assert.equal(session.recoveryEnvelope.past.length, 1, method);
  }
});

test('feedback SDK slide import forwards a source snapshot with guarded history', async () => {
  const document = { id: 'target', revision: 0, hash: 'a'.repeat(64), deck: { slides: [] } };
  const source = { ...document, id: 'source', sources: [{ id: 'evidence' }], bindings: [], parts: [] };
  const original = structuredClone(source);
  const input = { source_slide_ids: ['slide-1'], prefix: 'merged', after: null };
  const calls = [];
  const session = new DocumentSession(async request => {
    calls.push(structuredClone(request));
    return { document: { ...document, revision: 1 }, receipt: { inverse: [] } };
  }, document);
  await session.importSlides(source, input, { expectedRevision: 0 });
  assert.deepEqual(calls, [{ op: 'import_slides', document, expected_revision: 0, expected_hash: document.hash, source, ...input }]);
  assert.deepEqual(source, original);
  assert.equal(session.recoveryEnvelope.past.length, 1);
});

test('feedback SDK batch and import preserve state on failure cancellation and busy', async () => {
  const document = { id: 'feedback-guards', revision: 0, hash: 'a'.repeat(64), deck: { slides: [] } };
  const operations = [
    { op: 'add_part', slide_id: 'slide-1', id: 'part', spec: { version: 1, preset: 'list-horizontal/balanced', title: 'Synthetic', data: { kind: 'items', items: [{ label: 'First' }, { label: 'Second' }] } } },
    { op: 'add_graph', slide_id: 'slide-1', id: 'graph', spec: { version: 1, title: 'Synthetic', nodes: [{ id: 'node', label: 'Node', x: 0, y: 88 }] } },
    { op: 'set_slide_background', slide_id: 'slide-1', color: 'FFFFFF' },
  ];
  const invoke = (session, method, options) => method === 'batch' ? session.applyOperations(operations, options)
    : session.importSlides(document, { source_slide_ids: ['slide-1'], prefix: 'copy' }, options);
  for (const method of ['batch', 'import']) {
    for (const mode of ['failure', 'late-cancel', 'early-cancel', 'revision']) {
      const controller = new AbortController();
      let calls = 0;
      const session = new DocumentSession(async (_request, options) => {
        calls += 1;
        assert.equal(options.signal, controller.signal);
        if (mode === 'failure') throw new Error('Core rejected operation');
        controller.abort();
        return { document: { ...document, revision: 1 }, receipt: { inverse: [] } };
      }, document);
      const before = session.recoveryEnvelope;
      if (mode === 'early-cancel') controller.abort();
      await assert.rejects(() => invoke(session, method, { signal: controller.signal, expectedRevision: mode === 'revision' ? 99 : 0 }), /cancelled|rejected|conflict/i);
      assert.equal(calls, ['early-cancel', 'revision'].includes(mode) ? 0 : 1);
      assert.deepEqual(session.recoveryEnvelope, before);
      assert.equal(session.busy, false);
    }
    let release;
    const session = new DocumentSession(() => new Promise(resolve => { release = resolve; }), document);
    const pending = invoke(session, method);
    assert.equal(session.busy, true);
    try {
      await assert.rejects(() => invoke(session, 'batch'), /busy/i);
      await assert.rejects(() => invoke(session, 'import'), /busy/i);
    } finally { release({ document, receipt: null }); await pending; }
    assert.equal(session.busy, false);
  }
});

test('feedback SDK types accept the new contracts and reject raw or mistyped operations', async () => {
  const ts = (await import('typescript')).default;
  const { fileURLToPath } = await import('node:url');
  const filename = fileURLToPath(new URL('../packages/client/feedback-types.mts', import.meta.url)).replaceAll('\\', '/');
  const preamble = `
    import type { DocumentSession, AislideDocument, AuthoringOptions, AuthoringOperation, Frame, Crop,
      Connection, ConnectorRouting, ConnectorSettings, PictureInput, SlideImportInput, GraphSpec, PartSpec, GuidedAuthoring } from './index.mjs';
    declare const session: DocumentSession;
    declare const source: AislideDocument;
    const options: AuthoringOptions = { expectedRevision: 0, expectedHash: 'hash', signal: new AbortController().signal };
    const frame: Frame = { x: 0, y: 0, width: 300, height: 150 };
    const crop: Crop = { top: 0.1 };
    const connection: Connection = { element_id: 'text', site: 0 };
    const routing: ConnectorRouting = { points: [[0, 0], [1, 1]] };
    const connector: ConnectorSettings = { color: '@dk1', stroke_width: 1, arrow: true, start: connection, end: null, routing };
    const picture: PictureInput = { id: 'image', base64: 'synthetic', mime_type: 'image/png', alt: 'Synthetic', frame, crop };
    const imported: SlideImportInput = { source_slide_ids: ['slide-1'], prefix: 'copy', after: null };
    const operations: AuthoringOperation[] = [{ op: 'set_frame', slide_id: 'slide-1', id: 'text', frame }];
  `;
  const check = body => {
    const sourceText = preamble + body;
    const compilerOptions = { noEmit: true, strict: true, skipLibCheck: false, target: ts.ScriptTarget.ES2023, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler };
    const host = ts.createCompilerHost(compilerOptions);
    const getSourceFile = host.getSourceFile.bind(host);
    host.getSourceFile = (path, languageVersion, onError, shouldCreateNewSourceFile) => path === filename
      ? ts.createSourceFile(filename, sourceText, languageVersion, true)
      : getSourceFile(path, languageVersion, onError, shouldCreateNewSourceFile);
    return ts.getPreEmitDiagnostics(ts.createProgram([filename], compilerOptions, host));
  };
  const diagnostics = check(`
    const results: Promise<AislideDocument>[] = [
      session.applyOperations(operations, options),
      session.addElements('slide-1', [{ type: 'text', id: 'text', ...frame, text: 'Synthetic', font_size: 24, color: '@dk1', bold: false }], options),
      session.setFrames('slide-1', [{ id: 'text', frame }], options),
      session.setTextStyle('slide-1', { ids: ['text'], style: { bold: false } }, options),
      session.setSlideBackground('slide-1', 'FFFFFF', options),
      session.setConnector('slide-1', { id: 'edge', connector, frame }, options),
      session.setPictureCrop('slide-1', { id: 'image', crop }, options),
      session.setHyperlink('slide-1', { id: 'text', link: null }, options),
      session.setShapeAdjustment('slide-1', { id: 'shape', adjustment: { name: 'adj', value: 25000 } }, options),
      session.addPicture('slide-1', picture, options), session.importSlides(source, imported, options),
    ];
    const graph: GraphSpec = { version: 1, title: 'Synthetic', show_title: false, nodes: [{ id: 'node', label: 'Heading', detail: 'Detail', detail_font_size: 12, text_align: 'right', heading_bold: false, x: 0, y: 0, height: 512 }] };
    const part: PartSpec = { version: 1, preset: 'synthetic', title: 'Synthetic', data: { kind: 'diagram', graph }, layout: { ...frame, show_title: false } };
    const matrix: PartSpec = { ...part, data: { kind: 'matrix', corner_label: 'Criterion', rows: ['A', 'B'], columns: ['C', 'D'], cells: [['a', 'b'], ['c', 'd']] } };
    const managed: AuthoringOperation[] = [
      { op: 'add_part', slide_id: 'slide-1', id: 'part', spec: matrix },
      { op: 'update_part', slide_id: 'slide-1', id: 'part', spec: part },
      { op: 'add_graph', slide_id: 'slide-1', id: 'graph', spec: graph, layout: { ...frame, show_title: false } },
      { op: 'add_graph', slide_id: 'slide-1', id: 'graph-default', spec: graph },
      { op: 'add_graph', slide_id: 'slide-1', id: 'graph-null', spec: graph, layout: null },
      { op: 'update_graph', slide_id: 'slide-1', id: 'graph', spec: graph },
    ];
    results.push(session.applyOperations(managed, options), session.addPart('slide-1', { id: 'part', spec: part }),
      session.updatePart('slide-1', { id: 'part', spec: part }), session.addGraph('slide-1', { id: 'graph', spec: graph }),
      session.updateGraph('slide-1', { id: 'graph', spec: graph }));
    const authoring: GuidedAuthoring = { headline_style: 'keyword', slide_limit: 128 };
    void [results, part, authoring];
  `);
  assert.deepEqual(diagnostics.map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')), []);
  for (const invalid of [
    "session.applyOperations([{ op: 'replace', path: '/deck', value: {} }]);",
    "session.setTextStyle('slide-1', { ids: ['text'], style: { font_size: 'large' } });",
    "const invalid: GuidedAuthoring = { headline_style: 'freeform' };",
    "session.addPicture('slide-1', { ...picture, mime_type: 'image/svg+xml' });",
    "session.applyOperations([{ op: 'add_part', slide_id: 'slide-1', id: 'part', spec: {} }]);",
    "session.applyOperations([{ op: 'add_graph', slide_id: 'slide-1', id: 'graph', spec: { version: 1, title: '', nodes: [] }, layout: { ...frame, unknown: true } }]);",
    "session.applyOperations([{ op: 'update_graph', slide_id: 'slide-1', id: 'graph', spec: { version: 1, title: '', nodes: [] }, layout: frame }]);",
    "session.applyOperations([{ op: 'add_part', slide_id: 'slide-1', id: 'part', spec: { version: 1, preset: 'matrix', title: '', data: { kind: 'matrix', rows: [], columns: [], cells: [], corner_label: 1 } } }]);",
    "session.addGraph('slide-1', { id: 'graph', spec: { version: 1, title: '', nodes: [] }, layout: frame });",
  ]) assert.ok(check(invalid).length > 0, invalid);
});

test('master import SDK forwards guarded requests, capacity and one Undo', async () => {
  const document = { id: 'master-import-sdk', revision: 2, hash: 'a'.repeat(64), deck: { slides: [] } };
  const input = { kind: 'potx', base64: 'c3ludGhldGlj', source_sha256: 'b'.repeat(64), mode: 'masters', ids: ['master-1'], prefix: 'imported', name: 'Synthetic master' };
  const inspection = { kind: 'potx', source_sha256: input.source_sha256, width: 1280, height: 720, masters: [], slides: [], warnings: [], office_visual_parity: false };
  const preview = { base_revision: 2, base_hash: document.hash, source_sha256: input.source_sha256, candidate_hash: 'c'.repeat(64), design: {}, master_ids: ['imported-master-1'], layout_ids: [], preview_slides: [], warnings: [], office_visual_parity: false };
  const changed = { ...document, revision: 3, hash: preview.candidate_hash };
  const receipt = { inverse: [], after_hash: changed.hash };
  const calls = [];
  const controller = new AbortController();
  const transport = async (request, options) => {
    calls.push({ request: structuredClone(request), signal: options?.signal });
    if (request.op === 'inspect_master_source') return inspection;
    if (request.expected_revision !== (request.op === 'undo_transaction' ? 3 : 2)) throw new Error('Revision conflict');
    if (request.op !== 'undo_transaction' && request.expected_hash !== document.hash) throw new Error('Hash conflict');
    if (request.op === 'preview_master_import') return preview;
    if (request.op === 'import_masters') return { document: changed, receipt };
    assert.equal(request.op, 'undo_transaction');
    assert.deepEqual(request.receipt, receipt);
    return { document: { ...document, revision: 4 }, receipt: { inverse: [], after_hash: document.hash } };
  };
  const client = new AislideClient(transport, { capacityProfile: 'standard' });
  assert.equal(await client.inspectMasterSource({ kind: input.kind, base64: input.base64 }, { signal: controller.signal }), inspection);
  assert.deepEqual(calls[0].request, { op: 'inspect_master_source', kind: input.kind, base64: input.base64, capacity_profile: 'standard' });
  const session = new DocumentSession(transport, document, { capacityProfile: 'legacy' });
  const before = session.recoveryEnvelope;
  assert.equal(await session.previewMasterImport(input, { signal: controller.signal }), preview);
  assert.deepEqual(session.recoveryEnvelope, before);
  assert.deepEqual(calls[1].request, { op: 'preview_master_import', document, expected_revision: 2, expected_hash: document.hash, input, capacity_profile: 'legacy' });
  assert.deepEqual(await session.importMasters(input, preview.candidate_hash, { signal: controller.signal }), changed);
  assert.deepEqual(calls[2].request, { op: 'import_masters', document, expected_revision: 2, expected_hash: document.hash, input, expected_candidate_hash: preview.candidate_hash, capacity_profile: 'legacy' });
  assert.ok(calls.every(call => call.signal === controller.signal));
  assert.equal(session.recoveryEnvelope.past.length, 1);
  await session.undo();
  assert.equal(session.document.hash, document.hash);
  assert.equal(session.canUndo, false);
  assert.equal(session.canRedo, true);
  const guarded = new DocumentSession(transport, document);
  for (const options of [{ expectedRevision: 1 }, { expectedHash: 'd'.repeat(64) }]) {
    await assert.rejects(() => guarded.previewMasterImport(input, options), /conflict/i);
    await assert.rejects(() => guarded.importMasters(input, preview.candidate_hash, options), /conflict/i);
    assert.deepEqual(guarded.recoveryEnvelope.document, document);
    assert.equal(guarded.canUndo, false);
  }
});

test('master import SDK cancellation and busy guards retain document and history', async () => {
  const document = { id: 'master-import-cancel', revision: 0, hash: 'a'.repeat(64), deck: { slides: [] } };
  const input = { kind: 'pptx', base64: 'c3ludGhldGlj', source_sha256: 'b'.repeat(64), mode: 'slides', ids: ['slide-1'], prefix: 'imported', name: 'Synthetic slide' };
  for (const method of ['inspect', 'preview', 'import']) {
    const controller = new AbortController();
    let calls = 0;
    const transport = async (_request, options) => {
      calls += 1;
      assert.equal(options.signal, controller.signal);
      controller.abort();
      return { document: { ...document, revision: 1, hash: 'c'.repeat(64) }, receipt: { inverse: [] } };
    };
    const client = new AislideClient(transport);
    const session = new DocumentSession(transport, document);
    const before = session.recoveryEnvelope;
    const invoke = () => method === 'inspect' ? client.inspectMasterSource({ kind: input.kind, base64: input.base64 }, { signal: controller.signal })
      : method === 'preview' ? session.previewMasterImport(input, { signal: controller.signal })
        : session.importMasters(input, 'c'.repeat(64), { signal: controller.signal });
    await assert.rejects(invoke, { name: 'AbortError' });
    await assert.rejects(invoke, { name: 'AbortError' });
    assert.equal(calls, 1);
    assert.equal(session.busy, false);
    assert.deepEqual(session.recoveryEnvelope, before);
  }
  let release;
  const pending = new Promise(resolve => { release = resolve; });
  const session = new DocumentSession(() => pending, document);
  const before = session.recoveryEnvelope;
  const reading = session.previewMasterImport(input);
  assert.equal(session.busy, true);
  try {
    await assert.rejects(() => session.previewMasterImport(input), /busy/i);
    await assert.rejects(() => session.importMasters(input, 'c'.repeat(64)), /busy/i);
  } finally { release({ candidate_hash: 'c'.repeat(64) }); await reading; }
  assert.equal(session.busy, false);
  assert.deepEqual(session.recoveryEnvelope, before);
});

test('P1 delivery preparation forwards one guarded snapshot and discards cancelled results', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('delivery-sdk', 'Synthetic delivery');
  const before = session.recoveryEnvelope;
  const input = { preview: 'none', preflight: false, notes: true };
  const prepared = await session.prepareDelivery(input, { expectedRevision: session.revision, expectedHash: session.document.hash });
  assert.deepEqual(prepared.files.map(file => file.kind), ['pptx', 'notes']);
  assert.equal(prepared.manifest.document.hash, before.document.hash);
  assert.deepEqual(session.recoveryEnvelope, before);
  await assert.rejects(() => session.prepareDelivery(input, { expectedRevision: 999 }), /revision|hash/i);
  let calls = 0;
  const cancelled = new AbortController();
  const reader = new DocumentSession(async request => {
    calls += 1;
    assert.equal(request.op, 'prepare_delivery');
    assert.deepEqual(request.options, input);
    assert.equal(request.expected_hash, before.document.hash);
    cancelled.abort();
    return prepared;
  }, before.document);
  await assert.rejects(() => reader.prepareDelivery(input, { signal: cancelled.signal }), { name: 'AbortError' });
  await assert.rejects(() => reader.prepareDelivery(input, { signal: cancelled.signal }), { name: 'AbortError' });
  assert.equal(calls, 1);
  assert.deepEqual(reader.recoveryEnvelope, before);
});

test('P0 SDK preview, diagnostics and revision calls retain state on early and late cancellation', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('cancel-authoring', 'Synthetic cancellation');
  const original = session.recoveryEnvelope;
  const transportCalls = [];
  const controller = new AbortController();
  let late = false;
  const checked = new DocumentSession(async request => {
    transportCalls.push(request);
    if (late) controller.abort();
    return request.op === 'apply_slide_revision' ? { document: { ...session.document, revision: 1 }, receipt: { inverse: [], after_hash: 'candidate' } } : { revision: 0, hash: session.document.hash };
  }, session.document);
  const input = { page_indices: [0], max_dimension: 320 };
  await checked.previewPresentation(input);
  await checked.preflightPresentation({ page_indices: [0] });
  const edits = [{ op: 'replace_text', id: 'sample', text: 'candidate' }];
  await checked.previewSlideRevision('slide-1', edits, { maxDimension: 320, expectedRevision: 0, expectedHash: session.document.hash });
  assert.deepEqual(transportCalls.map(request => request.op), ['preview_presentation', 'preflight_presentation', 'preview_slide_revision']);
  assert.deepEqual(transportCalls[0].options, input);
  assert.equal(transportCalls[2].expected_hash, session.document.hash);
  assert.deepEqual(checked.recoveryEnvelope, original);
  late = true;
  await assert.rejects(() => checked.applySlideRevision('slide-1', edits, { expectedRevision: 0, expectedHash: session.document.hash, candidateHash: 'candidate', signal: controller.signal }), { name: 'AbortError' });
  assert.deepEqual(checked.recoveryEnvelope, original);
  const count = transportCalls.length;
  for (const operation of [
    () => checked.previewPresentation(input, { signal: controller.signal }),
    () => checked.preflightPresentation({}, { signal: controller.signal }),
    () => checked.previewSlideRevision('slide-1', edits, { signal: controller.signal }),
    () => checked.applySlideRevision('slide-1', edits, { expectedRevision: 0, expectedHash: session.document.hash, candidateHash: 'candidate', signal: controller.signal }),
  ]) await assert.rejects(operation, { name: 'AbortError' });
  assert.equal(transportCalls.length, count);
  for (const method of ['previewPresentation', 'preflightPresentation', 'previewSlideRevision']) {
    const cancelled = new AbortController();
    const reader = new DocumentSession(async () => { cancelled.abort(); return {}; }, session.document);
    await assert.rejects(() => method === 'previewSlideRevision' ? reader[method]('slide-1', edits, { signal: cancelled.signal }) : reader[method]({}, { signal: cancelled.signal }), { name: 'AbortError' });
    assert.deepEqual(reader.recoveryEnvelope, original);
  }
});

test('P0 native revision preserves opaque slide XML, shared media and exact Undo', async () => {
  const { unzipSync, zipSync } = await import('fflate');
  const client = new AislideClient(requestCore);
  const authored = await client.createPresentation('opaque-revision-author', 'Synthetic preservation');
  const picture = await client.createAsset({ id: 'shared-picture', mime_type: 'image/svg+xml', size: 64, alt: 'Synthetic rectangle', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>').toString('base64') });
  await authored.transact([
    { op: 'add', path: '/deck/slides/0/elements/-', value: { type: 'text', id: 'native-label', x: 30, y: 30, width: 500, height: 90, text: 'Before', font_size: 24, color: '000000', bold: false } },
    { op: 'add', path: '/deck/slides/0/elements/-', value: { ...picture, x: 30, y: 160 } },
  ]);
  await authored.editSlides([{ op: 'duplicate', slide_id: 'slide-1', id: 'untouched' }]);
  const parts = unzipSync(Buffer.from((await authored.exportPresentation()).base64, 'base64'));
  const marker = '<p:extLst><p:ext uri="urn:aislide:test:opaque"><check:state xmlns:check="urn:aislide:test:opaque">retain-unknown-xml</check:state></p:ext></p:extLst>';
  parts['ppt/slides/slide1.xml'] = Buffer.from(Buffer.from(parts['ppt/slides/slide1.xml']).toString('utf8').replace('</p:sld>', `${marker}</p:sld>`));
  const original = Buffer.from(zipSync(parts));
  const { session } = await client.openPresentation('opaque-revision', original.toString('base64'));
  const before = session.recoveryEnvelope;
  const slide = session.document.deck.slides[0];
  const text = slide.elements.find(element => element.type === 'text');
  const edits = [{ op: 'replace_text', id: text.id, text: 'After' }];
  const candidate = await session.previewSlideRevision(slide.id, edits, { maxDimension: 320 });
  assert.deepEqual(session.recoveryEnvelope, before);
  await session.applySlideRevision(slide.id, edits, { expectedRevision: candidate.base_revision, expectedHash: candidate.base_hash, candidateHash: candidate.candidate_hash });
  const changed = unzipSync(Buffer.from((await session.exportPresentation()).base64, 'base64'));
  assert.match(Buffer.from(changed['ppt/slides/slide1.xml']).toString('utf8'), /retain-unknown-xml/);
  const preserved = Object.keys(parts).filter(path => path.startsWith('ppt/media/') || path === 'ppt/slides/slide2.xml' || path === 'ppt/slides/_rels/slide2.xml.rels');
  assert.ok(preserved.some(path => path.startsWith('ppt/media/')));
  for (const path of preserved) assert.deepEqual(changed[path], parts[path], path);
  assert.deepEqual(session.document.origin, before.document.origin);
  await session.undo();
  assert.deepEqual(Buffer.from((await session.exportPresentation()).base64, 'base64'), original);
});

test('architecture icon SDK preserves explicit IDs, options, response and cancellation', async () => {
  const requests = [];
  const catalog = { version: 1, release: '2026-09-20', configured: false, message: 'Synthetic not-installed fixture', providers: [], icons: [] };
  const assets = { icons: [{ id: 'aws/test/service', base64: 'synthetic', mime_type: 'image/png', alt: 'Full synthetic service name', width: 512, height: 128 }] };
  const client = new AislideClient(async (request, options) => {
    requests.push({ request, options });
    return request.op === 'architecture_icons' ? catalog : assets;
  });
  const controller = new AbortController();
  const options = { signal: controller.signal };
  assert.equal(await client.architectureIcons(options), catalog);
  const ids = ['aws/test/service', 'azure/test/service'];
  assert.equal(await client.architectureIconAssets(ids, options), assets);
  assert.deepEqual(requests.map(({ request }) => request), [{ op: 'architecture_icons' }, { op: 'architecture_icon_assets', ids }]);
  assert.ok(requests.every(request => request.options === options));
  controller.abort();
  await assert.rejects(() => client.architectureIcons(options), { name: 'AbortError' });
  await assert.rejects(() => client.architectureIconAssets(ids, options), { name: 'AbortError' });
  assert.equal(requests.length, 2);
  const late = new AbortController();
  const lateClient = new AislideClient(async () => { late.abort(); return assets; });
  await assert.rejects(() => lateClient.architectureIconAssets(ids, { signal: late.signal }), { name: 'AbortError' });
});

test('phase6 SDK modern native workflow, exact Undo and masked inspection', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('phase6-sdk', 'Synthetic review');
  const draft = { author_name: 'Offline reviewer', created: '2026-09-19T10:00:00Z', body: [{ runs: [{ text: 'sentinel@example.invalid +1 (202) 555-0147' }] }] };
  await session.modernComment('slide-1', { type: 'create', draft, anchor: { kind: 'unknown' } });
  const source = await session.exportPresentation();
  const { session: opened } = await client.openPresentation('phase6-native', source.base64);
  const thread = opened.document.deck.slides[0].review.modern_threads[0];
  await opened.modernComment('slide-1', { type: 'reply', thread_id: thread.id, draft });
  await opened.modernComment('slide-1', { type: 'set_status', comment_id: thread.id, status: 'closed' });
  await opened.modernComment('slide-1', { type: 'update_body', comment_id: thread.id, body: [{ runs: [{ text: 'Updated synthetic rich body', style: { bold: true } }] }] });
  const { session: reopened } = await client.openPresentation('phase6-reopened', (await opened.exportPresentation()).base64);
  assert.equal(reopened.document.deck.slides[0].review.modern_threads[0].status, 'closed');
  assert.equal(reopened.document.deck.slides[0].review.modern_threads[0].replies.length, 1);
  assert.equal(reopened.document.deck.slides[0].review.modern_threads[0].body[0].runs[0].text, 'Updated synthetic rich body');
  const inspection = await reopened.inspectDocument();
  assert.ok(inspection.candidates.some(candidate => candidate.rule === 'email_candidate'));
  assert.equal(JSON.stringify(inspection).includes('example.invalid'), false);
  assert.equal(JSON.stringify(inspection).includes('555-0147'), false);
  await assert.rejects(() => reopened.exportCleanCopy({ new_document_id: 'no-pii-delete', categories: ['email_candidate'], confirmed: true }));
  await opened.undo(); await opened.undo(); await opened.undo();
  assert.equal((await opened.exportPresentation()).base64, source.base64);
  await opened.transact([{ op: 'add', path: '/deck/slides/0/elements/-', value: { type: 'table', id: 'headers', x: 40, y: 40, width: 700, height: 200, rows: [['', 'Column'], ['Row', 'Value']], font_size: 20 } }]);
  await opened.setTableHeaders('slide-1', 'headers', 'both');
  assert.ok((await opened.checkAccessibility()).issues.some(issue => issue.code === 'table_declared_header_empty' && issue.repair_target === 'table_headers'));
  const { session: headers } = await client.openPresentation('phase6-headers', (await opened.exportPresentation()).base64);
  assert.equal(headers.document.deck.slides[0].review.table_headers.headers, 'both');
});

test('phase6 SDK omitted modern fields preserve unsaved current model rather than stale origin', async () => {
  const client = new AislideClient(requestCore);
  let session = await client.createPresentation('phase6-omission', 'Synthetic omission');
  const draft = { author_name: 'Offline', created: '2026-09-19T10:00:00Z', body: [{ runs: [{ text: 'Initial' }] }] };
  await session.modernComment('slide-1', { type: 'create', draft, anchor: { kind: 'unknown' } });
  for (const native of [false, true]) {
    if (native) ({ session } = await client.openPresentation('phase6-omission-native', (await session.exportPresentation()).base64));
    const id = session.document.deck.slides[0].review.modern_threads[0].id;
    await session.modernComment('slide-1', { type: 'reply', thread_id: id, draft: { ...draft, body: [{ runs: [{ text: 'Unsaved reply' }] }] } });
    await session.modernComment('slide-1', { type: 'set_status', comment_id: id, status: 'closed' });
    const before = session.document;
    const olderClient = session.document.deck;
    delete olderClient.slides[0].review.modern_threads;
    olderClient.title = 'Older client unrelated edit';
    await session.replaceDeck(olderClient);
    assert.deepEqual(session.document.deck.slides[0].review.modern_threads, before.deck.slides[0].review.modern_threads);
    await session.undo();
    assert.equal(session.document.hash, before.hash);
    const explicit = session.document.deck;
    explicit.slides[0].review.modern_threads = [];
    await session.replaceDeck(explicit);
    assert.deepEqual(session.document.deck.slides[0].review.modern_threads, []);
    const { session: removed } = await client.openPresentation('phase6-explicit-empty', (await session.exportPresentation()).base64);
    assert.equal(removed.document.deck.slides[0].review?.modern_threads?.length ?? 0, 0);
    await session.undo();
    assert.equal(session.document.hash, before.hash);
  }
});

test('phase6 SDK authored modern metadata cannot be forged by raw transactions', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('phase6-metadata', 'Synthetic metadata');
  await session.modernComment('slide-1', { type: 'create', anchor: { kind: 'unknown' }, draft: { author_name: 'Offline original', created: '2026-09-19T10:00:00Z', body: [{ runs: [{ text: 'Original' }] }] } });
  const original = session.document;
  for (const [path, value] of [['/author/name', 'Forged'], ['/created', '2026-09-19T11:00:00Z'], ['/anchor', { kind: 'preserved' }]]) {
    await assert.rejects(() => session.transact([{ op: 'replace', path: '/deck/slides/0/review/modern_threads/0' + path, value }]));
    assert.equal(session.document.hash, original.hash);
  }
});

test('phase3 SDK projection and WordArt previews preserve native state and exact Undo', async () => {
  const client = new AislideClient(requestCore);
  const authored = await client.createPresentation('phase3-sdk', 'Synthetic statistics');
  const element = { type: 'chart', id: 'statistics', x: 30, y: 30, width: 800, height: 450, kind: 'line', categories: ['1','2','3'], series: [{ name: 'Measured', values: [3,5,7], color: '087F73', trendline: { kind: 'linear', forward: 1 }, error_bars: { kind: 'fixed_value', value: 1 } }] };
  await authored.transact([{ op: 'add', path: '/deck/slides/0/elements/-', value: element }]);
  const source = await authored.exportPresentation();
  const { session } = await client.openPresentation('phase3-opened', source.base64);
  const before = session.document;
  const { kind, categories, series, options } = element;
  const projection = await client.computeChartPresentation({ kind, categories, series, options });
  assert.ok(Math.abs(projection.series[0].trend.points.at(-1).y - 9) < 1e-10);
  const preview = await client.renderElementPreview(element);
  assert.ok(preview.svg.includes('stroke-dasharray'));
  assert.deepEqual(session.document, before);
  assert.equal(session.canUndo, false);
  assert.equal((await session.exportPresentation()).base64, source.base64);
  await session.transact([{ op: 'replace', path: '/deck/slides/0/elements/0/series/0/trendline/kind', value: 'exponential' }]);
  const copied = await session.copyFormat('slide-1', { id: 'statistics' });
  assert.ok(copied);
  const { session: reopened } = await client.openPresentation('phase3-edited', (await session.exportPresentation()).base64);
  assert.deepEqual(reopened.document.deck.slides[0].elements[0].series[0].values, [3,5,7]);
  await session.undo();
  assert.equal((await session.exportPresentation()).base64, source.base64);
  const wordart = { type: 'text', id: 'wordart', x: 30, y: 30, width: 500, height: 200, text: 'office affinity', font_size: 40, color: '087F73', bold: false, visual: { text_warp: 'arch_down' } };
  assert.ok((await client.renderElementPreview(wordart)).warnings.some(warning => warning.code === 'WORDART_APPROXIMATION'));
  const cancelled = new AbortController(); cancelled.abort();
  await assert.rejects(client.renderElementPreview(wordart, undefined, { signal: cancelled.signal }), /cancel/i);
});

test('G25 G27 SDK rich notes and auxiliary masters survive native reopen and exact Undo', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('native-notes-sdk', 'Synthetic notes');
  const original = await session.exportPresentation();
  const { session: opened } = await client.openPresentation('native-notes-opened', original.base64);
  await opened.updateRichNotes('slide-1', [{ runs: [{ text: 'Rich ', style: { bold: true } }, { text: 'notes', style: { italic: true } }] }], { expectedRevision: 0 });
  assert.equal(opened.document.deck.slides[0].notes, 'Rich notes');
  await assert.rejects(() => opened.updateRichNotes('slide-1', [], { expectedRevision: 0 }), /revision/i);
  const { session: reopened } = await client.openPresentation('native-notes-reopened', (await opened.exportPresentation()).base64);
  assert.equal(reopened.document.deck.slides[0].notes_paragraphs[0].runs[0].style.bold, true);
  await opened.undo();
  assert.equal((await opened.exportPresentation()).base64, original.base64);
  const auxiliary = opened.document.deck.auxiliary_design;
  auxiliary.handout_master = { name: 'Handout', background: '@lt1', theme: (await client.designDefaults()).theme, elements: [] };
  await opened.updateAuxiliaryDesign(auxiliary);
  const { session: master } = await client.openPresentation('native-master-reopened', (await opened.exportPresentation()).base64);
  assert.equal(master.document.deck.auxiliary_design.handout_master.name, 'Handout');
  await opened.undo();
  assert.equal((await opened.exportPresentation()).base64, original.base64);
  await opened.updateRichNotes('slide-1', [{ runs: [{ text: 'retained', field: { id: '00112233-4455-6677-8899-aabbccddeeff', kind: 'datetime13' } }] }]);
  const { session: timed } = await client.openPresentation('native-time', (await opened.exportPresentation()).base64);
  await timed.refreshFields('2026-09-18');
  assert.ok(timed.fieldWarnings.some(warning => warning.includes('reference time required')));
  await timed.refreshFields('2026-09-18', { referenceTime: '16:28:34', locale: 'en-US' });
  assert.equal(timed.document.deck.slides[0].notes, '4:28:34 PM');
  assert.deepEqual(timed.fieldWarnings, []);
});

test('G23 G25 SDK master themes and fields are native undoable and reject stale revisions', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('design-field-sdk', 'Synthetic design field');
  await session.updateDesign(await client.designDefaults());
  const original = await session.exportPresentation();
  const { session: opened } = await client.openPresentation('opened-design-field', original.base64);
  await opened.setDesignField({ master_id: 'master-1', kind: 'slide_number', reference_date: '2026-09-18' }, { expectedRevision: 0 });
  assert.ok(opened.document.deck.slides[0].elements.some(element => element.text === '1' && element.format?.placeholder?.kind === 'slide_number'));
  await assert.rejects(() => opened.setMasterTheme('master-1', null, { expectedRevision: 0 }), /revision/i);
  await opened.undo();
  assert.equal((await opened.exportPresentation()).base64, original.base64);
  const theme = (await client.designDefaults()).theme;
  theme.fonts.minor = 'Courier New';
  await opened.setMasterTheme('master-1', theme);
  const { session: reopened } = await client.openPresentation('reopened-design-field', (await opened.exportPresentation()).base64);
  assert.equal(reopened.document.deck.design.theme.fonts.minor, 'Courier New');
  await opened.undo();
  assert.equal((await opened.exportPresentation()).base64, original.base64);
});

test('G23 G25 SDK typed transforms preserve the session on early and late cancellation', async () => {
  for (const [method, args] of [['setMasterTheme', ['master', null]], ['setDesignField', [{ master_id: 'master', kind: 'date', reference_date: '2026-09-18' }]]]) {
    const before = { id: 'cancel-design', revision: 0, hash: 'before', deck: { slides: [] } };
    let calls = 0; let finish;
    const session = new DocumentSession(async () => { calls += 1; return new Promise(resolve => { finish = resolve; }); }, before);
    const early = new AbortController(); early.abort();
    await assert.rejects(() => session[method](...args, { signal: early.signal }), /cancelled/i);
    assert.equal(calls, 0);
    const late = new AbortController(); const pending = session[method](...args, { signal: late.signal });
    const rejected = assert.rejects(pending, /cancelled/i); late.abort(); finish(before.deck); await rejected;
    assert.equal(calls, 1); assert.deepEqual(session.document, before); assert.equal(session.canUndo, false);
  }
});

test('cell-path stateless SDK helpers forward complete typed input and respect early and late cancellation', async () => {
  for (const [method, op, input] of [
    ['setTableCellText', 'set_table_cell_text', { element: { type: 'table', format: { cells: [] } }, row: 1, column: 2, text: 'Changed' }],
    ['editVector', 'edit_vector', { element: { type: 'polygon', visual: { opacity: 0.5 } }, path: { commands: [{ op: 'close' }] } }],
  ]) {
    let calls = 0; let finish;
    const client = new AislideClient(async request => { calls += 1; assert.deepEqual(request, { ...input, op }); return new Promise(resolveReply => { finish = resolveReply; }); });
    const early = new AbortController(); early.abort();
    await assert.rejects(() => client[method](input, { signal: early.signal }), /cancelled/i);
    assert.equal(calls, 0);
    const late = new AbortController(); const pending = client[method](input, { signal: late.signal });
    const rejected = assert.rejects(pending, /cancelled/i); late.abort(); finish(input.element); await rejected;
    assert.equal(calls, 1);
    const normal = client[method](input); finish(input.element);
    assert.deepEqual(await normal, input.element);
  }
});

test('cell-path layout option forwards preserveFreeform only when explicitly supplied', async () => {
  for (const preserveFreeform of [undefined, false, true]) {
    const calls = [];
    const before = { id: 'layout-test', revision: 0, hash: 'before', deck: { slides: [] } };
    const session = new DocumentSession(async request => { calls.push(request); return request.op === 'assign_layout' ? before.deck : { document: before, receipt: null }; }, before);
    await session.assignLayout('slide', 'layout', { expectedRevision: 0, preserveFreeform });
    assert.deepEqual(calls[0], { op: 'assign_layout', deck: before.deck, slide_id: 'slide', layout_id: 'layout', ...(preserveFreeform === undefined ? {} : { preserve_freeform: preserveFreeform }) });
  }
});

test('static export and verified recovery SDK preserve source and start empty history', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('output-recovery-sdk', 'Output recovery');
  await session.addObject('slide-1', { id: 'text', kind: 'text' });
  const before = session.document;
  for (const format of ['pdf', 'png', 'jpeg']) {
    const result = await session.exportStatic({ format, scale: 0.5 });
    assert.equal(result.files.length, 1);
    assert.ok(Buffer.from(result.files[0].base64, 'base64').length > 10);
    assert.equal(result.pdf_text_outlined, format === 'pdf');
    assert.equal(result.pdf_searchable_text, format === 'pdf');
    assert.equal(result.pdf_selectable_text, format === 'pdf');
    assert.equal(result.pdf_tagged, format === 'pdf');
    assert.equal(result.pdf_semantic_overlay, format === 'pdf');
    assert.equal(result.pdf_editable_text, false);
    assert.equal(result.pdf_ua_certified, false);
    assert.equal(result.office_parity_verified, false);
  }
  assert.deepEqual(session.document, before);
  assert.equal(session.canUndo, true);
  const verified = await client.verifyRecovery(before);
  assert.deepEqual(verified, before);
  assert.ok(Object.isFrozen(verified));
  assert.ok(Object.isFrozen(verified.deck.slides[0].elements[0]));
  const recovered = await client.recoverPresentation(before);
  assert.equal(recovered.canUndo, false);
  assert.equal(recovered.canRedo, false);
  assert.equal(recovered.document.hash, before.hash);
  await assert.rejects(() => client.verifyRecovery({ ...before, hash: '0'.repeat(64) }), /transaction|hash/i);
  for (const method of ['exportStatic', 'verifyRecovery']) {
    let calls = 0; let finish;
    const transport = async () => { calls += 1; return new Promise(resolveReply => { finish = resolveReply; }); };
    const target = method === 'exportStatic' ? new DocumentSession(transport, before) : new AislideClient(transport);
    const input = method === 'exportStatic' ? {} : before;
    const early = new AbortController(); early.abort();
    await assert.rejects(() => target[method](input, { signal: early.signal }), /cancelled/i);
    assert.equal(calls, 0);
    const late = new AbortController(); const pending = target[method](input, { signal: late.signal });
    const rejected = assert.rejects(pending, /cancelled/i); late.abort(); finish(before); await rejected;
    assert.equal(calls, 1);
  }
});

test('expanded authoring SDK routes all operations without hidden calls and rejects early/late aborts', async () => {
  const document = { id: 'synthetic', revision: 0, hash: 'original', deck: { slides: [] } };
  const invocations = [
    ['replaceText', [{ search: { query: 'alpha' }, replacement: 'beta', replace_all: true }], 'replace_text'],
    ['replaceFont', ['Old', 'New'], 'replace_font'],
    ['formatText', ['slide', { id: 'text', start: 0, end: 1, style: { bold: true } }], 'format_text'],
    ['replaceTextContent', ['slide', { id: 'text', text: 'beta' }], 'replace_text_content'],
    ['updateParagraphs', ['slide', { id: 'text', paragraphs: [] }], 'update_paragraphs'],
    ['editTable', ['slide', { id: 'table', operations: [{ op: 'split', row: 0, column: 0 }] }], 'edit_table'],
    ['resizeCanvas', [{ width: 1600, height: 900, mode: 'scale' }], 'resize_canvas'],
    ['applyImageEdit', ['slide', { id: 'image', image: { base64: '', mime_type: 'image/png', width: 1, height: 1 } }], 'apply_image_edit'],
    ['editSelection', ['slide', { op: 'translate', ids: ['text'], dx: 1, dy: 0 }], 'edit_selection'],
  ];
  for (const [method, args, op] of invocations) {
    let calls = 0;
    let finish;
    const session = new DocumentSession(async (request) => {
      calls += 1;
      assert.equal(request.op, op);
      assert.equal(request.expected_revision, 0);
      assert.equal(request.document.hash, 'original');
      return new Promise((resolveReply) => { finish = resolveReply; });
    }, document);
    const early = new AbortController(); early.abort();
    await assert.rejects(() => session[method](...args, { signal: early.signal }), /cancelled/i);
    assert.equal(calls, 0);
    await assert.rejects(() => session[method](...args, { expectedRevision: 7 }), /Revision conflict/);
    assert.equal(calls, 0);
    const late = new AbortController();
    const pending = session[method](...args, { signal: late.signal });
    const rejected = assert.rejects(pending, /cancelled/i);
    late.abort();
    const transaction = { document: { ...document, revision: 1, hash: 'candidate' }, receipt: { inverse: [] } };
    finish(op === 'edit_selection' ? { transaction, clipboard: null, effects: {} } : transaction);
    await rejected;
    assert.equal(calls, 1);
    assert.deepEqual(session.document, document);
    assert.equal(session.canUndo, false);
    assert.equal(session.busy, false);
  }
  for (const [method, args] of [['authoringCapabilities', []], ['searchText', [{}, { query: 'x' }]], ['editImage', [{ base64: '', mime_type: 'image/png', params: {} }]], ['importTemplate', ['new', { kind: 'thmx', base64: '' }]]]) {
    let calls = 0; let finish;
    const client = new AislideClient(async () => { calls += 1; return new Promise((resolveReply) => { finish = resolveReply; }); });
    const early = new AbortController(); early.abort();
    await assert.rejects(() => client[method](...args, { signal: early.signal }), /cancelled/i);
    assert.equal(calls, 0);
    const late = new AbortController(); const pending = client[method](...args, { signal: late.signal });
    const rejected = assert.rejects(pending, /cancelled/i); late.abort(); finish(document);
    await rejected; assert.equal(calls, 1);
  }
});

test('expanded authoring SDK integrates rich, notes, selection, table, images, canvas and templates', async () => {
  const client = new AislideClient(requestCore);
  const capabilities = await client.authoringCapabilities();
  assert.equal(capabilities.limits.request_bytes, 96 * 1024 * 1024);
  assert.equal(capabilities.capacity_profiles.legacy.request_bytes, 4 * 1024 * 1024);
  assert.equal(capabilities.limits.canvas_max, 4096);
  const session = await client.createPresentation('expanded-sdk', 'Synthetic authoring');
  await session.addObject('slide-1', { id: 'text', kind: 'text' });
  await session.replaceTextContent('slide-1', { id: 'text', text: 'Alpha Beta' });
  await session.transact([{ op: 'replace', path: '/deck/slides/0/notes', value: 'Needle' }]);
  assert.equal((await session.searchText({ query: 'Needle', include_notes: true })).length, 1);
  await session.replaceText({ search: { query: 'Needle', include_notes: true }, replacement: 'Replaced', replace_all: true });
  assert.equal(session.document.deck.slides[0].notes, 'Replaced');
  await session.formatText('slide-1', { id: 'text', start: 0, end: 5, style: { bold: true } });
  await session.replaceTextContent('slide-1', { id: 'text', text: 'Alpha Gamma' });
  assert.equal(session.document.deck.slides[0].elements[0].format.paragraphs[0].runs[0].style.bold, true);
  await session.updateParagraphs('slide-1', { id: 'text', paragraphs: [{ runs: [{ text: 'Hello', style: { italic: true } }] }] });
  const beforeCopy = session.document;
  const copied = await session.editSelection('slide-1', { op: 'copy', ids: ['text'], format: 'keep_source_formatting' });
  assert.deepEqual(session.document, beforeCopy);
  await session.editSelection('slide-1', { op: 'paste', id_prefix: 'copy', dx: 0, dy: 0 }, { clipboard: copied.clipboard });
  assert.equal(session.document.deck.slides[0].elements.length, 2);
  await session.undo(); assert.equal(session.document.hash, beforeCopy.hash);
  await session.addObject('slide-1', { id: 'table', kind: 'table', rows: 2, columns: 2 });
  await session.editTable('slide-1', { id: 'table', operations: [{ op: 'insert_row', index: 1, values: ['', ''] }, { op: 'merge', region: { row: 1, column: 0, row_span: 1, col_span: 2 } }] });
  const beforeInvalid = session.document;
  await assert.rejects(() => session.editTable('slide-1', { id: 'table', operations: [{ op: 'remove_column', index: 1 }] }), /merge/i);
  assert.deepEqual(session.document, beforeInvalid);
  await session.undo();
  await session.resizeCanvas({ width: 1600, height: 900, mode: 'keep' });
  assert.equal(session.document.deck.width, 1600);
  for (const kind of ['potx', 'thmx']) {
    const exported = await session.exportTemplate(kind);
    assert.equal(exported.filename, `template.${kind}`);
    const opened = await client.importTemplate(`template-${kind}`, { kind, base64: exported.base64 });
    assert.equal(opened.revision, 0);
  }
  await session.addAsset('slide-1', { id: 'image', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>').toString('base64'), mime_type: 'image/svg+xml', alt: 'Synthetic image', size: 32 });
  const beforeImage = session.document;
  const picture = beforeImage.deck.slides[0].elements.find((element) => element.id === 'image');
  const image = await client.editImage({ base64: picture.base64, mime_type: picture.mime_type, params: { grayscale: true, resize_longest_side: 8, format: { kind: 'jpeg', quality: 80, matte: [255, 255, 255] } } });
  assert.equal(image.mime_type, 'image/jpeg');
  assert.deepEqual(session.document, beforeImage);
  await session.applyImageEdit('slide-1', { id: 'image', image });
  assert.equal(session.document.deck.slides[0].elements.find((element) => element.id === 'image').width, picture.width);
  await session.undo(); assert.equal(session.document.hash, beforeImage.hash);
});

test('guided SDK profiles create editable documents and reject early or late cancellation', async () => {
  const client = new AislideClient(requestCore);
  assert.equal((await client.bestPracticeProfiles()).profiles.length, 4);
  for (const input of guidedExamples()) {
    assert.equal((await client.bestPracticeGuide(input.profile_id)).language, 'en');
    const result = await client.createGuidedPresentation(`guided-${input.profile_id}`, input);
    assert.equal(result.validation.ready, true);
    assert.equal(result.validation.semantic_truth_verified, false);
    assert.equal(result.session.document.deck.slides.length, input.slides.length);
    const snapshot = result.session.document; snapshot.deck.title = 'Changed copy';
    assert.equal(result.session.document.deck.title, input.title);
    assert.equal(result.session.canUndo, false);
    const saved = await result.session.exportPresentation();
    const opened = await client.openPresentation(`reopen-${input.profile_id}`, saved.base64);
    assert.ok(opened.session.document.parts.every((part) => !part.stale));
  }
  let calls = 0;
  let finish;
  const response = new Promise((resolveReply) => { finish = resolveReply; });
  const delayed = new AislideClient(async () => { calls += 1; return response; });
  const early = new AbortController(); early.abort();
  await assert.rejects(() => delayed.createGuidedPresentation('early', guidedExamples()[0], { signal: early.signal }), /cancelled/i);
  assert.equal(calls, 0);
  const late = new AbortController();
  const pending = delayed.createGuidedPresentation('late', guidedExamples()[0], { signal: late.signal });
  const rejected = assert.rejects(pending, /cancelled/i);
  late.abort(); finish({ document: { version: 1, id: 'late', revision: 0, hash: 'unused' }, validation: { ready: true } });
  await rejected;
  assert.equal(calls, 1);
});

test('design preset SDK shares native layouts, preservation, revision guards and undo', async () => {
  const client = new AislideClient(requestCore);
  const presets = await client.designPresets();
  assert.equal(presets.length, 7);
  for (const preset of presets) {
    const session = await client.createPresentation(`preset-${preset.id}`, 'Preset example');
    const original = session.document;
    await session.applyDesignPreset(preset.id, { expectedRevision: 0 });
    assert.equal(session.document.deck.design.masters.length, 2);
    await assert.rejects(() => session.applyDesignPreset(preset.id, { expectedRevision: 0 }), /Revision conflict/);
    await session.undo();
    assert.equal(session.document.hash, original.hash);
    await session.redo();
    await session.assignLayout('slide-1', 'preset-cover');
    await session.editSlides(preset.design.layouts.filter((layout) => layout.id !== 'preset-cover').map((layout) => ({ op: 'insert', id: layout.id, title: layout.name, layout_id: layout.id })));
    const exported = await session.exportPresentation();
    const opened = (await client.openPresentation(`reopen-${preset.id}`, exported.base64)).session;
    assert.equal(opened.document.deck.design.theme.colors.accent1, preset.design.theme.colors.accent1);
    assert.equal(opened.document.deck.slides[0].layout_id, 'preset-cover');
    assert.equal(opened.document.deck.design.layouts.length, 11);
    assert.equal(opened.document.deck.slides.length, 7);
    await opened.applyDesignPreset('minimal');
    assert.equal(opened.document.deck.slides[0].elements.find((entry) => entry.id === 'title').text, 'Presentation title');
  }
});

test('new slides use the applied preset blank layout without adding sample text', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('preset-new-slide');
  await session.applyDesignPreset('public');
  await session.editSlides([{ op: 'insert', id: 'next', after: 'slide-1', title: 'Next slide' }]);
  assert.equal(session.document.deck.slides[1].layout_id, 'preset-blank');
  assert.deepEqual(session.document.deck.slides[1].elements, []);
});

test('design preset visual regions place new parts without moving body text', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('preset-part-layout', 'Visual layout');
  await session.applyDesignPreset('minimal');
  await session.assignLayout('slide-1', 'preset-visual-content');
  const body = session.document.deck.slides[0].elements.find((entry) => entry.id === 'body');
  const region = session.document.deck.design.layouts.find((entry) => entry.id === 'preset-visual-content').elements.find((entry) => entry.id === 'preset-visual-region');
  const catalog = await client.partCatalog();
  const spec = catalog.presets.find((entry) => entry.id === 'flow/balanced').example;
  await session.addPart('slide-1', { id: 'visual-part', spec });
  const part = session.document.deck.slides[0].elements.find((entry) => entry.id === 'visual-part');
  assert.ok(part.x >= region.x && part.y >= region.y);
  assert.ok(part.x + part.width <= region.x + region.width + 0.01);
  assert.ok(part.y + part.height <= region.y + region.height + 0.01);
  assert.equal(part.view_width, part.width);
  assert.equal(part.view_height, part.height);
  const labels = part.children.filter((entry) => entry.type === 'text');
  assert.ok(labels[0].y + labels[0].font_size * 1.35 < labels[1].y);
  assert.deepEqual(session.document.deck.slides[0].elements.find((entry) => entry.id === 'body'), body);
  const exported = await session.exportPresentation();
  const opened = (await client.openPresentation('visual-part-reopened', exported.base64)).session;
  assert.equal(opened.document.parts[0].stale, false);
  await opened.updatePart('slide-1', { id: 'visual-part', spec: { ...spec, title: 'Edited visual' } });
  const updated = opened.document.deck.slides[0].elements.find((entry) => entry.id === 'visual-part');
  assert.equal(updated.x, part.x);
  assert.ok(Math.abs(updated.view_width - updated.width) < 0.001);
  assert.ok(Math.abs(updated.children[0].font_size - labels[0].font_size) < 0.01);
});

test('editor SDK creates blank files, edits native slides and inserts SVG assets with undo', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('editor-sdk', 'New presentation');
  assert.equal(session.document.deck.slides.length, 1);
  assert.deepEqual(session.document.deck.slides[0].elements, []);
  await session.editSlides([{ op: 'insert', id: 'asset-slide', after: 'slide-1', title: 'Icons' }]);
  const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 4h16v16H4z" fill="#0017c1"/></svg>';
  await session.addAsset('asset-slide', { id: 'figma-icon', base64: Buffer.from(svg).toString('base64'), mime_type: 'image/svg+xml', alt: 'Provided icon', size: 96 });
  const saved = await session.exportPresentation();
  const reopened = (await client.openPresentation('editor-reopen', saved.base64)).session;
  assert.equal(reopened.document.deck.slides[1].id, 'asset-slide');
  assert.equal(reopened.document.deck.slides[1].elements[0].mime_type, 'image/png');
  await reopened.editSlides([{ op: 'duplicate', slide_id: 'asset-slide', id: 'copied-slide' }, { op: 'move', slide_id: 'copied-slide', index: 0 }]);
  assert.equal(reopened.document.deck.slides[0].id, 'copied-slide');
  await assert.rejects(() => reopened.editSlides([{ op: 'remove', slide_id: 'asset-slide' }], { expectedRevision: 0 }), /Revision conflict/);
  await reopened.undo();
  assert.equal((await reopened.exportPresentation()).base64, saved.base64);
});

test('graph SDK creates, edits and reopens native graph metadata with undo', async () => {
  const client = new AislideClient(requestCore);
  const catalog = await client.graphCatalog();
  const spec = structuredClone(catalog.examples[0].spec);
  const picture = await client.createGraphIcon({ mime_type: 'image/svg+xml', base64: Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="10" fill="#0017c1"/></svg>').toString('base64'), alt: 'API icon' });
  spec.nodes[1].icon = { base64: picture.base64, mime_type: picture.mime_type, alt: picture.alt };
  const session = await client.createDocument({ id: 'graph-sdk', deck: { version: 1, title: 'Graph', width: 1280, height: 720, slides: [{ id: 'slide', title: 'Graph', background: '@lt1', notes: '', elements: [] }] } });
  await session.addGraph('slide', { id: 'graph', spec });
  const original = session.document;
  await session.applyGraph('slide', { id: 'graph', operations: [{ op: 'move', ids: ['api'], dx: 0, dy: 64 }] }, { expectedRevision: 1 });
  assert.equal(session.document.parts[0].spec.data.graph.nodes[1].y, 284);
  await session.undo(); assert.equal(session.document.hash, original.hash);
  await session.redo();
  const exported = await session.exportPresentation();
  const reopened = await client.openPresentation('graph-sdk-open', exported.base64);
  assert.equal(reopened.session.document.parts[0].stale, false);
  assert.deepEqual(reopened.session.document.parts[0].spec.data.graph.nodes[1].icon, spec.nodes[1].icon);
  assert.equal(reopened.session.document.deck.slides[0].elements[0].children.filter((element) => element.type === 'picture').length, 1);
  await reopened.session.updateGraph('slide', { id: 'graph', spec });
  await assert.rejects(() => reopened.session.applyGraph('slide', { id: 'graph', operations: [{ op: 'remove', ids: ['user'] }] }, { expectedRevision: 0 }), /Revision conflict/);
  await reopened.session.applyGraph('slide', { id: 'graph', operations: [{ op: 'put_node', node: { ...spec.nodes[1], icon: null } }] });
  assert.equal(reopened.session.document.deck.slides[0].elements[0].children.filter((element) => element.type === 'picture').length, 0);
  await reopened.session.undo();
  assert.deepEqual(reopened.session.document.parts[0].spec.data.graph.nodes[1].icon, spec.nodes[1].icon);
});

test('metadata parts share catalog, revisioned updates, undo and single PPTX persistence', async () => {
  const client = new AislideClient(requestCore);
  const catalog = await client.partCatalog();
  assert.equal(catalog.presets.length, 108);
  const spec = catalog.presets.find((preset) => preset.id === 'vertical-bar-graph/balanced').example;
  const deck = { version: 1, title: 'Parts', width: 1280, height: 720, slides: [{ id: 'slide', title: 'Parts', background: 'FFFFFF', notes: 'Synthetic fixture', elements: [] }] };
  const session = await client.createDocument({ id: 'parts-sdk', deck });
  await session.addPart('slide', { id: 'part', spec });
  const inserted = session.document;
  const updated = structuredClone(spec); updated.title = 'Edited part';
  await session.updatePart('slide', { id: 'part', spec: updated }, { expectedRevision: 1 });
  assert.equal(session.document.parts[0].spec.title, 'Edited part');
  await assert.rejects(() => session.updatePart('slide', { id: 'part', spec }, { expectedRevision: 0 }), /Revision conflict/);
  await session.undo(); assert.deepEqual(session.document.deck, inserted.deck);
  await session.redo();
  const exported = await session.exportPresentation();
  const reopened = await client.openPresentation('reopened-parts', exported.base64);
  assert.equal(reopened.session.document.parts[0].stale, false);
  await reopened.session.updatePart('slide', { id: 'part', spec });
  assert.equal(reopened.session.document.parts[0].spec.title, spec.title);
});

test('fractional part geometry remains stable through JSON transport', async () => {
  const client = new AislideClient(requestCore);
  const catalog = await client.partCatalog();
  const preset = catalog.presets.find((preset) => preset.id === 'water-fall/balanced');
  const session = await client.createDocument({ id: 'parts-fractions', deck: { version: 1, title: 'Fractions', width: 1280, height: 720, slides: [{ id: 'slide', title: 'Fractions', background: '@lt1', notes: 'Synthetic fixture', elements: [] }] } });
  await session.addPart('slide', { id: 'part', spec: preset.example });
  await session.updatePart('slide', { id: 'part', spec: { ...preset.example, title: 'Updated fractions' } });
  assert.equal(session.document.parts[0].stale, false);
  const exported = await session.exportPresentation();
  const reopened = await client.openPresentation('fraction-reopen', exported.base64);
  assert.equal(reopened.session.document.parts[0].stale, false);
  assert.equal((await reopened.session.exportPresentation()).base64, exported.base64);
});

test('one PPTX opens, edits and saves without a checkpoint file', async () => {
  const client = new AislideClient(requestCore);
  const compiled = await client.request({ op: 'compile', report: await client.request({ op: 'sample' }) });
  const session = await client.createDocument({ id: 'single-pptx-client', deck: compiled.deck });
  const exported = await session.exportPresentation();
  assert.equal(exported.checkpoint, undefined);
  const opened = await client.openPresentation('single-pptx-reopened', exported.base64);
  assert.equal(opened.session.document.deck.slides.length, 12);
  assert.equal(opened.session.document.origin.native, true);
  const title = opened.session.document.deck.slides[0].elements.findIndex((element) => element.id === 'title');
  const original = opened.session.document;
  await opened.session.transact([{ op: 'replace', path: `/deck/slides/0/elements/${title}/text`, value: 'One editable PPTX\nNo sidecar' }]);
  const updated = await opened.session.exportPresentation();
  const verified = await client.openPresentation('edited-single-pptx', updated.base64);
  assert.equal(verified.session.document.deck.slides[0].elements[title].text, 'One editable PPTX\nNo sidecar');
  await opened.session.undo();
  assert.equal(opened.session.document.hash, original.hash);
  await assert.rejects(() => client.openPresentation('protected', Buffer.from([0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]).toString('base64')), /encrypted.*legacy/);
});

test('authoring APIs share revision guards, theme inheritance and undo', async () => {
  const client = new AislideClient(requestCore);
  const design = await client.designDefaults();
  const catalog = await client.objectCatalog();
  assert.equal(catalog.shapes.length, 40);
  const deck = { version: 1, title: 'API authoring', width: 1280, height: 720, slides: [{ id: 'slide-1', title: 'API', background: 'FFFFFF', notes: 'Synthetic API fixture', elements: [] }] };
  const session = await client.createDocument({ id: 'authoring-api', deck });
  await session.updateDesign(design, { expectedRevision: 0 });
  await session.assignLayout('slide-1', 'title-content', { expectedRevision: 1 });
  await session.addObject('slide-1', { id: 'native-ellipse', kind: 'shape', preset: 'ellipse' }, { expectedRevision: 2 });
  assert.equal(session.document.deck.slides[0].elements.at(-1).preset, 'ellipse');
  const theme = structuredClone(design.theme); theme.colors.accent1 = 'B53055';
  await session.applyTheme(theme, { expectedRevision: 3 });
  assert.equal(session.document.deck.design.theme.colors.accent1, 'B53055');
  await assert.rejects(() => session.assignLayout('slide-1', 'blank', { expectedRevision: 0 }), /Revision conflict/);
  assert.equal(session.revision, 4);
  await session.undo();
  assert.equal(session.document.deck.design.theme.colors.accent1, design.theme.colors.accent1);
  const exported = await session.exportProject();
  assert.equal((await client.openProject(exported)).document.hash, session.document.hash);
});

test('portable client ingests, edits, undoes and restores a source-bound project without a GUI', async () => {
  const client = new AislideClient(requestCore);
  const source = await client.ingest({ format: 'csv', name: 'fixture.csv', base64: Buffer.from('Quarter,Value\nQ1,-2\nQ2,0\nQ3,12\n').toString('base64') });
  const result = await client.dataReport(source, { title: 'Provided values', period: 'Fixture', table_index: 0, category_column: 0, value_columns: [1], row_start: 0, row_count: 3, chart_kind: 'column' });
  const session = await client.createDocument({ id: 'client-fixture', deck: result.compiled.deck, report: result.report, sources: [source], bindings: result.bindings });
  const snapshot = session.document;
  snapshot.deck.title = 'External mutation';
  assert.equal(session.document.deck.title, 'Provided values');
  const index = session.document.deck.slides[3].elements.findIndex((element) => element.type === 'chart');
  await session.transact([{ op: 'replace', path: `/deck/slides/3/elements/${index}/x`, value: 80 }]);
  assert.equal(session.revision, 1);
  await assert.rejects(() => session.transact([{ op: 'replace', path: '/deck/title', value: 'Stale' }], { expectedRevision: 0 }));
  assert.equal(session.revision, 1);
  await session.undo();
  assert.equal(session.document.deck.slides[3].elements[index].x, 64);
  await session.redo();
  assert.equal(session.document.deck.slides[3].elements[index].x, 80);
  const bundle = await session.exportProject();
  assert.equal(Buffer.from(bundle.base64, 'base64').subarray(0, 2).toString(), 'PK');
  const restored = await client.openProject(bundle);
  assert.deepEqual(restored.document, session.document);
  assert.equal(restored.document.sources[0].sha256, source.sha256);
  const controller = new AbortController(); controller.abort();
  const before = session.document;
  await assert.rejects(() => session.transact([{ op: 'replace', path: '/deck/title', value: 'Cancelled' }], { signal: controller.signal }));
  assert.deepEqual(session.document, before);
});

test('a late transport success after cancellation never commits session state', async () => {
  const initial = { version: 1, id: 'cancellation-fixture', revision: 0, hash: 'before', deck: { title: 'Before' }, sources: [], bindings: [] };
  let finish;
  const pendingReply = new Promise((resolveReply) => { finish = resolveReply; });
  const session = new DocumentSession(async () => pendingReply, initial);
  const controller = new AbortController();
  const pending = session.transact([{ op: 'replace', path: '/deck/title', value: 'After' }], { signal: controller.signal });
  const rejected = assert.rejects(pending, /cancelled/i);
  controller.abort();
  finish({ document: { ...initial, revision: 1, hash: 'after', deck: { title: 'After' } }, receipt: { document_id: initial.id, after_hash: 'after', inverse: [] } });
  await rejected;
  assert.deepEqual(session.document, initial);
  assert.equal(session.canUndo, false);
  assert.equal(session.busy, false);
});

test('review SDK preserves immutable helpers, local comments, field caches, notes and clean-copy isolation', async () => {
  const client = new AislideClient(requestCore);
  const session = await client.createPresentation('review-sdk', 'Synthetic review SDK');
  await session.addObject('slide-1', { id: 'text', kind: 'text' });
  await session.replaceTextContent('slide-1', { id: 'text', text: 'Alpha Beta' });
  const beforeHelper = session.document;
  const formatted = await client.formatTextElement({ element: beforeHelper.deck.slides[0].elements[0], start: 0, end: 5, style: { bold: true } });
  const replaced = await client.replaceElementText({ element: formatted, text: 'Alpha Gamma' });
  assert.equal(replaced.format.paragraphs[0].runs[0].style.bold, true);
  assert.equal(replaced.document, undefined);
  assert.deepEqual(session.document, beforeHelper);
  const revision = session.revision;
  await session.updateNotes('slide-1', 'PRIVATE_SYNTHETIC_NOTES', { expectedRevision: revision });
  await assert.rejects(() => session.updateNotes('slide-1', 'stale draft', { expectedRevision: revision }), /Revision conflict/);
  assert.equal(session.document.deck.slides[0].notes, 'PRIVATE_SYNTHETIC_NOTES');
  const comment = { id: 'first', author: 'PRIVATE_SYNTHETIC_AUTHOR', initials: 'SA', timestamp: '2026-09-17T10:00:00Z', text: 'PRIVATE_SYNTHETIC_COMMENT' };
  await session.addComment('slide-1', comment);
  await session.replyComment('slide-1', 'first', { ...comment, id: 'reply' });
  await session.resolveComment('slide-1', 'first', true);
  await session.setAccessibility('slide-1', 'text', { title: 'Synthetic text' });
  const order = await session.setReadingOrder('slide-1', ['text']);
  assert.match(order.warnings.join(' '), /z-order/);
  assert.equal((await session.checkAccessibility()).wcag_certified, false);
  const inspection = await session.inspectDocument();
  assert.equal(inspection.complete_personal_data_detection, false);
  assert.equal(JSON.stringify(inspection).includes('PRIVATE_SYNTHETIC'), false);
  const field = { id: '{00112233-4455-6677-8899-aabbccddeeff}', kind: 'slidenum' };
  const date = { id: '00112233-4455-6677-8899-aabbccddee00', kind: 'datetime1' };
  await session.updateParagraphs('slide-1', { id: 'text', paragraphs: [{ runs: [{ text: 'Page ' }, { text: '9', field }, { text: ' ' }, { text: '1/1/2000', field: date }] }] });
  const beforeRefresh = session.document;
  await session.refreshFields('2024-02-29');
  assert.equal(session.document.deck.slides[0].elements[0].text, 'Page 1 2/29/2024');
  assert.deepEqual(session.document.deck.slides[0].elements[0].format.paragraphs[0].runs[1].field, field);
  await session.undo(); assert.equal(session.document.hash, beforeRefresh.hash);
  await session.redo();
  const beforeInvalid = session.document;
  await assert.rejects(() => session.refreshFields('2023-02-29'), /reference date/);
  await assert.rejects(() => session.updateParagraphs('slide-1', { id: 'text', paragraphs: [{ runs: [{ text: '1', field: { ...field, id: 'not-a-uuid' } }] }] }), /UUID/);
  assert.deepEqual(session.document, beforeInvalid);
  const exported = await session.exportPresentation();
  const { session: reopened } = await client.openPresentation('review-sdk-opened', exported.base64);
  assert.equal(reopened.document.deck.slides[0].review.comments[1].parent_id, 'first');
  assert.equal(reopened.document.deck.slides[0].review.comments[0].resolved, true);
  const beforeReopen = reopened.document;
  await reopened.resolveComment('slide-1', 'first', false);
  assert.equal(reopened.document.deck.slides[0].review.comments[0].resolved, false);
  await reopened.undo(); assert.equal(reopened.document.hash, beforeReopen.hash);
  const beforeClean = reopened.document;
  const options = { new_document_id: 'clean-review-sdk', categories: ['notes'], confirmed: true };
  await assert.rejects(() => reopened.exportCleanCopy({ ...options, confirmed: false }), /confirmation/);
  await assert.rejects(() => reopened.exportCleanCopy({ ...options, new_document_id: reopened.document.id }), /new document ID/);
  const clean = await reopened.exportCleanCopy(options);
  assert.equal(clean.document.id, options.new_document_id);
  assert.equal(clean.document.revision, 0);
  assert.equal(clean.document.deck.slides[0].notes, '');
  assert.equal(clean.document.deck.slides[0].review.comments.length, 2);
  assert.deepEqual(reopened.document, beforeClean);
  await reopened.removeComment('slide-1', 'first');
  assert.equal(reopened.document.deck.slides[0].review.comments, undefined);
  await reopened.undo(); assert.equal(reopened.document.hash, beforeReopen.hash);
});

test('review SDK late cancellation and notes revision guards leave state and history untouched', async () => {
  const initial = { id: 'cancel-review', revision: 2, hash: 'before', deck: { slides: [{ id: 'slide', notes: 'before' }] } };
  for (const [method, args] of [
    ['addComment', ['slide', {}]], ['replyComment', ['slide', 'parent', {}]],
    ['resolveComment', ['slide', 'parent', false]], ['removeComment', ['slide', 'parent']],
    ['modernComment', ['slide', { type: 'remove', comment_id: 'parent' }]], ['setTableHeaders', ['slide', 'table', 'both']],
    ['setAccessibility', ['slide', 'element', null]], ['setReadingOrder', ['slide', ['element']]],
    ['refreshFields', ['2026-09-17']], ['updateNotes', ['slide', 'after']],
  ]) {
    let finish; let calls = 0;
    const session = new DocumentSession(async (request) => {
      calls += 1;
      assert.equal(request.op === 'transaction' ? request.transaction.expected_revision : request.expected_revision, 2);
      return new Promise((resolveReply) => { finish = resolveReply; });
    }, initial);
    await assert.rejects(() => session[method](...args, { expectedRevision: 1 }), /Revision conflict/);
    assert.equal(calls, 0);
    const controller = new AbortController(); controller.abort();
    await assert.rejects(() => session[method](...args, { signal: controller.signal }), /cancelled/i);
    assert.equal(calls, 0);
    const late = new AbortController();
    const pending = session[method](...args, { expectedRevision: 2, signal: late.signal });
    const rejected = assert.rejects(pending, /cancelled/i);
    await assert.rejects(() => session.updateNotes('slide', 'overlap'), /busy/);
    late.abort(); finish({ document: { ...initial, revision: 3, hash: 'candidate' }, receipt: { inverse: [] }, warnings: ['z-order'] });
    await rejected;
    assert.deepEqual(session.document, initial);
    assert.equal(session.canUndo, false); assert.equal(session.busy, false);
  }
});

test('review API source files retain exactly one UTF-8 BOM', async () => {
  const files = ['crates/aislide-core/src/protocol.rs', 'crates/aislide-core/src/authoring_ops.rs', 'crates/aislide-core/src/comments.rs', 'crates/aislide-core/src/modern_comments.rs', 'crates/aislide-core/src/review.rs', 'crates/aislide-core/tests/review_api.rs', 'crates/aislide-core/tests/review_features.rs', 'apps/studio/src/ReviewPanel.tsx', 'tests/e2e/editing-workflows.spec.ts', 'packages/client/index.mjs', 'packages/client/index.d.mts', 'packages/client/types.ts', 'tools/mcp.mjs', 'tools/client.test.mjs', 'tools/mcp-poc.test.mjs'];
  for (const path of files) {
    const bytes = await readFile(new URL(`../${path}`, import.meta.url));
    assert.equal(bytes.subarray(0, 3).toString('hex'), 'efbbbf', `${path}: missing BOM`);
    assert.notEqual(bytes.subarray(3, 6).toString('hex'), 'efbbbf', `${path}: duplicate BOM`);
    assert.doesNotThrow(() => new TextDecoder('utf-8', { fatal: true }).decode(bytes), `${path}: invalid UTF-8`);
  }
});
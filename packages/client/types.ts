export type Bounds = { id: string; x: number; y: number; width: number; height: number }
export type ChartKind = 'column' | 'bar' | 'line' | 'pie' | 'doughnut' | 'area' | 'scatter' | 'stacked_column' | 'stacked_bar' | 'percent_stacked_column'
export type ChartData = { kind: ChartKind; categories: string[]; series: { name: string; values: number[]; color: string }[] }
export type Placeholder = { kind: 'title' | 'body' | 'subtitle' | 'footer' | 'date' | 'slide_number'; index: number }
export type TextFormat = { italic?: boolean; underline?: boolean; alignment?: 'left' | 'center' | 'right' | 'justify'; vertical?: 'top' | 'middle' | 'bottom'; bullet?: 'none' | 'bullet' | 'numbered'; font_family?: string | null; hyperlink?: string | null; placeholder?: Placeholder | null; inherit_layout?: boolean }
export type Theme = { name: string; colors: Record<string, string>; fonts: { major: string; minor: string; east_asian: string; complex_script: string } }
export type Master = { id: string; name: string; background: string; elements: Element[] }
export type SlideLayout = { id: string; name: string; master_id: string; background: string | null; elements: Element[] }
export type Design = { theme: Theme; masters: Master[]; layouts: SlideLayout[] }
export type DesignRegion = { layout_id: string; name: string; x: number; y: number; width: number; height: number }
export type DesignPreset = { id: string; name: string; design: Design; rules: { margin: number; gutter: number; heading_size: number; body_size: number; regions: DesignRegion[] } }
export type ObjectKind = 'text' | 'shape' | 'table' | 'chart' | 'line' | 'arrow'
export type ObjectCatalog = { shapes: { id: string; name: string }[]; charts: ChartKind[]; table: { max_rows: number; max_columns: number } }
export type Element = Bounds & (
  | { type: 'text'; text: string; font_size: number; color: string; bold: boolean; format?: TextFormat }
  | { type: 'rect'; fill: string }
  | { type: 'polygon'; points: [number, number][]; fill: string; stroke: string; stroke_width: number }
  | { type: 'shape'; preset: string; fill: string; stroke: string; stroke_width: number; rotation: number; text: string; font_size: number; color: string; bold: boolean; format?: TextFormat }
  | { type: 'table'; rows: string[][]; font_size: number }
  | ({ type: 'chart' } & ChartData)
  | { type: 'picture'; base64: string; mime_type: 'image/png' | 'image/jpeg'; alt: string; crop: { left: number; top: number; right: number; bottom: number } }
  | { type: 'connector'; color: string; stroke_width: number; arrow: boolean; flip_v?: boolean; start?: { element_id: string; site: number } | null; end?: { element_id: string; site: number } | null; routing?: { points: [number, number][]; start_arrow: boolean; dashed: boolean } | null }
  | { type: 'group'; view_width: number; view_height: number; children: Element[] }
)
export type Slide = { id: string; title: string; background: string; elements: Element[]; notes: string; layout_id?: string | null; inherit_background?: boolean; hide_master_graphics?: boolean; native_source_id?: string | null }
export type SlideOperation =
  | { op: 'insert'; id: string; after?: string | null; title: string; layout_id?: string | null }
  | { op: 'duplicate'; slide_id: string; id: string }
  | { op: 'remove'; slide_id: string }
  | { op: 'move'; slide_id: string; index: number }
  | { op: 'rename'; slide_id: string; title: string }
export type AssetInput = { id: string; base64: string; mime_type: 'image/svg+xml' | 'image/png' | 'image/jpeg'; alt: string; size: number }
export type ElementOperation = { op: 'duplicate'; id: string; new_id: string } | { op: 'remove'; id: string } | { op: 'order'; id: string; index: number }
export type Deck = { version: 1; title: string; width: 1280; height: 720; slides: Slide[]; design?: Design | null }
export type Section = { title: string; layout: 'cover' | 'metrics' | 'table' | 'columns' | 'statement' | 'chart' | 'process'; body: string[]; metrics: { label: string; value: string }[]; rows: string[][]; chart?: ChartData }
export type Report = { title: string; subtitle: string; period: string; source: string; sections: Section[] }
export type Issue = { code: string; severity: string; message: string }
export type Compiled = { deck: Deck; issues: Issue[] }
export type Exported = { base64: string; filename: string }
export type ProviderStatus = { configured: boolean; endpoint: string | null; model: string | null; remote: boolean; timeout_seconds: number; message: string }
export type GeneratedReport = { report: Report; compiled: Compiled; provenance: { mode: 'model'; model: string; remote: boolean; source_sha256: string; elapsed_ms: number; verified: false; attempts: number } }
export type Inspection = { slides: { part: string; texts: { shape_id: string; run_index: number; text: string }[] }[]; warnings: string[] }
export type SourceFormat = 'csv' | 'json' | 'xlsx' | 'markdown' | 'text' | 'pdf' | 'png' | 'jpeg'
export type Scalar = string | number | boolean | null
export type Attribution = { citation: string; url: string; license: string; derived_from_sha256?: string; transformation?: string }
export type SourceInput = { name: string; format: SourceFormat; base64: string; ocr?: boolean; ocr_language?: string; attribution?: Attribution }
export type SourceTable = { name: string; columns: string[]; rows: Scalar[][]; locators: string[][] }
export type SourceDocument = { version: 1; id: string; name: string; format: SourceFormat; sha256: string; byte_length: number; content_sha256: string; tables: SourceTable[]; text: string; warnings: string[]; pages: { page: number; locator: string; text: string; method: string; regions: { text: string; x: number; y: number; width: number; height: number }[] }[]; raster?: { width: number; height: number; mime_type: string; sha256: string; byte_length: number }; attribution?: Attribution }
export type SourceBinding = { slide_id: string; element_id: string; field: string; source_id: string; source_sha256: string; locator: string; value: Scalar; raw_value: Scalar; transform: 'strict_numeric' | 'display_scalar'; stale: boolean }
export type DataMapping = { title: string; period: string; table_index: number; category_column: number; value_columns: number[]; row_start: number; row_count: number; chart_kind: ChartData['kind'] }
export type DataReport = { report: Report; compiled: Compiled; bindings: SourceBinding[] }
export type AislideDocument = { version: 1; id: string; revision: number; hash: string; deck: Deck; sources: SourceDocument[]; bindings: SourceBinding[]; parts?: PartInstance[]; report?: Report | null; origin?: { base64: string; sha256: string; native?: boolean } }
export type ImportedObject = { slide_id: string; element_id: string; part: string; editable_fields: string[] }
export type LayoutReport = { engine: string; office_parity_verified: false; measurements: { slide_id: string; element_id: string; measured_height: number; height: number; overflow: boolean; missing_glyphs: number; fonts: string[] }[]; fonts: string[]; issues: Issue[] }
export type Checkpoint = { format: 'aislide.project'; version: 1; pptx_sha256: string; document: AislideDocument }
export type ProjectExport = Exported & { checkpoint: Checkpoint; checkpoint_filename: string }
export type PresentationExport = Exported & { layout: LayoutReport | null }
export type PatchOperation = { op: 'add' | 'remove' | 'replace' | 'move' | 'copy' | 'test'; path: string; value?: unknown; from?: string }

export type PartItem = { label: string; detail?: string; value?: number | null }
export type PartData =
  | { kind: 'chart'; categories: string[]; series: { name: string; values: number[] }[]; x_axis?: string; y_axis?: string }
  | { kind: 'items'; items: PartItem[]; center?: string }
  | { kind: 'tree'; nodes: { id: string; label: string; parent?: string | null }[] }
  | { kind: 'network'; nodes: PartItem[]; edges: { from: number; to: number; label?: string }[] }
  | { kind: 'matrix'; rows: string[]; columns: string[]; cells: string[][] }
  | { kind: 'groups'; groups: { label: string; items: string[] }[] }
  | { kind: 'timeline'; periods: string[]; tasks: { label: string; start: number; end: number; progress?: number }[] }
  | { kind: 'waterfall'; steps: { label: string; value: number; total?: boolean }[]; unit?: string }
  | { kind: 'map'; points: { label: string; longitude: number; latitude: number; value?: number | null }[] }
  | { kind: 'diagram'; graph: GraphSpec }
export type PartSpec = { version: 1; preset: string; title: string; subtitle?: string; data: PartData }
export type PartPreset = { id: string; category: string; category_name: string; name: string; family: 'Charts' | 'Diagrams'; example: PartSpec }
export type PartCatalog = { version: 1; presets: PartPreset[]; schema: unknown; style: string; default_bounds: Omit<Bounds, 'id'> }
export type PartInstance = { slide_id: string; element_id: string; spec: PartSpec; render_sha256: string; native_sha256?: string | null; stale: boolean }

export type AuthoringProfile = 'consulting-decision' | 'technical-explainer' | 'event-talk' | 'status-report'
export type GuidedEvidence = { id: string; kind: 'source' | 'assumption' | 'unknown'; reference: string; statement: string }
export type ClauseSupport = { clause: string; body_paths: string[]; evidence_ids: string[] }
export type NumberEvidence = { path: string; value: Scalar; evidence_id: string }
export type DecisionIssue = { id: string; question: string; requested_decision: string; criterion: string; owner: string; due: string; evidence_ids: string[]; analysis_slide_ids: string[] }
export type GuidedSlide = { id: string; section: string; headline: string; sentence_form: 'causal' | 'conditional' | 'contrast' | 'causal-focus' | 'evaluation' | 'proposal' | 'explanation' | 'comparison' | 'outcome'; pattern_id: string; question: string; parent_message: string; transition: string; parallel_basis: string; part?: PartSpec | null; support: ClauseSupport[]; numbers?: NumberEvidence[] }
export type GuidedInput = { version: 1; profile_id: AuthoringProfile; title: string; audience: string; purpose: string; governing_message: string; language: 'en' | 'ja'; brand_color?: string | null; evidence: GuidedEvidence[]; issues?: DecisionIssue[]; slides: GuidedSlide[] }
export type GuidedReview = { ready: boolean; issues: string[]; review_required: string[]; semantic_truth_verified: false; office_visual_parity: false }
export type BestPracticeGuide = { version: 1; profile_id: AuthoringProfile; title: string; language: 'en'; markdown: string; patterns: { id: string; name: string; purpose: string; capability: 'native-template' | 'composition-required' | 'guidance-only'; rule: string }[]; input_schema: unknown; automatic_patterns: string[]; semantic_truth_verified: false; limits: { slides: number; evidence: number; issues: number }; creation: string }
export type BestPracticeProfiles = { version: 1; profiles: { id: AuthoringProfile; title: string; language: 'en'; default_primary: string }[] }

export type GraphNodeKind = 'rectangle' | 'rounded_rectangle' | 'ellipse' | 'diamond' | 'cylinder' | 'cloud'
export type GraphPort = 'auto' | 'top' | 'left' | 'bottom' | 'right'
export type GraphRoute = 'straight' | 'elbow'
export type GraphIcon = { base64: string; mime_type: 'image/png' | 'image/jpeg'; alt?: string }
export type GraphNode = { id: string; label: string; kind?: GraphNodeKind; x: number; y: number; width?: number; height?: number; fill?: string; stroke?: string; color?: string; font_size?: number; group?: string | null; icon?: GraphIcon | null }
export type GraphEdge = { id: string; source: string; target: string; source_port?: GraphPort; target_port?: GraphPort; label?: string; route?: GraphRoute; color?: string; arrow?: boolean; start_arrow?: boolean; dashed?: boolean }
export type GraphGroup = { id: string; label: string; x: number; y: number; width: number; height: number; fill?: string; stroke?: string }
export type GraphSpec = { version: 1; title: string; subtitle?: string; nodes: GraphNode[]; edges?: GraphEdge[]; groups?: GraphGroup[] }
export type GraphAlignment = 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom'
export type GraphOperation =
  | { op: 'put_node'; node: GraphNode }
  | { op: 'put_edge'; edge: GraphEdge }
  | { op: 'put_group'; group: GraphGroup }
  | { op: 'move'; ids: string[]; dx: number; dy: number }
  | { op: 'remove'; ids: string[] }
  | { op: 'align'; ids: string[]; alignment: GraphAlignment }
  | { op: 'layout'; columns: number }
export type GraphCatalog = { version: 1; shapes: GraphNodeKind[]; ports: GraphPort[]; routes: GraphRoute[]; limits: { nodes: number; edges: number; groups: number; rendered_elements: number }; canvas: { width: number; height: number; content_top: number }; schema: unknown; operation_schema: unknown; examples: { id: string; name: string; spec: GraphSpec }[] }
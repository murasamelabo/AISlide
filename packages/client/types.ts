export type Bounds = { id: string; x: number; y: number; width: number; height: number }
export type ChartData = { kind: 'column' | 'bar' | 'line'; categories: string[]; series: { name: string; values: number[]; color: string }[] }
export type Element = Bounds & (
  | { type: 'text'; text: string; font_size: number; color: string; bold: boolean }
  | { type: 'rect'; fill: string }
  | { type: 'table'; rows: string[][]; font_size: number }
  | ({ type: 'chart' } & ChartData)
  | { type: 'picture'; base64: string; mime_type: 'image/png' | 'image/jpeg'; alt: string; crop: { left: number; top: number; right: number; bottom: number } }
  | { type: 'connector'; color: string; stroke_width: number; arrow: boolean; flip_v?: boolean; start?: { element_id: string; site: number } | null; end?: { element_id: string; site: number } | null }
  | { type: 'group'; view_width: number; view_height: number; children: Element[] }
)
export type Slide = { id: string; title: string; background: string; elements: Element[]; notes: string }
export type Deck = { version: 1; title: string; width: 1280; height: 720; slides: Slide[] }
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
export type AislideDocument = { version: 1; id: string; revision: number; hash: string; deck: Deck; sources: SourceDocument[]; bindings: SourceBinding[]; report?: Report | null; origin?: { base64: string; sha256: string } }
export type ImportedObject = { slide_id: string; element_id: string; part: string; editable_fields: string[] }
export type LayoutReport = { engine: string; office_parity_verified: false; measurements: { slide_id: string; element_id: string; measured_height: number; height: number; overflow: boolean; missing_glyphs: number; fonts: string[] }[]; fonts: string[]; issues: Issue[] }
export type Checkpoint = { format: 'aislide.project'; version: 1; pptx_sha256: string; document: AislideDocument }
export type ProjectExport = Exported & { checkpoint: Checkpoint; checkpoint_filename: string }
export type PatchOperation = { op: 'add' | 'remove' | 'replace' | 'move' | 'copy' | 'test'; path: string; value?: unknown; from?: string }
export type Bounds = { id: string; x: number; y: number; width: number; height: number }
export type Element = Bounds & (
  | { type: 'text'; text: string; font_size: number; color: string; bold: boolean }
  | { type: 'rect'; fill: string }
  | { type: 'table'; rows: string[][]; font_size: number }
)
export type Slide = { id: string; title: string; background: string; elements: Element[]; notes: string }
export type Deck = { version: 1; title: string; width: 1280; height: 720; slides: Slide[] }
export type Section = { title: string; layout: 'cover' | 'metrics' | 'table' | 'columns' | 'statement'; body: string[]; metrics: { label: string; value: string }[]; rows: string[][] }
export type Report = { title: string; subtitle: string; period: string; source: string; sections: Section[] }
export type Compiled = { deck: Deck; issues: { code: string; severity: string; message: string }[] }
export type Exported = { base64: string; filename: string }
export type ProviderStatus = { configured: boolean; endpoint: string | null; model: string | null; remote: boolean; timeout_seconds: number; message: string }
export type GeneratedReport = { report: Report; compiled: Compiled; provenance: { mode: 'model'; model: string; remote: boolean; source_sha256: string; elapsed_ms: number; verified: false } }
export type Inspection = { slides: { part: string; texts: { shape_id: string; run_index: number; text: string }[] }[]; warnings: string[] }
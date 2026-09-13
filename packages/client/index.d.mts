import type { AislideDocument, Checkpoint, DataMapping, DataReport, Deck, ImportedObject, PatchOperation, ProjectExport, Report, SourceBinding, SourceDocument, SourceInput } from './types'
export * from './types'
export type RequestOptions = { signal?: AbortSignal }
export type TransactionOptions = RequestOptions & { expectedRevision?: number }
export type ReplaceOptions = TransactionOptions & { sources?: SourceDocument[]; bindings?: SourceBinding[]; report?: Report | null }
export type CoreTransport = <T>(request: unknown, options?: RequestOptions) => Promise<T>
export class AislideClient {
  constructor(transport: CoreTransport)
  request<T>(request: unknown, options?: RequestOptions): Promise<T>
  ingest(input: SourceInput, options?: RequestOptions): Promise<SourceDocument>
  dataReport(source: SourceDocument, mapping: DataMapping, options?: RequestOptions): Promise<DataReport>
  importPresentation(id: string, base64: string, options?: RequestOptions): Promise<{ session: DocumentSession; warnings: string[]; objects: ImportedObject[] }>
  createDocument(input: { id: string; deck: Deck; sources?: SourceDocument[]; bindings?: SourceBinding[]; report?: Report | null }, options?: RequestOptions): Promise<DocumentSession>
  openProject(input: { base64: string; checkpoint: Checkpoint }, options?: RequestOptions): Promise<DocumentSession>
}
export class DocumentSession {
  constructor(transport: CoreTransport, document: AislideDocument)
  readonly document: AislideDocument
  readonly revision: number
  readonly canUndo: boolean
  readonly canRedo: boolean
  readonly busy: boolean
  transact(operations: PatchOperation[], options?: TransactionOptions): Promise<AislideDocument>
  replaceDeck(deck: Deck, options?: ReplaceOptions): Promise<AislideDocument>
  undo(options?: RequestOptions): Promise<AislideDocument>
  redo(options?: RequestOptions): Promise<AislideDocument>
  exportProject(options?: RequestOptions): Promise<ProjectExport>
}
import type { AislideDocument, Checkpoint, DataMapping, DataReport, Deck, Design, Element, ImportedObject, ObjectCatalog, ObjectKind, PatchOperation, PresentationExport, ProjectExport, Report, SourceBinding, SourceDocument, SourceInput, Theme } from './types'
import type { PartCatalog, PartSpec, GraphCatalog, GraphIcon, GraphSpec, GraphOperation, SlideOperation, ElementOperation, AssetInput, DesignPreset } from './types'
import type { AuthoringProfile, BestPracticeGuide, BestPracticeProfiles, GuidedInput, GuidedReview } from './types'
export * from './types'
export type RequestOptions = { signal?: AbortSignal }
export type TransactionOptions = RequestOptions & { expectedRevision?: number }
export type ReplaceOptions = TransactionOptions & { sources?: SourceDocument[]; bindings?: SourceBinding[]; report?: Report | null }
export type CoreTransport = <T>(request: unknown, options?: RequestOptions) => Promise<T>
export type ObjectInput = { id: string; kind: ObjectKind; preset?: string; rows?: number; columns?: number }
export class AislideClient {
  constructor(transport: CoreTransport)
  request<T>(request: unknown, options?: RequestOptions): Promise<T>
  ingest(input: SourceInput, options?: RequestOptions): Promise<SourceDocument>
  dataReport(source: SourceDocument, mapping: DataMapping, options?: RequestOptions): Promise<DataReport>
  designDefaults(options?: RequestOptions): Promise<Design>
  designPresets(options?: RequestOptions): Promise<DesignPreset[]>
  objectCatalog(options?: RequestOptions): Promise<ObjectCatalog>
  partCatalog(options?: RequestOptions): Promise<PartCatalog>
  bestPracticeProfiles(options?: RequestOptions): Promise<BestPracticeProfiles>
  bestPracticeGuide(profileId: AuthoringProfile, options?: RequestOptions): Promise<BestPracticeGuide>
  validateGuidedPresentation(input: GuidedInput, options?: RequestOptions): Promise<GuidedReview>
  createGuidedPresentation(id: string, input: GuidedInput, options?: RequestOptions): Promise<{ session: DocumentSession; validation: GuidedReview; profile_id: AuthoringProfile; model_inference: false }>
  graphCatalog(options?: RequestOptions): Promise<GraphCatalog>
  createGraphIcon(input: Pick<AssetInput, 'base64' | 'mime_type'> & { alt?: string }, options?: RequestOptions): Promise<GraphIcon>
  createGraph(input: { id: string; spec: GraphSpec; theme?: Theme }, options?: RequestOptions): Promise<Element>
  transformGraph(spec: GraphSpec, operations: GraphOperation[], options?: RequestOptions): Promise<GraphSpec>
  createPart(input: { id: string; spec: PartSpec; theme?: Theme }, options?: RequestOptions): Promise<Element>
  createObject(input: ObjectInput, options?: RequestOptions): Promise<Element>
  createAsset(input: AssetInput, options?: RequestOptions): Promise<Element>
  createPresentation(id: string, title?: string, options?: RequestOptions): Promise<DocumentSession>
  openPresentation(id: string, base64: string, options?: RequestOptions): Promise<{ session: DocumentSession; warnings: string[]; objects: ImportedObject[]; format: 'open_xml_pptx' }>
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
  updateDesign(design: Design, options?: TransactionOptions): Promise<AislideDocument>
  applyDesignPreset(presetId: string, options?: TransactionOptions): Promise<AislideDocument>
  applyTheme(theme: Theme, options?: TransactionOptions): Promise<AislideDocument>
  assignLayout(slideId: string, layoutId: string, options?: TransactionOptions): Promise<AislideDocument>
  addObject(slideId: string, input: ObjectInput, options?: TransactionOptions): Promise<AislideDocument>
  addAsset(slideId: string, input: AssetInput, options?: TransactionOptions): Promise<AislideDocument>
  editSlides(operations: SlideOperation[], options?: TransactionOptions): Promise<AislideDocument>
  editElements(slideId: string, operations: ElementOperation[], options?: TransactionOptions): Promise<AislideDocument>
  addPart(slideId: string, input: { id: string; spec: PartSpec }, options?: TransactionOptions): Promise<AislideDocument>
  updatePart(slideId: string, input: { id: string; spec: PartSpec }, options?: TransactionOptions): Promise<AislideDocument>
  addGraph(slideId: string, input: { id: string; spec: GraphSpec }, options?: TransactionOptions): Promise<AislideDocument>
  updateGraph(slideId: string, input: { id: string; spec: GraphSpec }, options?: TransactionOptions): Promise<AislideDocument>
  applyGraph(slideId: string, input: { id: string; operations: GraphOperation[] }, options?: TransactionOptions): Promise<AislideDocument>
  undo(options?: RequestOptions): Promise<AislideDocument>
  redo(options?: RequestOptions): Promise<AislideDocument>
  exportProject(options?: RequestOptions): Promise<ProjectExport>
  exportPresentation(options?: RequestOptions): Promise<PresentationExport>
}
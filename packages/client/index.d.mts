import type { AislideDocument, Checkpoint, DataMapping, DataReport, Deck, Design, Element, ImportedObject, ObjectCatalog, ObjectKind, PatchOperation, PresentationExport, ProjectExport, Report, SourceBinding, SourceDocument, SourceInput, Theme } from './types'
import type { PartCatalog, PartSpec, GraphCatalog, GraphIcon, GraphSpec, GraphOperation, GraphCreation, GraphDiagnosticsSnapshot, SlideOperation, ElementOperation, AssetInput, DesignPreset } from './types'
import type { AuthoringProfile, BestPracticeGuide, BestPracticeProfiles, GuidedInput, GuidedReview } from './types'
import type { ArchitectureIconCatalog, ArchitectureIconAssets } from './types'
import type { MasterSourceInput, MasterSourceInspection, MasterImportInput, MasterImportPreview } from './types'
import type { AuthoringOperation, Frame, Crop, ConnectorSettings, PictureInput, RunStyle, ShapeAdjustment, SlideImportInput } from './types'
import type { AuthoringCapabilities, SearchOptions, TextMatch, TextReplaceOptions, FormatTextInput, ReplaceTextContentInput, UpdateParagraphsInput, CanvasResizeInput, TemplateKind, TemplateInput, TemplateExport, ImageEditInput, EditedImage, ApplyImageEditInput, EditTableInput, SelectionOperation, ElementBundle, SelectionEditResult } from './types'
export * from './types'
import type { FormatTextElementInput, ReplaceElementTextInput, CommentInput, ModernCommentOperation, TableHeaders, ElementAccessibility, ReadingOrderResult, AccessibilityReport, DocumentInspection, CleanCopyOptions, CleanCopyExport } from './types'
import type { SetTableCellTextInput, EditVectorInput, CombineShapesInput } from './types'
import type { EmbedFontInput, FontInfo, FontInspection, NativeFontInspection } from './types'
import type { DictionaryImport, ProofingDictionary, ProofTextInput, ProofResult, FormatSnapshot, CopyFormatInput, ApplyFormatInput } from './types'
import type { TextAssistInput, TextAssistance, ApplyTextAssistInput, SegmentedImage, SegmentationStatus, ProviderStatus } from './types'
export type RequestOptions = { signal?: AbortSignal; capacityProfile?: CapacityProfile }
export type CapacityProfile = 'legacy' | 'standard' | 'large'
export type SessionRecovery = { format: 'aislide.session'; version: 1; capacity_profile: CapacityProfile; document: AislideDocument; past: import('./types').UndoReceipt[]; future: import('./types').UndoReceipt[]; history_boundary: 'history_limit' | null }
export type CapacityLimits = Readonly<{ slides: number; elements_per_slide: number; elements_total: number; group_depth: number; document_bytes: number; request_bytes: number; archive_bytes: number; image_encoded_bytes: number; unique_images: number; raster_work_bytes: number; json_nodes: number; json_depth: number }>
export const CAPACITY_PROFILES: Readonly<Record<CapacityProfile, CapacityLimits>>
export const FONT_LIMITS: Readonly<{ face_bytes: number; total_bytes: number; faces: number }>
export function encodeCoreRequest(request: unknown): string
import type { StaticExportOptions, StaticExport, VerifiedRecoveryDocument, PreviewOptions, PresentationPreview, PreflightOptions, PreflightReport, RevisionEdit, RevisionPreview, DeliveryOptions, PreparedDelivery } from './types'
export type TransactionOptions = RequestOptions & { expectedRevision?: number }
export type AuthoringOptions = TransactionOptions & { expectedHash?: string }
export type SessionSummary = {
  document_id: string; revision: number; hash: string; title: string; width: number; height: number; slide_count: number
  capacity_profile: CapacityProfile; can_undo: boolean; can_redo: boolean; busy: boolean; next_offset: number | null
  slides: { id: string; title: string; element_count: number; has_notes: boolean }[]
  slide_id?: string; element_count?: number
  elements?: { id: string; type: Element['type']; parent_id: string | null; frame: Frame; text_preview?: string; text_truncated?: boolean; child_count?: number }[]
}
export type AssignLayoutOptions = TransactionOptions & { preserveFreeform?: boolean }
export type ReplaceOptions = TransactionOptions & { sources?: SourceDocument[]; bindings?: SourceBinding[]; report?: Report | null }
export type CoreTransport = <T>(request: unknown, options?: RequestOptions) => Promise<T>
export type ObjectInput = { id: string; kind: ObjectKind; preset?: string; rows?: number; columns?: number }
export class AislideClient {
  constructor(transport: CoreTransport, options?: Pick<RequestOptions, 'capacityProfile'>)
  request<T>(request: unknown, options?: RequestOptions): Promise<T>
  /** Current core schemas and limits; does not certify Office rendering. */
  authoringCapabilities(options?: RequestOptions): Promise<AuthoringCapabilities>
  computeChartPresentation(input: import('./types').ChartData, options?: RequestOptions): Promise<import('./types').ChartPresentation>
  renderElementPreview(element: Extract<Element, { type: 'chart' | 'text' | 'shape' }>, theme?: Theme, options?: RequestOptions): Promise<import('./types').ElementPreview>
  textAssist(input: TextAssistInput, options?: RequestOptions): Promise<TextAssistance>
  segmentationStatus(options?: RequestOptions): Promise<SegmentationStatus>
  segmentImage(input: Pick<ImageEditInput, 'base64' | 'mime_type'>, options?: RequestOptions): Promise<SegmentedImage>
  importProofingDictionary(input: DictionaryImport, options?: RequestOptions): Promise<ProofingDictionary>
  proofText(input: ProofTextInput, options?: RequestOptions): Promise<ProofResult>
  inspectFont(base64: string, options?: RequestOptions): Promise<FontInfo>
  inspectPptxFonts(base64: string, options?: RequestOptions): Promise<NativeFontInspection>
  inspectMasterSource(input: MasterSourceInput, options?: RequestOptions): Promise<MasterSourceInspection>
  capacityProfiles(options?: RequestOptions): Promise<{ default: CapacityProfile; legacy: CapacityLimits; standard: CapacityLimits; large: CapacityLimits }>
  verifySessionRecovery(envelope: SessionRecovery, options?: RequestOptions): Promise<SessionRecovery>
  recoverSession(envelope: SessionRecovery, options?: RequestOptions): Promise<DocumentSession>
  verifyRecovery(document: AislideDocument, options?: RequestOptions): Promise<VerifiedRecoveryDocument>
  recoverPresentation(document: AislideDocument, options?: RequestOptions): Promise<DocumentSession>
  formatTextElement(input: FormatTextElementInput, options?: RequestOptions): Promise<Element>
  replaceElementText(input: ReplaceElementTextInput, options?: RequestOptions): Promise<Element>
  setTableCellText(input: SetTableCellTextInput, options?: RequestOptions): Promise<Element>
  editVector(input: EditVectorInput, options?: RequestOptions): Promise<Element>
  /** Literal search. Paths are relative to Deck; offsets are Unicode scalars, end exclusive. */
  searchText(deck: Deck, input: SearchOptions, options?: RequestOptions): Promise<TextMatch[]>
  /** Pure local image preparation. Use session.applyImageEdit separately to mutate a picture. */
  editImage(input: ImageEditInput, options?: RequestOptions): Promise<EditedImage>
  /** Explicit POTX/THMX factory returning a new revision-zero document, never modifying another session. */
  importTemplate(id: string, input: TemplateInput, options?: RequestOptions): Promise<DocumentSession>
  ingest(input: SourceInput, options?: RequestOptions): Promise<SourceDocument>
  dataReport(source: SourceDocument, mapping: DataMapping, options?: RequestOptions): Promise<DataReport>
  designDefaults(options?: RequestOptions): Promise<Design>
  designCapabilities(options?: RequestOptions): Promise<import('./types').DesignCapabilities>
  designPresets(options?: RequestOptions): Promise<DesignPreset[]>
  objectCatalog(options?: RequestOptions): Promise<ObjectCatalog>
  partCatalog(options?: RequestOptions): Promise<PartCatalog>
  bestPracticeProfiles(options?: RequestOptions): Promise<BestPracticeProfiles>
  bestPracticeGuide(profileId: AuthoringProfile, options?: RequestOptions): Promise<BestPracticeGuide>
  validateGuidedPresentation(input: GuidedInput, options?: RequestOptions): Promise<GuidedReview>
  createGuidedPresentation(id: string, input: GuidedInput, options?: RequestOptions): Promise<{ session: DocumentSession; validation: GuidedReview; profile_id: AuthoringProfile; model_inference: false }>
  graphCatalog(options?: RequestOptions): Promise<GraphCatalog>
  architectureIcons(options?: RequestOptions): Promise<ArchitectureIconCatalog>
  architectureIconAssets(ids: string[], options?: RequestOptions): Promise<ArchitectureIconAssets>
  createGraphIcon(input: Pick<AssetInput, 'base64' | 'mime_type'> & { alt?: string }, options?: RequestOptions): Promise<GraphIcon>
  createGraph(input: { id: string; spec: GraphSpec; theme?: Theme; include_diagnostics: true }, options?: RequestOptions): Promise<GraphCreation>
  createGraph(input: { id: string; spec: GraphSpec; theme?: Theme; include_diagnostics?: false }, options?: RequestOptions): Promise<Element>
  createGraph(input: { id: string; spec: GraphSpec; theme?: Theme; include_diagnostics?: boolean }, options?: RequestOptions): Promise<Element | GraphCreation>
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
  constructor(transport: CoreTransport, document: AislideDocument, options?: Pick<RequestOptions, 'capacityProfile'>)
  static recover(transport: CoreTransport, envelope: SessionRecovery, options?: RequestOptions): Promise<DocumentSession>
  readonly capacityProfile: CapacityProfile
  readonly recoveryEnvelope: SessionRecovery
  readonly historyBoundary: 'history_limit' | null
  setCapacityProfile(profile: CapacityProfile, options?: RequestOptions): Promise<AislideDocument>
  readonly document: AislideDocument
  readonly revision: number
  readonly canUndo: boolean
  readonly canRedo: boolean
  readonly busy: boolean
  readonly fieldWarnings: string[]
  readonly graphDiagnostics: GraphDiagnosticsSnapshot | null
  getSummary(options?: { offset?: number; limit?: number; slideId?: string }): SessionSummary
  transact(operations: PatchOperation[], options?: TransactionOptions): Promise<AislideDocument>
  /** 1..128 typed operations, one core transaction and Undo per changed batch. Prefer add_part/add_graph for regenerable metadata; add_elements is unmanaged. add_graph accepts layout here only; update_graph retains its existing PartLayout. Core capacity and 128 total metadata entries still apply. */
  applyOperations(operations: AuthoringOperation[], options?: AuthoringOptions): Promise<AislideDocument>
  addElements(slideId: string, elements: Element[], options?: AuthoringOptions): Promise<AislideDocument>
  /** Geometry only; group child geometry and absolute table tracks scale, fonts and strokes do not. */
  setFrames(slideId: string, frames: { id: string; frame: Frame }[], options?: AuthoringOptions): Promise<AislideDocument>
  /** Partial overlay; retains unspecified run and paragraph styles, unlike applyFormat's format-painter semantics. */
  setTextStyle(slideId: string, input: { ids: string[]; style: RunStyle }, options?: AuthoringOptions): Promise<AislideDocument>
  setSlideBackground(slideId: string, color: string, options?: AuthoringOptions): Promise<AislideDocument>
  /** Replaces connector settings; omitted start/end/routing clear them. Visual properties remain unchanged. */
  setConnector(slideId: string, input: { id: string; connector: ConnectorSettings; frame?: Frame | null }, options?: AuthoringOptions): Promise<AislideDocument>
  setPictureCrop(slideId: string, input: { id: string; crop: Crop }, options?: AuthoringOptions): Promise<AislideDocument>
  setHyperlink(slideId: string, input: { id: string; link: string | null }, options?: AuthoringOptions): Promise<AislideDocument>
  setShapeAdjustment(slideId: string, input: { id: string; adjustment: ShapeAdjustment }, options?: AuthoringOptions): Promise<AislideDocument>
  addPicture(slideId: string, input: PictureInput, options?: AuthoringOptions): Promise<AislideDocument>
  /** Imports selected authored source slides with matching canvas in one Undo. Native source documents fail closed. */
  importSlides(source: AislideDocument, input: SlideImportInput, options?: AuthoringOptions): Promise<AislideDocument>
  replaceDeck(deck: Deck, options?: ReplaceOptions): Promise<AislideDocument>
  updateDesign(design: Design, options?: TransactionOptions): Promise<AislideDocument>
  setMasterTheme(masterId: string, theme: Theme | null, options?: TransactionOptions): Promise<AislideDocument>
  setDesignField(field: import('./types').DesignField, options?: TransactionOptions): Promise<AislideDocument>
  applyDesignPreset(presetId: string, options?: TransactionOptions): Promise<AislideDocument>
  applyTheme(theme: Theme, options?: TransactionOptions): Promise<AislideDocument>
  assignLayout(slideId: string, layoutId: string, options?: AssignLayoutOptions): Promise<AislideDocument>
  addObject(slideId: string, input: ObjectInput, options?: TransactionOptions): Promise<AislideDocument>
  addAsset(slideId: string, input: AssetInput, options?: TransactionOptions): Promise<AislideDocument>
  editSlides(operations: SlideOperation[], options?: TransactionOptions): Promise<AislideDocument>
  editElements(slideId: string, operations: ElementOperation[], options?: TransactionOptions): Promise<AislideDocument>
  updateNotes(slideId: string, notes: string, options?: TransactionOptions): Promise<AislideDocument>
  updateRichNotes(slideId: string, paragraphs: import('./types').RichParagraph[], options?: TransactionOptions): Promise<AislideDocument>
  updateAuxiliaryDesign(design: import('./types').AuxiliaryDesign, options?: TransactionOptions): Promise<AislideDocument>
  addComment(slideId: string, comment: CommentInput, options?: TransactionOptions): Promise<AislideDocument>
  modernComment(slideId: string, operation: ModernCommentOperation, options?: TransactionOptions): Promise<AislideDocument>
  setTableHeaders(slideId: string, elementId: string, policy: TableHeaders, options?: TransactionOptions): Promise<AislideDocument>
  replyComment(slideId: string, parentId: string, comment: CommentInput, options?: TransactionOptions): Promise<AislideDocument>
  resolveComment(slideId: string, commentId: string, resolved: boolean, options?: TransactionOptions): Promise<AislideDocument>
  removeComment(slideId: string, commentId: string, options?: TransactionOptions): Promise<AislideDocument>
  setAccessibility(slideId: string, elementId: string, metadata: ElementAccessibility | null, options?: TransactionOptions): Promise<AislideDocument>
  setReadingOrder(slideId: string, order: string[], options?: TransactionOptions): Promise<ReadingOrderResult>
  refreshFields(referenceDate: string, options?: TransactionOptions & { referenceTime?: string; locale?: string }): Promise<AislideDocument>
  checkAccessibility(options?: RequestOptions): Promise<AccessibilityReport>
  inspectDocument(options?: RequestOptions): Promise<DocumentInspection>
  exportCleanCopy(input: CleanCopyOptions, options?: RequestOptions): Promise<CleanCopyExport>
  searchText(input: SearchOptions, options?: RequestOptions): Promise<TextMatch[]>
  textAssist(input: TextAssistInput, options?: RequestOptions): Promise<TextAssistance>
  textAiStatus(options?: RequestOptions): Promise<ProviderStatus>
  applyTextAssist(slideId: string, input: ApplyTextAssistInput, options?: TransactionOptions): Promise<AislideDocument>
  importProofingDictionary(input: DictionaryImport, options?: RequestOptions): Promise<ProofingDictionary>
  proofText(input: ProofTextInput, options?: RequestOptions): Promise<ProofResult>
  copyFormat(slideId: string, input: CopyFormatInput, options?: RequestOptions): Promise<FormatSnapshot>
  sampleSlidePixel(slideId: string, x: number, y: number, options?: RequestOptions): Promise<import('./types').SlidePixelSample>
  applyFormat(slideId: string, input: ApplyFormatInput, options?: TransactionOptions): Promise<AislideDocument>
  setProofingLanguage(slideId: string, input: { id: string; language: string }, options?: TransactionOptions): Promise<AislideDocument>
  /** Choose replace_all=true or selected matches exclusively. Checks revision, expected match text and native preservation. */
  replaceText(input: TextReplaceOptions, options?: TransactionOptions): Promise<AislideDocument>
  /** Exact explicit family replacement; does not install fonts or replace theme tokens. */
  replaceFont(from: string, to: string, options?: TransactionOptions): Promise<AislideDocument>
  embedFont(input: EmbedFontInput, options?: TransactionOptions): Promise<AislideDocument>
  setFontUsage(sha256: string, licenseAcknowledged: boolean, options?: TransactionOptions): Promise<AislideDocument>
  listFonts(options?: RequestOptions): Promise<FontInspection>
  /** Apply a rich run style to [start,end) Unicode-scalar offsets in text or shape content. */
  formatText(slideId: string, input: FormatTextInput, options?: TransactionOptions): Promise<AislideDocument>
  replaceTextContent(slideId: string, input: ReplaceTextContentInput, options?: TransactionOptions): Promise<AislideDocument>
  /** Replace rich paragraphs and synchronize plain text. An empty array clears content. */
  updateParagraphs(slideId: string, input: UpdateParagraphsInput, options?: TransactionOptions): Promise<AislideDocument>
  editTable(slideId: string, input: EditTableInput, options?: TransactionOptions): Promise<AislideDocument>
  /** Copy leaves revision/history unchanged. Imported raw-copy and topology changes fail closed. Inspect effects for detached/stale metadata. */
  editSelection(slideId: string, operation: SelectionOperation, options?: TransactionOptions & { clipboard?: ElementBundle | null }): Promise<SelectionEditResult>
  combineShapes(slideId: string, input: CombineShapesInput, options?: TransactionOptions): Promise<SelectionEditResult>
  /** Each dimension is 320..4096. Scale may make part metadata stale; keep rejects out-of-bounds content. */
  resizeCanvas(input: CanvasResizeInput, options?: TransactionOptions): Promise<AislideDocument>
  /** Apply prepared image bytes without hidden processing; preserve frame/crop/alt, remove superseded SVG source. */
  applyImageEdit(slideId: string, input: ApplyImageEditInput, options?: TransactionOptions): Promise<AislideDocument>
  /** Return template bytes without writing a file or changing the document. */
  exportTemplate(kind: TemplateKind, options?: RequestOptions): Promise<TemplateExport>
  previewMasterImport(input: MasterImportInput, options?: TransactionOptions & { expectedHash?: string }): Promise<MasterImportPreview>
  importMasters(input: MasterImportInput, expectedCandidateHash: string, options?: TransactionOptions & { expectedHash?: string }): Promise<AislideDocument>
  exportStatic(input?: StaticExportOptions, options?: RequestOptions): Promise<StaticExport>
  previewPresentation(input?: PreviewOptions, options?: RequestOptions): Promise<PresentationPreview>
  preflightPresentation(input?: PreflightOptions, options?: RequestOptions): Promise<PreflightReport>
  prepareDelivery(input?: DeliveryOptions, options?: TransactionOptions & { expectedHash?: string }): Promise<PreparedDelivery>
  previewSlideRevision(slideId: string, edits: RevisionEdit[], options?: TransactionOptions & { expectedHash?: string; maxDimension?: number }): Promise<RevisionPreview>
  applySlideRevision(slideId: string, edits: RevisionEdit[], options: RequestOptions & { expectedRevision: number; expectedHash: string; candidateHash: string }): Promise<AislideDocument>
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
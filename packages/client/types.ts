export type Bounds = { id: string; x: number; y: number; width: number; height: number }
export type StaticExportFormat = 'png' | 'jpeg' | 'pdf'
export type StaticExportOptions = { format?: StaticExportFormat; page_indices?: number[] | null; scale?: number; transparent?: boolean; jpeg_quality?: number; jpeg_matte?: [number, number, number]; max_output_bytes?: number; deny_warnings?: boolean }
export type PreviewOptions = { page_indices?: number[] | null; max_dimension?: number; layout?: 'pages' | 'contact_sheet'; max_output_bytes?: number; format?: 'png' | 'jpeg'; overflow?: 'shrink' | 'error' }
export type PreviewImage = { base64: string; mime_type: 'image/png' | 'image/jpeg'; width: number; height: number; byte_length: number; sha256: string }
export type PresentationPreview = { revision: number; hash: string; requested_max_dimension: number; actual_max_dimension: number; quality_reduced: boolean; pages: { page_index: number; slide_id: string; image_index: number; x: number; y: number; width: number; height: number }[]; images: PreviewImage[]; warnings: StaticExport['warnings']; office_visual_parity: false }
export type PreflightOptions = { page_indices?: number[] | null; min_font_size?: number }
export type PreflightFinding = { code: string; severity: 'info' | 'warning' | 'error'; evidence: 'renderer' | 'geometry' | 'heuristic' | 'document'; page_index: number; slide_id: string; element_ids: string[]; scopes: string[]; bounds: { x: number; y: number; width: number; height: number }; message: string; suggestions: string[] }
export type PreflightReport = { revision: number; hash: string; page_indices: number[]; findings: PreflightFinding[]; checks: string[]; limitations: string[]; office_visual_parity: false; semantic_truth_verified: false }
export type RevisionEdit = { op: 'translate'; ids: string[]; dx: number; dy: number } | { op: 'align'; ids: string[]; alignment: 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom'; relative_to: 'selection' | 'page' } | { op: 'set_text_frame'; id: string; x: number; y: number; width: number; height: number } | { op: 'replace_text'; id: string; text: string } | { op: 'update_part'; id: string; spec: PartSpec } | { op: 'update_graph'; id: string; spec: GraphSpec }
export type RevisionPreview = { slide_id: string; base_revision: number; base_hash: string; candidate_hash: string; affected_ids: string[]; stale_part_ids: string[]; source_bindings_stale: boolean; native_preservation_checked: boolean; office_visual_parity: false; before: PresentationPreview; after: PresentationPreview }
export type DeliveryOptions = { page_indices?: number[] | null; pdf?: boolean; preview?: 'none' | 'pages' | 'contact_sheet'; notes?: boolean; source_report?: boolean; preflight?: boolean; max_dimension?: number; min_font_size?: number; max_output_bytes?: number }
export type DeliveryFile = { kind: 'pptx' | 'pdf' | 'preview' | 'notes' | 'source_report'; suffix: string; mime_type: string; page_indices: number[]; byte_length: number; sha256: string; base64: string; width?: number; height?: number }
export type DeliveryChecks = { document_verified: true; pptx_exported: true; native_preservation_checked: boolean; source_bindings_current: true; layout_measured: boolean; layout_scope?: 'visible_slide_elements_and_used_design' | 'not_measured_for_native_origin'; layout_issues: { code: string; severity: string; message: string }[]; preflight_page_indices: number[]; static_render_page_indices: number[]; office_visual_parity: false; source_authenticity_verified: false; source_freshness_verified: false; semantic_truth_verified: false; accessibility_checked: false }
export type DeliveryManifest = { format: 'aislide.delivery'; version: 1; producer: { name: string; version: string; engine: 'aislide-core'; contract_version: 1 }; document: { id: string; revision: number; hash: string; title: string; slide_count: number }; options: Required<DeliveryOptions>; files: (Omit<DeliveryFile, 'base64' | 'width' | 'height'> & { width: number | null; height: number | null })[]; preview_pages: PresentationPreview['pages']; multi_file_atomic: false; checks: DeliveryChecks; preflight: PreflightReport | null; render_warnings: StaticExport['warnings']; privacy: { pptx_includes_notes: true; pptx_may_include_sources_and_bindings: true; separate_notes_requested: boolean; separate_source_report_requested: boolean }; limitations: string[] }
export type PreparedDelivery = { revision: number; hash: string; files: DeliveryFile[]; manifest: DeliveryManifest }
export type StaticExportFile = { filename: string; base64: string; byte_length: number; mime_type: 'image/png' | 'image/jpeg' | 'application/pdf'; page_indices: number[]; width: number; height: number }
export type StaticExport = { files: StaticExportFile[]; warnings: { code: string; page_index: number; element_id: string; message: string }[]; office_parity_verified: false; pdf_rasterized: false; pdf_text_outlined: boolean; pdf_editable_text: false; pdf_tagged: boolean; pdf_searchable_text: boolean; pdf_selectable_text: boolean; pdf_semantic_overlay: boolean; pdf_ua_certified: false }
export type ImmutableValue<Value> = Value extends object ? { readonly [Key in keyof Value]: ImmutableValue<Value[Key]> } : Value
export type VerifiedRecoveryDocument = ImmutableValue<AislideDocument>
export type ChartKind = 'column' | 'bar' | 'line' | 'pie' | 'doughnut' | 'area' | 'scatter' | 'stacked_column' | 'stacked_bar' | 'percent_stacked_column' | 'percent_stacked_bar' | 'combo' | 'bubble' | 'radar' | 'radar_filled' | 'column3d' | 'bar3d' | 'pie3d' | 'funnel' | 'waterfall' | 'histogram' | 'box_whisker' | 'treemap' | 'sunburst'
export type ChartAxis = 'primary' | 'secondary'
export type AxisOptions = { min?: number | null; max?: number | null; major_unit?: number | null; minor_unit?: number | null; log_base?: number | null; reverse?: boolean; number_format?: string | null }
export type ChartDataLabels = { show_value?: boolean; show_category_name?: boolean; show_series_name?: boolean; show_percent?: boolean; position?: 'center' | 'inside_end' | 'outside_end' | 'best_fit' | null; number_format?: string | null }
export type HistogramOptions = { samples: number[]; binning: { rule: 'count'; count: number } | { rule: 'width'; width: number }; interval_closed: 'left' | 'right'; underflow?: number | null; overflow?: number | null }
export type BoxWhiskerOptions = { samples: number[][]; quartile_method: 'inclusive' | 'exclusive'; mean_line: boolean; mean_marker: boolean; nonoutliers: boolean; outliers: boolean }
export type HierarchyOptions = { paths: string[][]; parent_labels?: 'none' | 'banner' | 'overlapping' | null }
export type ChartOptions = { primary_axis?: AxisOptions; secondary_axis?: AxisOptions; category_axis?: AxisOptions; legend?: 'bottom' | 'top' | 'left' | 'right' | 'top_right' | 'hidden' | null; data_labels?: ChartDataLabels | null; waterfall_totals?: number[] | null; histogram?: HistogramOptions | null; box_whisker?: BoxWhiskerOptions | null; hierarchy?: HierarchyOptions | null }
export type ChartTrendline = { kind: 'linear' | 'exponential' | 'logarithmic' | 'polynomial' | 'power' | 'moving_average'; order?: number | null; period?: number | null; intercept?: number | null; forward?: number | null; backward?: number | null; display_equation?: boolean; display_r_squared?: boolean }
export type ChartErrorBars = { kind: 'fixed_value' | 'percentage' | 'standard_deviation' | 'standard_error' | 'custom'; direction?: 'x' | 'y'; bar_type?: 'both' | 'plus' | 'minus'; value?: number | null; plus?: number[] | null; minus?: number[] | null }
export type ChartSeries = { name: string; values: number[]; color: string; kind?: ChartKind | null; axis?: ChartAxis | null; bubble_sizes?: number[] | null; trendline?: ChartTrendline | null; error_bars?: ChartErrorBars | null }
export type ChartData = { kind: ChartKind; categories: string[]; series: ChartSeries[]; options?: ChartOptions }
export type PresentationAxis = { domain: [number, number]; log_base: number | null; reverse: boolean; ticks: { value: number; position: number; label: string }[]; minor_ticks: { value: number; position: number; label: string }[] }
export type ChartPresentation = { category_axis: PresentationAxis; primary_axis: PresentationAxis; secondary_axis: PresentationAxis | null; series: { axis: ChartAxis; points: { x: number; y: number }[]; labels: string[]; errors: { index: number; direction: 'x' | 'y'; lower: { x: number; y: number }; upper: { x: number; y: number } }[]; trend: { points: { x: number; y: number }[]; coefficients: number[]; center: number; scale: number; equation: string | null; r_squared: number | null } | null }[]; warnings: string[]; office_parity_verified: false }
export type ElementPreview = { svg: string; warnings: { code: string; message: string; element_id: string; page_index: number }[]; office_parity_verified: false }
export type HistogramData = { kind: 'column'; sample_count: number; bins: { lower: number; upper: number; upper_inclusive: boolean; count: number }[]; categories: string[]; series: ChartSeries[] }
export type ChartCapability = { id: ChartKind | 'histogram' | 'box_whisker' | 'treemap' | 'sunburst' | 'waterfall' | 'funnel' | 'line3d' | 'area3d' | 'surface3d'; native: boolean; create: boolean; read: boolean; edit: boolean; axis_options: boolean; secondary_axis: boolean; data_labels: boolean; trendlines: boolean; error_bars: boolean; bubble_sizes: boolean; derived: boolean; office_validated: boolean; limits?: { categories: number; series: number }; limitations: string[] }
export type Placeholder = { kind: 'title' | 'body' | 'subtitle' | 'footer' | 'date' | 'slide_number'; index: number }
export type RunStyle = { bold?: boolean | null; italic?: boolean | null; underline?: boolean | null; font_size?: number | null; color?: string | null; font_family?: string | null; baseline?: number | null; highlight?: string | null; language?: string | null }
export type KnownFieldKind = 'slidenum' | `datetime${1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13}`
export type RichField = { id: string; kind: string }
export type RichRun = { text: string; style?: RunStyle; field?: RichField | null }
export type RichSpacing = { kind: 'percent' | 'points'; value: number }
export type RichTab = { position: number; alignment?: 'left' | 'center' | 'right' | 'decimal' }
export type RichParagraph = { runs: RichRun[]; alignment?: 'left' | 'center' | 'right' | 'justify' | null; bullet?: 'none' | 'bullet' | 'numbered' | null; bullet_character?: string | null; numbering?: 'arabicPeriod' | 'arabicParenR' | 'arabicParenBoth' | 'arabicPlain' | 'alphaLcPeriod' | 'alphaUcPeriod' | 'alphaLcParenR' | 'alphaUcParenR' | 'romanLcPeriod' | 'romanUcPeriod' | null; number_start?: number | null; level?: number | null; margin_left?: number | null; indent?: number | null; line_spacing?: RichSpacing | null; space_before?: RichSpacing | null; space_after?: RichSpacing | null; tabs?: RichTab[] }
export type TextFormat = { italic?: boolean; underline?: boolean; alignment?: 'left' | 'center' | 'right' | 'justify'; vertical?: 'top' | 'middle' | 'bottom'; bullet?: 'none' | 'bullet' | 'numbered'; font_family?: string | null; hyperlink?: string | null; placeholder?: Placeholder | null; inherit_layout?: boolean; paragraphs?: RichParagraph[] }
export type Theme = { name: string; colors: Record<string, string>; fonts: { major: string; minor: string; east_asian: string; complex_script: string } }
export type Master = { id: string; name: string; background: string; elements: Element[]; theme?: Theme | null }
export type AuxiliaryMaster = { name: string; background: string; theme: Theme; elements: Element[] }
export type AuxiliaryDesign = { width: number; height: number; notes_master?: AuxiliaryMaster | null; handout_master?: AuxiliaryMaster | null }
export type DesignField = { master_id: string; layout_id?: string | null; kind: 'slide_number' | 'date' | 'footer'; reference_date: string; text?: string }
export type DesignCapabilities = { master_theme_override: true; default_theme_preserves_explicit_masters: true; native_theme_copy_on_write: true; field_kinds: KnownFieldKind[]; placeholder_kinds: DesignField['kind'][]; scopes: ('master' | 'layout')[]; reference_date: string; page_number_projection: true; native_fields: true; native_field_cache_refresh: true; unknown_fields: string; notes_master: true; handout_master: true; rich_notes: true; table_fields: false; office_recalculation_verified: false; studio: { master_theme: true; master_layout_fields: true; review_field_scope: true }; limitations: string[] }
export type SlideLayout = { id: string; name: string; master_id: string; background: string | null; elements: Element[] }
export type Design = { theme: Theme; masters: Master[]; layouts: SlideLayout[] }
export type MasterSourceInput = { kind: 'pptx' | 'potx'; base64: string }
export type MasterSourceInspection = { kind: MasterSourceInput['kind']; source_sha256: string; width: number; height: number; masters: { id: string; name: string; layout_count: number; importable: boolean; reason: string | null }[]; slides: { id: string; name: string; importable: boolean; reason: string | null }[]; warnings: string[]; office_visual_parity: false }
export type MasterImportInput = MasterSourceInput & { source_sha256: string; mode: 'masters' | 'slides'; ids: string[]; prefix: string; name: string }
export type MasterImportPreview = { base_revision: number; base_hash: string; source_sha256: string; candidate_hash: string; design: Design; master_ids: string[]; layout_ids: string[]; preview_slides: Slide[]; warnings: string[]; office_visual_parity: false }
export type DesignRegion = { layout_id: string; name: string; x: number; y: number; width: number; height: number }
export type DesignPreset = { id: string; name: string; design: Design; rules: { margin: number; gutter: number; heading_size: number; body_size: number; regions: DesignRegion[] } }
export type ObjectKind = 'text' | 'shape' | 'table' | 'chart' | 'line' | 'arrow'
export type ObjectCatalog = { shapes: { id: string; name: string }[]; charts: ChartKind[]; chart_capabilities: ChartCapability[]; table: { max_rows: number; max_columns: number } }
export type TableDimensions = { unit: 'relative' | 'absolute'; values: number[] }
export type TableMerge = { row: number; column: number; row_span: number; col_span: number }
export type TableCellStyle = { fill?: string | null; outline?: { color: string; width: number } | null; padding?: { left: number; right: number; top: number; bottom: number } | null; vertical?: 'top' | 'middle' | 'bottom' | null; text_style?: RunStyle | null; text_format?: TextFormat | null }
export type TableFormat = { column_widths?: TableDimensions | null; row_heights?: TableDimensions | null; merges?: TableMerge[]; cells?: { row: number; column: number; style: TableCellStyle }[] }
export type GradientStop = { offset: number; color: string; opacity: number }
export type Gradient = { kind: 'linear'; angle: number; stops: GradientStop[] } | { kind: 'radial'; center: [number, number]; stops: GradientStop[] }
export type Shadow = { color: string; opacity: number; blur: number; distance: number; angle: number }
export type Glow = { color: string; opacity: number; radius: number }
export type Reflection = { blur: number; distance: number; start_opacity: number; end_opacity: number; end_position: number }
export type TextWarp = 'arch_up' | 'arch_down' | 'wave1' | 'wave2' | 'inflate' | 'deflate' | 'slant_up' | 'slant_down'
export type PictureMask = 'ellipse' | 'round_rect' | 'diamond' | 'hexagon'
export type ShapeAdjustment = { name: 'adj'; value: number }
export type PathCommand = { op: 'move' | 'line'; point: [number, number] } | { op: 'quadratic'; control: [number, number]; point: [number, number] } | { op: 'cubic'; control1: [number, number]; control2: [number, number]; point: [number, number] } | { op: 'close' }
export type VectorPath = { commands: PathCommand[] }
export type ConnectionSite = { x: number; y: number; angle: number }
export type BooleanOperation = 'union' | 'intersect' | 'subtract' | 'xor' | 'fragment'
export type CombineShapesInput = { ids: string[]; operation: BooleanOperation; result_id: string }
export type GeometryCapabilities = { operations: BooleanOperation[]; max_shapes: number; max_vertices: number; max_path_commands: number; max_edge_pairs: number; max_fragments: number; native_coordinate_units: number; fill_rule: 'nonzero_opposite_winding'; clipping_fill_rule: 'even_odd'; curve_flattening: true; flatten_tolerance_px: number; flatten_max_depth: number; curve_output: 'polygon'; empty_result: 'reject_without_changes'; touching_contours: 'reject'; style_reference: 'first_supplied_id'; backend: 'geo/i_overlay'; office_visual_parity: false }
/** Degrees and scene pixels; alpha/offsets 0..1. Shape.rotation and connector routing own their legacy transforms. Opacity is shape fill or picture pixels only. Polygon.points must equal the path's complete control-point list. */
export type VisualStyle = { rotation?: number | null; flip_h?: boolean; flip_v?: boolean; hidden?: boolean; locked?: boolean; opacity?: number | null; gradient?: Gradient | null; shadow?: Shadow | null; glow?: Glow | null; soft_edge?: number | null; reflection?: Reflection | null; text_warp?: TextWarp | null; adjustments?: ShapeAdjustment[]; picture_mask?: PictureMask | null; path?: VectorPath | null; connection_sites?: ConnectionSite[] }
export type Element = Bounds & (
  | { type: 'text'; text: string; font_size: number; color: string; bold: boolean; format?: TextFormat; visual?: VisualStyle | null }
  | { type: 'rect'; fill: string; visual?: VisualStyle | null }
  | { type: 'polygon'; points: [number, number][]; fill: string; stroke: string; stroke_width: number; visual?: VisualStyle | null }
  | { type: 'shape'; preset: string; fill: string; stroke: string; stroke_width: number; rotation: number; text: string; font_size: number; color: string; bold: boolean; format?: TextFormat; visual?: VisualStyle | null }
  | { type: 'table'; rows: string[][]; font_size: number; format?: TableFormat }
  | ({ type: 'chart' } & ChartData)
  | { type: 'picture'; base64: string; mime_type: 'image/png' | 'image/jpeg'; alt: string; crop: { left: number; top: number; right: number; bottom: number }; visual?: VisualStyle | null; svg?: string | null }
  | { type: 'connector'; color: string; stroke_width: number; arrow: boolean; flip_v?: boolean; start?: Connection | null; end?: Connection | null; routing?: ConnectorRouting | null; visual?: VisualStyle | null }
  | { type: 'group'; view_width: number; view_height: number; children: Element[]; visual?: VisualStyle | null }
)
export type Comment = { id: string; author: string; initials: string; timestamp: string; text: string; x?: number; y?: number; parent_id?: string | null; resolved?: boolean; native_author_id?: number | null; native_index?: number | null }
export type CommentInput = Omit<Comment, 'parent_id' | 'native_author_id' | 'native_index'>
export type ModernCommentStatus = 'active' | 'resolved' | 'closed'
export type ModernCommentAuthor = { id: string; name: string; user_id: string; provider_id: string; initials?: string | null }
export type ModernCommentAnchor = { kind: 'unknown' | 'preserved' } | { kind: 'slide'; slide_id: number; slide_creation_id: number } | { kind: 'shape'; slide_id: number; slide_creation_id: number; shape_id: number; shape_creation_id: string } | { kind: 'text_range'; slide_id: number; shape_id: number; start: number; end: number }
export type ModernReply = { id: string; author: ModernCommentAuthor; created: string; status: ModernCommentStatus; body: RichParagraph[] }
export type ModernThread = ModernReply & { anchor: ModernCommentAnchor; replies: ModernReply[] }
export type ModernCommentDraft = { author_name: string; initials?: string | null; created: string; body: RichParagraph[] }
export type ModernCommentOperation =
  | { type: 'create'; draft: ModernCommentDraft; anchor: { kind: 'unknown' } }
  | { type: 'reply'; thread_id: string; draft: ModernCommentDraft }
  | { type: 'set_status'; comment_id: string; status: ModernCommentStatus }
  | { type: 'update_body'; comment_id: string; body: RichParagraph[] }
  | { type: 'remove'; comment_id: string }
export type TableHeaders = 'unknown' | 'none' | 'first_row' | 'first_column' | 'both'
export type ElementAccessibility = { title?: string | null; description?: string | null; decorative?: boolean | null }
export type SlideReview = { comments?: Comment[]; modern_threads?: ModernThread[] | null; table_headers?: Record<string, TableHeaders>; accessibility?: Record<string, ElementAccessibility>; reading_order?: string[] }
export type AccessibilityIssue = { code: string; slide_id: string; element_id: string | null; status: string; contrast_ratio: number | null; required_ratio: number | null; repair_target: 'alternative_text' | 'table_headers' | null }
export type AccessibilityReport = { issues: AccessibilityIssue[]; checked_features: string[]; limitations: string[]; wcag_certified: false }
export type InspectionCategory = 'sources' | 'custom_xml' | 'comments' | 'notes' | 'unused_media' | 'off_slide' | 'invisible'
export type PersonalDataCandidate = { rule: 'email_candidate' | 'phone_candidate' | 'personal_property_candidate'; scope: 'current_deck' | 'embedded_origin'; surface: string; count: number; locations: number[][]; masked_value: '[REDACTED]' }
export type DocumentInspection = { findings: { category: InspectionCategory; count: number; paths: string[] }[]; candidates: PersonalDataCandidate[]; candidate_scan_truncated: boolean; limitations: string[]; complete_personal_data_detection: false }
export type CleanCopyOptions = { new_document_id: string; categories: InspectionCategory[]; confirmed: boolean }
export type CleanCopyExport = Exported & { document: AislideDocument; inspection: DocumentInspection }
export type ReadingOrderResult = { document: AislideDocument; warnings: string[] }
export type Slide = { id: string; title: string; background: string; elements: Element[]; notes: string; notes_paragraphs?: RichParagraph[]; layout_id?: string | null; inherit_background?: boolean; hide_master_graphics?: boolean; native_source_id?: string | null; review?: SlideReview | null }
export type SlideOperation =
  | { op: 'insert'; id: string; after?: string | null; title: string; layout_id?: string | null }
  | { op: 'duplicate'; slide_id: string; id: string }
  | { op: 'remove'; slide_id: string }
  | { op: 'move'; slide_id: string; index: number }
  | { op: 'rename'; slide_id: string; title: string }
export type AssetInput = { id: string; base64: string; mime_type: 'image/svg+xml' | 'image/png' | 'image/jpeg' | 'image/emf' | 'image/wmf'; alt: string; size: number }
export type ElementOperation = { op: 'duplicate'; id: string; new_id: string } | { op: 'remove'; id: string } | { op: 'order'; id: string; index: number }
export type FontStyle = 'regular' | 'bold' | 'italic' | 'bold_italic'
export type FontPermission = 'installable' | 'editable' | 'preview_print' | 'restricted' | 'bitmap_only' | 'unknown'
export type EmbeddedFont = { family: string; style: FontStyle; base64: string; license_acknowledged: boolean }
export type FontInfo = { family: string; style: FontStyle; fs_type: number | null; permission: FontPermission; no_subsetting: boolean; byte_length: number; sha256: string; format: 'ttf' | 'otf'; usable: boolean; license_verified: false; copyright: string | null; license_description: string | null; license_url: string | null }
export type FontInspection = { fonts: FontInfo[]; office_verified: false }
export type NativeFontInspection = { fonts: { family: string; style: FontStyle; sha256: string | null; byte_length: number; info: FontInfo | null; status: string }[]; office_verified: false }
export type EmbedFontInput = { base64: string; license_acknowledged: boolean }
export type Deck = { version: 1; title: string; width: number; height: number; slides: Slide[]; design?: Design | null; auxiliary_design?: AuxiliaryDesign | null; embedded_fonts?: EmbeddedFont[] }
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
export type UndoReceipt = { document_id: string; after_hash: string; inverse: PatchOperation[] }

export type Frame = Omit<Bounds, 'id'>
export type Crop = { left?: number; top?: number; right?: number; bottom?: number }
export type Connection = { element_id: string; site: number }
export type ConnectorRouting = { points: [number, number][]; start_arrow?: boolean; dashed?: boolean; custom?: boolean }
export type ConnectorSettings = { color: string; stroke_width: number; arrow: boolean; flip_v?: boolean; start?: Connection | null; end?: Connection | null; routing?: ConnectorRouting | null }
export type PictureFit = 'contain' | 'cover' | 'stretch'
export type PictureInput = { id: string; base64: string; mime_type: 'image/png' | 'image/jpeg'; alt: string; frame?: Frame | null; crop?: Crop; fit?: PictureFit }
export type AuthoringOperation =
  | { op: 'add_elements'; slide_id: string; elements: Element[] }
  | { op: 'set_frame'; slide_id: string; id: string; frame: Frame }
  | { op: 'set_text_style'; slide_id: string; ids: string[]; style: RunStyle }
  | { op: 'set_slide_background'; slide_id: string; color: string }
  | { op: 'update_notes'; slide_id: string; notes: string }
  | { op: 'set_table_headers'; slide_id: string; element_id: string; policy: TableHeaders }
  | { op: 'set_accessibility'; slide_id: string; element_id: string; metadata: ElementAccessibility | null }
  | { op: 'set_connector'; slide_id: string; id: string; connector: ConnectorSettings; frame?: Frame | null }
  | { op: 'set_picture_crop'; slide_id: string; id: string; crop: Crop }
  | { op: 'set_hyperlink'; slide_id: string; id: string; link: string | null }
  | { op: 'set_shape_adjustment'; slide_id: string; id: string; adjustment: ShapeAdjustment }
  | ({ op: 'add_picture'; slide_id: string } & PictureInput)
  | { op: 'add_part'; slide_id: string; id: string; spec: PartSpec }
  | { op: 'update_part'; slide_id: string; id: string; spec: PartSpec }
  | { op: 'add_graph'; slide_id: string; id: string; spec: GraphSpec; layout?: PartLayout | null }
  | { op: 'update_graph'; slide_id: string; id: string; spec: GraphSpec }
export type SlideImportInput = { source_slide_ids: string[]; prefix: string; after?: string | null }

export type PartItem = { label: string; detail?: string; value?: number | null }
export type PartData =
  | { kind: 'chart'; categories: string[]; series: { name: string; values: number[] }[]; x_axis?: string; y_axis?: string }
  | { kind: 'items'; items: PartItem[]; center?: string }
  | { kind: 'tree'; nodes: { id: string; label: string; parent?: string | null }[] }
  | { kind: 'network'; nodes: PartItem[]; edges: { from: number; to: number; label?: string }[] }
  | { kind: 'matrix'; corner_label?: string; rows: string[]; columns: string[]; cells: string[][] }
  | { kind: 'groups'; groups: { label: string; items: string[] }[] }
  | { kind: 'timeline'; periods: string[]; tasks: { label: string; start: number; end: number; progress?: number }[] }
  | { kind: 'waterfall'; steps: { label: string; value: number; total?: boolean }[]; unit?: string }
  | { kind: 'map'; points: { label: string; longitude: number; latitude: number; value?: number | null }[] }
  | { kind: 'diagram'; graph: GraphSpec }
export type PartLayout = Frame & { show_title?: boolean }
export type PartSpec = { version: 1; preset: string; title: string; subtitle?: string; data: PartData; layout?: PartLayout | null }
export type PartPreset = { id: string; category: string; category_name: string; name: string; family: 'Charts' | 'Diagrams'; example: PartSpec; recommended?: boolean; use_when?: string; avoid_when?: string }
export type PartCatalog = { version: 1; presets: PartPreset[]; schema: unknown; style: string; default_bounds: Omit<Bounds, 'id'> }
export type PartInstance = { slide_id: string; element_id: string; spec: PartSpec; render_sha256: string; native_sha256?: string | null; stale: boolean }

export type AuthoringProfile = 'consulting-decision' | 'technical-explainer' | 'event-talk' | 'status-report'
export type GuidedEvidence = { id: string; kind: 'source' | 'assumption' | 'unknown'; reference: string; statement: string }
export type ClauseSupport = { clause: string; body_paths: string[]; evidence_ids: string[] }
export type NumberEvidence = { path: string; value: Scalar; evidence_id: string }
export type DecisionIssue = { id: string; question: string; requested_decision: string; criterion: string; owner: string; due: string; evidence_ids: string[]; analysis_slide_ids: string[] }
export type GuidedAuthoring = { context?: 'reading' | 'projection' | null; density?: 'comfortable' | 'compact' | null; spacing?: 'standard' | 'relaxed' | null; body_font_min?: number | null; headline_font_size?: number | null; font_family?: string | null; headline_style?: 'sentence' | 'keyword' | null; slide_limit?: number | null }
export type GuidedSlide = { id: string; section: string; headline: string; sentence_form: 'causal' | 'conditional' | 'contrast' | 'causal-focus' | 'evaluation' | 'proposal' | 'explanation' | 'comparison' | 'outcome'; pattern_id: string; question: string; parent_message: string; transition: string; parallel_basis: string; part?: PartSpec | null; support: ClauseSupport[]; numbers?: NumberEvidence[]; speaker_notes?: string | null }
export type GuidedInput = { version: 1; profile_id: AuthoringProfile; title: string; audience: string; purpose: string; governing_message: string; language: 'en' | 'ja'; brand_color?: string | null; authoring?: GuidedAuthoring | null; evidence: GuidedEvidence[]; issues?: DecisionIssue[]; slides: GuidedSlide[] }
export type GuidedReview = { ready: boolean; issues: string[]; review_required: string[]; semantic_truth_verified: false; office_visual_parity: false }
export type BestPracticeGuide = { version: 1; profile_id: AuthoringProfile; title: string; language: 'en'; markdown: string; patterns: { id: string; name: string; purpose: string; capability: 'native-template' | 'composition-required' | 'guidance-only'; rule: string }[]; input_schema: unknown; automatic_patterns: string[]; semantic_truth_verified: false; limits: { slides: number; evidence: number; issues: number }; creation: string }
export type BestPracticeProfiles = { version: 1; profiles: { id: AuthoringProfile; title: string; language: 'en'; default_primary: string }[] }

export type GraphNodeKind = 'rectangle' | 'rounded_rectangle' | 'ellipse' | 'diamond' | 'cylinder' | 'cloud'
export type GraphPresentation = 'card' | 'icon'
export type GraphPort = 'auto' | 'top' | 'left' | 'bottom' | 'right'
export type GraphRoute = 'straight' | 'elbow' | 'manual'
export type GraphIcon = { base64: string; mime_type: 'image/png' | 'image/jpeg'; alt?: string }
export type GraphTextAlign = 'left' | 'center' | 'right'
export type GraphLabelFit = 'wrap' | 'shrink'
export type GraphNode = { id: string; label: string; label_fit?: GraphLabelFit; detail?: string | null; detail_font_size?: number | null; text_align?: GraphTextAlign | null; heading_bold?: boolean; kind?: GraphNodeKind; presentation?: GraphPresentation; x: number; y: number; width?: number; height?: number; fill?: string; stroke?: string; color?: string; font_size?: number; group?: string | null; icon?: GraphIcon | null }
export type GraphLabelPlacement = { position: number; side?: 'above' | 'below'; offset?: number; on_overlap?: 'warn' | 'error' }
export type GraphBadge = { number: number; position?: number; size?: number; font_size?: number; fill?: string; color?: string }
export type GraphEdge = { id: string; source: string; target: string; source_port?: GraphPort; target_port?: GraphPort; label?: string; route?: GraphRoute; color?: string; arrow?: boolean; start_arrow?: boolean; dashed?: boolean; stroke_width?: number | null; label_color?: string | null; label_font_size?: number | null; source_offset?: number | null; target_offset?: number | null; waypoints?: [number, number][]; label_placement?: GraphLabelPlacement | null; badge?: GraphBadge | null }
export type GraphGroup = { id: string; label: string; x: number; y: number; width: number; height: number; fill?: string; stroke?: string; parent?: string | null; icon?: GraphIcon | null; padding?: number | null; header_height?: number | null; header_font_size?: number | null; header_color?: string | null }
export type GraphSpec = { version: 1; title: string; subtitle?: string; show_title?: boolean; nodes: GraphNode[]; edges?: GraphEdge[]; groups?: GraphGroup[] }
export type GraphFinding = { code: string; severity: 'warning' | 'info'; graph_id: string; entity_id: string; element_ids: string[]; bounds: [number, number, number, number]; message: string; slide_id?: string; lines?: number; font_size?: number }
export type GraphDiagnostics = { status: 'complete' | 'partial' | 'unavailable'; findings: GraphFinding[] }
export type GraphDiagnosticsSnapshot = GraphDiagnostics & { revision: number; hash: string }
export type GraphCreation = { element: Element; diagnostics: GraphDiagnostics }
export type GraphAlignment = 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom'
export type GraphOperation =
  | { op: 'put_node'; node: GraphNode }
  | { op: 'put_edge'; edge: GraphEdge }
  | { op: 'put_group'; group: GraphGroup }
  | { op: 'move'; ids: string[]; dx: number; dy: number }
  | { op: 'remove'; ids: string[] }
  | { op: 'align'; ids: string[]; alignment: GraphAlignment }
  | { op: 'layout'; columns: number }
export type GraphCatalog = { version: 1; shapes: GraphNodeKind[]; ports: GraphPort[]; routes: GraphRoute[]; limits: { nodes: number; edges: number; groups: number; group_depth?: number; rendered_elements: number }; canvas: { width: number; height: number; content_top: number }; schema: unknown; operation_schema: unknown; examples: { id: string; name: string; spec: GraphSpec }[] }

export type SearchOptions = { query: string; case_sensitive?: boolean; whole_word?: boolean; include_notes?: boolean; include_masters?: boolean; max_matches?: number }
export type ArchitectureIconProviderId = 'azure' | 'aws' | 'gcp'
export type ArchitectureIconArchive = { id: string; url: string; sha256: string; bytes: number }
export type ArchitectureIconProvider = { id: ArchitectureIconProviderId; name: string; terms_url: string; notice: string; archives: ArchitectureIconArchive[] }
export type ArchitectureIconSource = { archive_id: string; entry: string; sha256: string }
export type ArchitectureIconEntry = { id: string; provider: ArchitectureIconProviderId; name: string; kind: string; categories: string[]; aliases: string[]; source: ArchitectureIconSource; png_sha256: string; png_bytes: number; width: number; height: number }
export type ArchitectureIconCatalog = { version: 1; release: string; configured: boolean; message: string; providers: ArchitectureIconProvider[]; icons: ArchitectureIconEntry[] }
export type ArchitectureIconAsset = { id: string; base64: string; mime_type: 'image/png'; alt: string; width: number; height: number }
export type ArchitectureIconAssets = { icons: ArchitectureIconAsset[] }
export type TextMatch = { path: string; start: number; end: number; expected: string; snippet: string }
export type TextReplaceOptions = { search: SearchOptions; replacement: string; replace_all?: boolean; selected?: TextMatch[] | null }
export type FormatTextInput = { id: string; start: number; end: number; style: RunStyle }
export type ProofingDictionary = { language: string; words: string[]; synonyms?: Record<string, string[]>; translations?: Record<string, Record<string, string>> }
export type DictionaryImport = { language: string; format: 'wordlist' | 'json'; content: string }
export type ProofTextInput = { text: string; language: string; dictionary?: ProofingDictionary | null; term?: string; target_language?: string }
export type TextAssistInput = { task: 'proofread' | 'translate'; text: string; language: string; target_language?: string | null }
export type TextAssistCandidate = { text: string; source_sha256: string }
export type TextAssistance = { candidate: TextAssistCandidate; provenance: { mode: 'model'; model: string; remote: false; source_sha256: string; elapsed_ms: number; verified: false; attempts: 1 } }
export type ApplyTextAssistInput = { id: string; expected_text: string; candidate: TextAssistCandidate }
export type SegmentationStatus = { configured: boolean; model: 'u2netp'; sha256: string; runtime: string; remote: false; tensor_budget_bytes: number; message: string }
export type SegmentedImage = { image: EditedImage; provenance: { mode: 'model'; model: 'u2netp'; model_sha256: string; source_sha256: string; runtime: string; remote: false; elapsed_ms: number; planned_tensor_bytes: number; verified: false } }
export type ProofIssue = { word: string; start: number; end: number; suggestions: string[] }
export type ProofResult = { language: string; sample_dictionary: boolean; complete_dictionary: false; issues: ProofIssue[]; synonyms: string[]; translation: string | null }
export type PaintEffects = Pick<VisualStyle, 'shadow' | 'glow' | 'soft_edge' | 'reflection'>
export type SurfaceStyle = { fill: string; stroke: string | null; stroke_width: number | null; opacity: number | null; gradient: Gradient | null }
export type TextFormatSnapshot = { run: Omit<RunStyle, 'highlight' | 'language'>; paragraph: RichParagraph & { runs: [] }; vertical: 'top' | 'middle' | 'bottom'; surface?: SurfaceStyle | null; effects?: PaintEffects | null; text_warp?: TextWarp | null }
export type FormatSnapshot = TextFormatSnapshot
  | { kind: 'filled'; surface: SurfaceStyle; effects: PaintEffects }
  | { kind: 'picture'; opacity: number | null; effects: PaintEffects }
  | { kind: 'table'; font_size: number; rows: number; columns: number; cells: NonNullable<TableFormat['cells']> }
  | { kind: 'chart'; colors: string[]; legend: ChartOptions['legend']; data_labels: ChartOptions['data_labels']; axis_formats: [string | null, string | null, string | null] }
export type SlidePixelSample = { x: number; y: number; color: string; rgba: [number, number, number, number]; warnings: StaticExport['warnings']; office_parity_verified: false }
export type CopyFormatInput = { id: string; paragraph_index?: number; run_index?: number }
export type ApplyFormatInput = { ids: string[]; style: FormatSnapshot }
export type FormatTextElementInput = { element: Element; start: number; end: number; style: RunStyle }
export type ReplaceElementTextInput = { element: Element; text: string }
export type SetTableCellTextInput = { element: Element; row: number; column: number; text: string }
export type EditVectorInput = { element: Element; path: VectorPath }
export type ReplaceTextContentInput = { id: string; text: string }
export type UpdateParagraphsInput = { id: string; paragraphs: RichParagraph[] }
export type CanvasResizeInput = { width: number; height: number; mode: 'scale' | 'keep' }
export type TemplateKind = 'potx' | 'thmx'
export type TemplateInput = { kind: TemplateKind; base64: string }
export type TemplateExport = { base64: string; filename: 'template.potx' | 'template.thmx' }
export type ImageOutputFormat = { kind: 'png' } | { kind: 'jpeg'; quality: number; matte?: [number, number, number] | null }
export type ImageEditParams = { brightness?: number; contrast?: number; saturation?: number; grayscale?: boolean; background_key?: { color: [number, number, number]; tolerance: number } | null; resize_longest_side?: number | null; format?: ImageOutputFormat }
export type ImageEditInput = { base64: string; mime_type: 'image/png' | 'image/jpeg'; params: ImageEditParams }
export type EditedImage = { base64: string; mime_type: 'image/png' | 'image/jpeg'; width: number; height: number }
export type ApplyImageEditInput = { id: string; image: EditedImage }
export type TableOperation =
  | { op: 'update_format'; format: TableFormat }
  | { op: 'set_cell_style'; row: number; column: number; style: TableCellStyle }
  | { op: 'set_cell_text'; row: number; column: number; text: string }
  | { op: 'merge'; region: TableMerge }
  | { op: 'split'; row: number; column: number }
  | { op: 'insert_row' | 'insert_column'; index: number; values: string[] }
  | { op: 'remove_row' | 'remove_column'; index: number }
export type EditTableInput = { id: string; operations: TableOperation[] }
export type SelectionOperation =
  | { op: 'translate'; ids: string[]; dx: number; dy: number }
  | { op: 'resize'; ids: string[]; x: number; y: number; width: number; height: number }
  | { op: 'align'; ids: string[]; alignment: 'left' | 'center' | 'right' | 'top' | 'middle' | 'bottom'; relative_to: 'selection' | 'page' }
  | { op: 'distribute'; ids: string[]; axis: 'horizontal' | 'vertical'; relative_to: 'selection' | 'page' }
  | { op: 'group'; ids: string[]; group_id: string }
  | { op: 'ungroup'; ids: string[] }
  | { op: 'copy' | 'cut'; ids: string[]; format: 'keep_source_formatting' }
  | { op: 'paste'; id_prefix: string; dx: number; dy: number }
export type ClipboardSource = { id: string; hash: string; origin_sha256?: string | null }
export type ElementBundle = { version: 1; format: 'keep_source_formatting'; source_slide_id: string; source_theme: Theme; elements: Element[]; source_document?: ClipboardSource | null }
export type SelectionEffects = { affected_ids: string[]; added_ids: string[]; removed_ids: string[]; reparented_ids: string[]; id_map: Record<string, string>; metadata_review_required: boolean; native_validation_required: boolean; raw_native_preserved: boolean; warnings: string[] }
export type SelectionEditResult = { document: AislideDocument; clipboard: ElementBundle | null; effects: SelectionEffects; changes: string[] }
export type ReviewCapabilities = { field_kinds: KnownFieldKind[]; field_reference_date: string; native_field_refresh: boolean; comments: string; authenticated_authors: false; modern_powerpoint_threads: boolean; reading_order_changes_z_order: true; complete_personal_data_detection: false; wcag_certified: false; clean_copy: string; limitations: string[] }
export type VisualAuthoringCapabilities = { preview_pages: number; preview_max_dimension: number; preview_encoded_bytes: number; preview_wire_bytes: number; preflight_findings: number; preflight_objects_per_page: number; revision_edits: number; revision_targets_per_edit: number; revision_edit_bytes: number; preview_schema: unknown; preflight_schema: unknown; revision_edit_schema: unknown; office_visual_parity: false; semantic_truth_verified: false }
export type DeliveryCapabilities = { visual_pages: number; output_bytes: number; mcp_response_bytes: number; mcp_max_files_including_manifest: number; options_schema: unknown; notes_opt_in: true; source_report_opt_in: true; multi_file_atomic: false }
export type MasterImportCapabilities = { formats: MasterSourceInput['kind'][]; modes: MasterImportInput['mode'][]; selection_limit: number; masters_limit: number; layouts_limit: number; same_canvas_required: true; append_only: true; automatic_assignment: false; source_fonts_imported: false; unsupported_content: string; office_visual_parity: false }
export type TypedAuthoringCapabilities = { batch_limit: number; element_roots_per_add: number; managed_parts_limit: number; managed_metadata_persisted: true; operations: AuthoringOperation['op'][]; scale_fonts: false; scale_strokes: false; one_undo: true }
export type SlideImportCapabilities = { source: 'authored_document_only'; selected_slides: number; same_canvas: true; design_deduplication: true; office_visual_parity: false }
export type AuthoringCapabilities = { version: 1; operations: string[]; limits: { request_bytes: number; document_bytes: number; operations: number; canvas_min: number; canvas_max: number; search_matches: number; selection_ids: number; table_rows: number; table_columns: number; image_bytes: number; image_dimension: number }; selection_schema: unknown; search_schema: unknown; replace_schema: unknown; charts: ChartCapability[]; templates: TemplateKind[]; office_visual_parity: false; raw_clipboard_xml: false; limitations: string[]; review: ReviewCapabilities; geometry: GeometryCapabilities; visual_authoring: VisualAuthoringCapabilities; delivery: DeliveryCapabilities; master_import: MasterImportCapabilities; typed_authoring: TypedAuthoringCapabilities; slide_import: SlideImportCapabilities }
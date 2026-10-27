use serde::{Deserialize, Serialize};

/// JSON document returned by the import endpoint and accepted by export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasDocument {
    pub presentation: Presentation,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Presentation {
    pub title: String,
    pub preserve_element_order: bool,
    pub show_branding: bool,
    pub slides: Vec<Slide>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    pub id: String,
    pub name: String,
    pub width: f64,
    pub height: f64,
    pub background_color: String,
    pub elements: Vec<Element>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Element {
    Text(TextElement),
    Shape(ShapeElement),
    Line(LineElement),
    Image(ImageElement),
}

impl Element {
    /// Returns the stable canvas identifier shared by every drawable element variant.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Text(element) => &element.id,
            Self::Shape(element) => &element.id,
            Self::Line(element) => &element.id,
            Self::Image(element) => &element.id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub text: String,
    pub color: String,
    pub font_face: String,
    pub font_size: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub break_line: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextElement {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub text: String,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f64,
    pub text_color: String,
    pub font_size: f64,
    pub font_face: String,
    pub padding: f64,
    pub align: HorizontalAlign,
    pub valign: VerticalAlign,
    pub runs: Vec<TextRun>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotate: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_h: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_v: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub border_radius: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub fill_opacity: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub stroke_opacity: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeElement {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub shape: String,
    #[serde(default)]
    pub text: String,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f64,
    pub text_color: String,
    pub font_size: f64,
    pub font_face: String,
    pub padding: f64,
    pub align: HorizontalAlign,
    pub valign: VerticalAlign,
    pub runs: Vec<TextRun>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotate: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_h: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_v: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub border_radius: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub fill_opacity: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub stroke_opacity: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineElement {
    #[serde(default)]
    pub elbow_direction: ElbowDirection,
    pub id: String,
    #[serde(default)]
    pub line_type: LineType,
    #[serde(default = "half")]
    pub bend_position: f64,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub stroke: String,
    pub stroke_width: f64,
    pub dash: DashStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub begin_arrow: Option<ArrowType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_arrow: Option<ArrowType>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub stroke_opacity: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotate: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageElement {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub src: String,
    pub fit: ImageFit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<ImageCrop>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alt_text: String,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotate: f64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_h: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_v: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub border_radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DashStyle {
    Solid,
    Dash,
    Dot,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ElbowDirection {
    #[default]
    HorizontalFirst,
    VerticalFirst,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineType {
    #[default]
    Straight,
    Elbow,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArrowType {
    Triangle,
    Arrow,
    Diamond,
    Oval,
    Stealth,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFit {
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageCrop {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StoredTemplate {
    pub template_id: String,
    pub template_json: CanvasDocument,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateKind {
    Commentary,
    #[default]
    Diagram,
}

impl TemplateKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commentary => "commentary",
            Self::Diagram => "diagram",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateSource {
    Builtin,
    Import,
}

impl TemplateSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Import => "import",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StoredTemplateMetadata {
    pub template_id: String,
    pub kind: TemplateKind,
    pub title: String,
    pub description: String,
    pub source: TemplateSource,
    pub created_at: i64,
    pub catalog_version: Option<i64>,
    pub catalog_checksum: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredTemplatePreview {
    pub template_id: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredTemplatePreviewPage {
    pub previews: Vec<StoredTemplatePreview>,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateInsert {
    pub template: StoredTemplate,
    pub assets: Vec<TemplateAsset>,
    pub metadata: StoredTemplateMetadata,
    pub preview: Option<StoredTemplatePreview>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredTemplateCatalogItem {
    pub template: StoredTemplate,
    pub metadata: StoredTemplateMetadata,
    pub has_preview: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredTemplateWithAssets {
    pub template_id: String,
    pub template_json: CanvasDocument,
    pub assets: Vec<TemplateAsset>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateAsset {
    pub asset_id: String,
    pub template_id: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResponse {
    pub template_id: String,
    pub template_json: CanvasDocument,
    pub warnings: Vec<String>,
    pub preview_status: TemplatePreviewStatus,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchImportTemplate {
    pub template_id: String,
    pub preview_available: bool,
    pub template_json: CanvasDocument,
}

#[derive(Clone, Debug, Serialize)]
pub struct BatchImportResponse {
    pub templates: Vec<BatchImportTemplate>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplatePreviewStatus {
    Ready,
    Unavailable,
}

impl TemplatePreviewStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateResponse {
    pub template_id: String,
    pub template_json: CanvasDocument,
}

/// Lightweight stored-template entry returned by the library endpoint.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateListItem {
    pub template_id: String,
    pub title: String,
    pub description: String,
    pub kind: TemplateKind,
    pub slide_count: usize,
    pub element_count: usize,
    pub preview_url: Option<String>,
}

/// Collection returned by `GET /templates`.
#[derive(Clone, Debug, Serialize)]
pub struct TemplateListResponse {
    pub templates: Vec<TemplateListItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewPageMetadata {
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
    pub has_previous_page: bool,
    pub has_next_page: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewListItem {
    pub template_id: String,
    pub preview_url: String,
    pub content_type: String,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct TemplatePreviewListResponse {
    pub pagination: TemplatePreviewPageMetadata,
    pub previews: Vec<TemplatePreviewListItem>,
}

#[derive(Clone, Debug)]
pub struct ExportedPowerPoint {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct InsertPowerPointInput {
    pub presentation: serde_json::Value,
    pub target_bytes: Vec<u8>,
    pub target_file_name: String,
    pub insert_after_slide: usize,
}

#[derive(Clone, Debug)]
pub struct Conversion {
    pub template_json: CanvasDocument,
    pub warnings: Vec<String>,
}

const fn one() -> f64 {
    1.0
}

const fn half() -> f64 {
    0.5
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_zero(value: &f64) -> bool {
    value.abs() < f64::EPSILON
}

fn is_one(value: &f64) -> bool {
    (*value - 1.0).abs() < f64::EPSILON
}

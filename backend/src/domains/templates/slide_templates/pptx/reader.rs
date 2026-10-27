use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use super::super::error::FailureKind;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use roxmltree::{Document, Node};
use zip_slide::ZipArchive;

use super::super::{
    ArrowType, CanvasDocument, Conversion, DashStyle, ElbowDirection, Element, HorizontalAlign,
    ImageCrop, ImageElement, ImageFit, LineElement, LineType, Presentation, ShapeElement, Slide,
    SlideResult, SlideTemplateError, TextElement, TextRun, VerticalAlign,
};

const DRAWING_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const EMUS_PER_PIXEL: f64 = 9_525.0;
const MAX_ENTRY_BYTES: u64 = 50 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 250 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const SLIDE_BOUNDS_TOLERANCE_PX: f64 = 1.0;

/// Checks the non-ZIP64 central directory before any package entry is expanded.
/// The same validation runs for imported decks and insertion targets.
pub(super) fn validate_package(source: &[u8]) -> SlideResult<()> {
    fn u16_at(source: &[u8], offset: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            source.get(offset..offset + 2)?.try_into().ok()?,
        ))
    }
    fn u32_at(source: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            source.get(offset..offset + 4)?.try_into().ok()?,
        ))
    }
    let start = source.len().saturating_sub(65_535 + 22);
    let end = source
        .len()
        .checked_sub(22)
        .ok_or_else(invalid_powerpoint)?;
    let directory_end = (start..=end)
        .rev()
        .find(|offset| u32_at(source, *offset) == Some(0x0605_4b50))
        .ok_or_else(invalid_powerpoint)?;
    let disk = u16_at(source, directory_end + 4).ok_or_else(invalid_powerpoint)?;
    let directory_disk = u16_at(source, directory_end + 6).ok_or_else(invalid_powerpoint)?;
    let count = u16_at(source, directory_end + 10).ok_or_else(invalid_powerpoint)?;
    let directory_size = u32_at(source, directory_end + 12).ok_or_else(invalid_powerpoint)?;
    let directory_offset = u32_at(source, directory_end + 16).ok_or_else(invalid_powerpoint)?;
    if disk != 0
        || directory_disk != 0
        || count == u16::MAX
        || usize::from(count) > MAX_ENTRIES
        || directory_size == u32::MAX
        || directory_offset == u32::MAX
        || u64::from(directory_offset) + u64::from(directory_size) > directory_end as u64
    {
        return Err(invalid_powerpoint());
    }
    let mut offset = directory_offset as usize;
    let mut total = 0_u64;
    let mut required = std::collections::HashSet::from([
        "[Content_Types].xml",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
    ]);
    for _ in 0..count {
        if offset + 46 > source.len() || u32_at(source, offset) != Some(0x0201_4b50) {
            return Err(invalid_powerpoint());
        }
        let entry_bytes = u32_at(source, offset + 24).ok_or_else(invalid_powerpoint)?;
        let name_length = usize::from(u16_at(source, offset + 28).ok_or_else(invalid_powerpoint)?);
        let extra_length = usize::from(u16_at(source, offset + 30).ok_or_else(invalid_powerpoint)?);
        let comment_length =
            usize::from(u16_at(source, offset + 32).ok_or_else(invalid_powerpoint)?);
        let next = offset + 46 + name_length + extra_length + comment_length;
        if entry_bytes == u32::MAX || next > source.len() {
            return Err(invalid_powerpoint());
        }
        total += u64::from(entry_bytes);
        if u64::from(entry_bytes) > MAX_ENTRY_BYTES || total > MAX_EXPANDED_BYTES {
            return Err(powerpoint_too_large());
        }
        let name = String::from_utf8_lossy(&source[offset + 46..offset + 46 + name_length]);
        if !safe_part_name(&name) {
            return Err(invalid_powerpoint());
        }
        required.remove(name.as_ref());
        offset = next;
    }
    if offset != directory_offset as usize + directory_size as usize || !required.is_empty() {
        return Err(invalid_powerpoint());
    }
    Ok(())
}

pub(super) fn read(source: &[u8]) -> SlideResult<Conversion> {
    let parts = read_package(source)?;
    let presentation_xml = utf8_part(&parts, "ppt/presentation.xml")?;
    let presentation_rels = utf8_part(&parts, "ppt/_rels/presentation.xml.rels")?;
    let presentation = parse_xml(presentation_xml)?;
    let presentation_relationships = relationships(presentation_rels, "ppt/presentation.xml")?;
    let (width, height) = presentation
        .descendants()
        .find(|node| node.has_tag_name((PRESENTATION_NS, "sldSz")))
        .map(|node| {
            (
                emu_attribute(node, "cx", 12_192_000) / EMUS_PER_PIXEL,
                emu_attribute(node, "cy", 6_858_000) / EMUS_PER_PIXEL,
            )
        })
        .unwrap_or((1280.0, 720.0));
    let slide_paths = presentation
        .descendants()
        .filter(|node| node.has_tag_name((PRESENTATION_NS, "sldId")))
        .filter_map(|node| node.attribute((RELATIONSHIP_NS, "id")))
        .filter_map(|id| presentation_relationships.get(id))
        .cloned()
        .collect::<Vec<_>>();
    if slide_paths.is_empty() {
        return Err(invalid_powerpoint());
    }

    let mut warnings = Vec::new();
    let mut slides = Vec::with_capacity(slide_paths.len());
    for (index, slide_path) in slide_paths.iter().enumerate() {
        let slide_xml = utf8_part(&parts, slide_path)?;
        let rels_path = relationship_part_path(slide_path);
        let slide_relationships = match parts.get(&rels_path) {
            Some(bytes) => relationships(
                std::str::from_utf8(bytes).map_err(|_| invalid_powerpoint())?,
                slide_path,
            )?,
            None => HashMap::new(),
        };
        let theme_colors = ThemeColors::for_slide(&parts, &slide_relationships);
        let text_styles =
            InheritedTextStyles::for_slide(&parts, &slide_relationships, &theme_colors);
        let warning_start = warnings.len();
        let mut context = SlideParseContext {
            parts: &parts,
            relationships: &slide_relationships,
            slide_index: index,
            source_path: slide_path,
            theme_colors: &theme_colors,
            text_styles: &text_styles,
            warnings: &mut warnings,
            inherited: false,
        };
        let mut slide = parse_slide(slide_xml, (width, height), &mut context)?;
        let slide_document = parse_xml(slide_xml)?;
        if !slide_document
            .descendants()
            .any(|node| node.has_tag_name((PRESENTATION_NS, "bg")))
        {
            if let Some(color) =
                inherited_background_color(&parts, &slide_relationships, &theme_colors)
            {
                slide.background_color = color;
            }
        }
        let local_warnings = context.warnings.split_off(warning_start);
        let mut inherited = inherited_drawables(&mut context, slide_xml, (width, height))?;
        context.warnings.extend(local_warnings);
        inherited.append(&mut slide.elements);
        slide.elements = inherited;
        if slide.elements.is_empty() {
            context.warnings.push(format!("slide {}: shapeTree.elements: No exportable slide elements were found. The PowerPoint will be empty.", index + 1));
        }
        slide.name = slide_title(&slide.elements, index);
        slides.push(slide);
    }
    let title = slides
        .first()
        .map_or_else(|| "Slide 1".to_owned(), |slide| slide.name.clone());
    Ok(Conversion {
        template_json: CanvasDocument {
            presentation: Presentation {
                title,
                preserve_element_order: true,
                show_branding: false,
                slides,
            },
        },
        warnings,
    })
}

#[derive(Default)]
struct ThemeColors {
    values: HashMap<String, String>,
    major_font: Option<String>,
    minor_font: Option<String>,
    line_widths: Vec<f64>,
}

impl ThemeColors {
    fn for_slide(
        parts: &HashMap<String, Vec<u8>>,
        slide_relationships: &HashMap<String, String>,
    ) -> Self {
        let theme_path = relationship_target_with_segment(slide_relationships, "/theme/")
            .map(str::to_owned)
            .or_else(|| {
                let layout_path =
                    relationship_target_with_segment(slide_relationships, "/slideLayouts/")?;
                let layout_relationships = part_relationships(parts, layout_path)?;
                relationship_target_with_segment(&layout_relationships, "/theme/")
                    .map(str::to_owned)
                    .or_else(|| {
                        let master_path = relationship_target_with_segment(
                            &layout_relationships,
                            "/slideMasters/",
                        )?;
                        let master_relationships = part_relationships(parts, master_path)?;
                        relationship_target_with_segment(&master_relationships, "/theme/")
                            .map(str::to_owned)
                    })
            })
            .or_else(|| {
                parts
                    .keys()
                    .find(|path| path.starts_with("ppt/theme/") && path.ends_with(".xml"))
                    .cloned()
            });

        theme_path
            .as_deref()
            .and_then(|path| parts.get(path))
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map_or_else(Self::default, Self::from_xml)
    }

    fn from_xml(xml: &str) -> Self {
        let Ok(document) = Document::parse(xml) else {
            return Self::default();
        };
        let values = document
            .descendants()
            .find(|node| node.tag_name().name() == "clrScheme")
            .map(|color_scheme| {
                color_scheme
                    .children()
                    .filter(Node::is_element)
                    .filter_map(|slot| {
                        let value =
                            slot.descendants()
                                .find_map(|color| match color.tag_name().name() {
                                    "srgbClr" => color.attribute("val").map(clean_color),
                                    "sysClr" => color.attribute("lastClr").map(clean_color),
                                    _ => None,
                                })?;
                        Some((slot.tag_name().name().to_owned(), value))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let theme_font = |name| {
            document
                .descendants()
                .find(|node| node.tag_name().name() == name)
                .and_then(|node| {
                    node.children()
                        .find(|child| child.tag_name().name() == "latin")
                })
                .and_then(|node| node.attribute("typeface"))
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        };
        Self {
            values,
            major_font: theme_font("majorFont"),
            minor_font: theme_font("minorFont"),
            line_widths: document
                .descendants()
                .find(|node| node.tag_name().name() == "lnStyleLst")
                .map(|line_styles| {
                    line_styles
                        .children()
                        .filter(|node| node.tag_name().name() == "ln")
                        .map(|line| {
                            line.attribute("w")
                                .and_then(|value| value.parse::<f64>().ok())
                                .map_or(1.0, |value| value / 12_700.0)
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    fn resolve(&self, value: &str) -> String {
        let slot = match value {
            "tx1" => "dk1",
            "bg1" => "lt1",
            "tx2" => "dk2",
            "bg2" => "lt2",
            value => value,
        };
        self.values
            .get(slot)
            .cloned()
            .unwrap_or_else(|| scheme_color(value).to_owned())
    }

    fn resolve_font(&self, value: &str) -> String {
        match value {
            "+mj-lt" | "+mj-ea" | "+mj-cs" => self.major_font.as_deref().unwrap_or("Arial"),
            "+mn-lt" | "+mn-ea" | "+mn-cs" => self.minor_font.as_deref().unwrap_or("Arial"),
            value => value,
        }
        .to_owned()
    }

    fn line_width(&self, style_index: usize) -> Option<f64> {
        style_index
            .checked_sub(1)
            .and_then(|index| self.line_widths.get(index))
            .copied()
    }
}

#[derive(Clone, Default)]
struct RunStyle {
    color: Option<String>,
    font_face: Option<String>,
    font_size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
}

impl RunStyle {
    fn from_node(node: Option<Node<'_, '_>>, theme_colors: &ThemeColors) -> Self {
        let Some(node) = node else {
            return Self::default();
        };
        let font_face = node
            .children()
            .find(|child| matches!(child.tag_name().name(), "latin" | "ea" | "cs"))
            .and_then(|font| font.attribute("typeface"))
            .filter(|font| !font.is_empty())
            .map(|font| theme_colors.resolve_font(font));
        let color = node
            .children()
            .find(|child| child.tag_name().name() == "solidFill")
            .and_then(|fill| descendant_color(fill, theme_colors));
        Self {
            color,
            font_face,
            font_size: node
                .attribute("sz")
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| *value > 0.0)
                .map(|value| value / 100.0),
            bold: boolean_attribute(node, "b"),
            italic: boolean_attribute(node, "i"),
            underline: node.attribute("u").map(|value| value != "none"),
        }
    }

    fn from_shape_style(node: Node<'_, '_>, theme_colors: &ThemeColors) -> Self {
        let font_reference = node
            .children()
            .find(|child| child.tag_name().name() == "style")
            .and_then(|style| {
                style
                    .children()
                    .find(|child| child.tag_name().name() == "fontRef")
            });
        let Some(font_reference) = font_reference else {
            return Self::default();
        };
        let font_face = match font_reference.attribute("idx") {
            Some("major") => theme_colors.major_font.clone(),
            Some("minor") => theme_colors.minor_font.clone(),
            _ => None,
        };
        Self {
            color: descendant_color(font_reference, theme_colors),
            font_face,
            ..Self::default()
        }
    }

    fn overlay(&mut self, other: &Self) {
        if other.color.is_some() {
            self.color.clone_from(&other.color);
        }
        if other.font_face.is_some() {
            self.font_face.clone_from(&other.font_face);
        }
        if other.font_size.is_some() {
            self.font_size = other.font_size;
        }
        if other.bold.is_some() {
            self.bold = other.bold;
        }
        if other.italic.is_some() {
            self.italic = other.italic;
        }
        if other.underline.is_some() {
            self.underline = other.underline;
        }
    }

    fn text_run(&self, text: String) -> TextRun {
        TextRun {
            text,
            color: self.color.clone().unwrap_or_else(|| "111827".to_owned()),
            font_face: self.font_face.clone().unwrap_or_else(|| "Arial".to_owned()),
            font_size: self.font_size.unwrap_or(16.0),
            bold: self.bold.unwrap_or(false),
            italic: self.italic.unwrap_or(false),
            underline: self.underline.unwrap_or(false),
            break_line: false,
        }
    }
}

#[derive(Clone)]
enum BulletStyle {
    None,
    Character(String),
    Automatic { kind: String, start_at: usize },
}

#[derive(Clone, Default)]
struct ParagraphStyle {
    run: RunStyle,
    bullet: Option<BulletStyle>,
    align: Option<HorizontalAlign>,
}

impl ParagraphStyle {
    fn from_node(node: Option<Node<'_, '_>>, theme_colors: &ThemeColors) -> Self {
        let Some(node) = node else {
            return Self::default();
        };
        let bullet = if node
            .children()
            .any(|child| child.tag_name().name() == "buNone")
        {
            Some(BulletStyle::None)
        } else if let Some(character) = node
            .children()
            .find(|child| child.tag_name().name() == "buChar")
            .and_then(|bullet| bullet.attribute("char"))
        {
            Some(BulletStyle::Character(character.to_owned()))
        } else {
            node.children()
                .find(|child| child.tag_name().name() == "buAutoNum")
                .map(|automatic| BulletStyle::Automatic {
                    kind: automatic
                        .attribute("type")
                        .unwrap_or("arabicPeriod")
                        .to_owned(),
                    start_at: usize_attribute(automatic, "startAt", 1).max(1),
                })
        };
        Self {
            run: RunStyle::from_node(
                node.children()
                    .find(|child| child.tag_name().name() == "defRPr"),
                theme_colors,
            ),
            bullet,
            align: node.attribute("algn").map(parse_align),
        }
    }

    fn overlay(&mut self, other: &Self) {
        self.run.overlay(&other.run);
        if other.align.is_some() {
            self.align = other.align;
        }
        if other.bullet.is_some() {
            self.bullet.clone_from(&other.bullet);
        }
    }
}

struct TextStyleLevels {
    levels: [ParagraphStyle; 9],
}

#[derive(Clone, Default)]
struct TextBodyStyle {
    valign: Option<VerticalAlign>,
    padding: Option<f64>,
}

impl TextBodyStyle {
    fn from_node(node: Option<Node<'_, '_>>) -> Self {
        let Some(node) = node else {
            return Self::default();
        };
        Self {
            valign: node.attribute("anchor").map(parse_valign),
            padding: node
                .attribute("lIns")
                .and_then(|value| value.parse::<f64>().ok())
                .map(|value| value / 12_700.0),
        }
    }

    fn overlay(&mut self, other: &Self) {
        if other.valign.is_some() {
            self.valign = other.valign;
        }
        if other.padding.is_some() {
            self.padding = other.padding;
        }
    }
}

impl Default for TextStyleLevels {
    fn default() -> Self {
        Self {
            levels: std::array::from_fn(|_| ParagraphStyle::default()),
        }
    }
}

impl TextStyleLevels {
    fn from_node(node: Option<Node<'_, '_>>, theme_colors: &ThemeColors) -> Self {
        let Some(node) = node else {
            return Self::default();
        };
        Self {
            levels: std::array::from_fn(|level| {
                let tag = format!("lvl{}pPr", level + 1);
                ParagraphStyle::from_node(
                    node.descendants()
                        .find(|child| child.tag_name().name() == tag),
                    theme_colors,
                )
            }),
        }
    }

    fn level(&self, level: usize) -> &ParagraphStyle {
        &self.levels[level.min(self.levels.len() - 1)]
    }
}

#[derive(Clone)]
struct PlaceholderKey {
    kind: String,
    index: Option<String>,
}

#[derive(Clone, Default)]
struct ShapeDefaults {
    geometry: Option<Geometry>,
    fill: Option<(String, f64)>,
    line: Option<(String, f64, f64)>,
    preset: Option<String>,
    font: RunStyle,
}

impl ShapeDefaults {
    fn from_node(node: Node<'_, '_>, theme: &ThemeColors) -> Self {
        let properties = node
            .children()
            .find(|child| child.tag_name().name() == "spPr");
        let style = node
            .children()
            .find(|child| child.tag_name().name() == "style");
        let fill_reference = style.and_then(|style| {
            style
                .children()
                .find(|child| child.tag_name().name() == "fillRef")
        });
        let line_reference = style
            .and_then(|style| {
                style
                    .children()
                    .find(|child| child.tag_name().name() == "lnRef")
            })
            .filter(|reference| reference.attribute("idx") != Some("0"));
        let fill = properties
            .and_then(|properties| fill_option(properties, theme))
            .or_else(|| {
                let reference = fill_reference?;
                Some((
                    descendant_color(reference, theme)?,
                    color_opacity(reference),
                ))
            });
        let line = if let Some(properties) = properties.filter(|properties| {
            properties
                .children()
                .any(|child| child.tag_name().name() == "ln")
        }) {
            Some(line_style(properties, theme))
        } else {
            line_reference.and_then(|reference| {
                Some((
                    descendant_color(reference, theme)?,
                    1.0,
                    color_opacity(reference),
                ))
            })
        };
        Self {
            geometry: properties
                .filter(|properties| {
                    properties
                        .children()
                        .any(|child| child.tag_name().name() == "xfrm")
                })
                .map(|_| geometry(node)),
            fill,
            line,
            preset: shape_preset(node).map(str::to_owned),
            font: RunStyle::from_shape_style(node, theme),
        }
    }

    fn overlay(&mut self, other: &Self) {
        if other.geometry.is_some() {
            self.geometry = other.geometry;
        }
        if other.fill.is_some() {
            self.fill.clone_from(&other.fill);
        }
        if other.line.is_some() {
            self.line.clone_from(&other.line);
        }
        if other.preset.is_some() {
            self.preset.clone_from(&other.preset);
        }
        self.font.overlay(&other.font);
    }
}

struct PlaceholderTextStyles {
    key: PlaceholderKey,
    levels: TextStyleLevels,
    body: TextBodyStyle,
    shape: ShapeDefaults,
}

#[derive(Default)]
struct InheritedTextStyles {
    layout: Vec<PlaceholderTextStyles>,
    master: Vec<PlaceholderTextStyles>,
    title: TextStyleLevels,
    body: TextStyleLevels,
    other: TextStyleLevels,
}

impl InheritedTextStyles {
    fn for_slide(
        parts: &HashMap<String, Vec<u8>>,
        slide_relationships: &HashMap<String, String>,
        theme_colors: &ThemeColors,
    ) -> Self {
        let Some(layout_path) =
            relationship_target_with_segment(slide_relationships, "/slideLayouts/")
        else {
            return Self::default();
        };
        let layout = parts
            .get(layout_path)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|xml| Document::parse(xml).ok());
        let layout_relationships = part_relationships(parts, layout_path).unwrap_or_default();
        let master_path = relationship_target_with_segment(&layout_relationships, "/slideMasters/");
        let master = master_path
            .and_then(|path| parts.get(path))
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|xml| Document::parse(xml).ok());

        Self {
            layout: layout.as_ref().map_or_else(Vec::new, |document| {
                placeholder_styles(document, theme_colors)
            }),
            master: master.as_ref().map_or_else(Vec::new, |document| {
                placeholder_styles(document, theme_colors)
            }),
            title: TextStyleLevels::from_node(
                master.as_ref().and_then(|document| {
                    document
                        .descendants()
                        .find(|node| node.tag_name().name() == "titleStyle")
                }),
                theme_colors,
            ),
            body: TextStyleLevels::from_node(
                master.as_ref().and_then(|document| {
                    document
                        .descendants()
                        .find(|node| node.tag_name().name() == "bodyStyle")
                }),
                theme_colors,
            ),
            other: TextStyleLevels::from_node(
                master.as_ref().and_then(|document| {
                    document
                        .descendants()
                        .find(|node| node.tag_name().name() == "otherStyle")
                }),
                theme_colors,
            ),
        }
    }

    fn shape_style(&self, node: Node<'_, '_>, theme: &ThemeColors) -> ShapeDefaults {
        let mut shape = ShapeDefaults::default();
        if let Some(key) = placeholder_key(node) {
            if let Some(master) = matching_placeholder(&self.master, &key) {
                shape.overlay(&master.shape);
            }
            if let Some(layout) = matching_placeholder(&self.layout, &key) {
                shape.overlay(&layout.shape);
            }
        }
        shape.overlay(&ShapeDefaults::from_node(node, theme));
        shape
    }

    fn paragraph_style(&self, node: Node<'_, '_>, level: usize) -> ParagraphStyle {
        let Some(key) = placeholder_key(node) else {
            return ParagraphStyle::default();
        };
        let mut style = match key.kind.as_str() {
            "title" | "ctrTitle" => self.title.level(level).clone(),
            "body" | "subTitle" | "obj" => self.body.level(level).clone(),
            _ => self.other.level(level).clone(),
        };
        if let Some(master) = matching_placeholder(&self.master, &key) {
            style.overlay(master.levels.level(level));
        }
        if let Some(layout) = matching_placeholder(&self.layout, &key) {
            style.overlay(layout.levels.level(level));
        }
        style
    }

    fn body_style(&self, node: Node<'_, '_>) -> TextBodyStyle {
        let Some(key) = placeholder_key(node) else {
            return TextBodyStyle::default();
        };
        let mut style = TextBodyStyle::default();
        if let Some(master) = matching_placeholder(&self.master, &key) {
            style.overlay(&master.body);
        }
        if let Some(layout) = matching_placeholder(&self.layout, &key) {
            style.overlay(&layout.body);
        }
        style
    }
}

fn placeholder_styles(
    document: &Document<'_>,
    theme_colors: &ThemeColors,
) -> Vec<PlaceholderTextStyles> {
    document
        .descendants()
        .filter(|node| node.tag_name().name() == "sp")
        .filter_map(|shape| {
            let text_body = shape
                .children()
                .find(|child| child.tag_name().name() == "txBody");
            Some(PlaceholderTextStyles {
                key: placeholder_key(shape)?,
                shape: ShapeDefaults::from_node(shape, theme_colors),
                levels: TextStyleLevels::from_node(text_body, theme_colors),
                body: TextBodyStyle::from_node(text_body.and_then(|text_body| {
                    text_body
                        .children()
                        .find(|child| child.tag_name().name() == "bodyPr")
                })),
            })
        })
        .collect()
}

fn placeholder_key(node: Node<'_, '_>) -> Option<PlaceholderKey> {
    let placeholder = node
        .descendants()
        .find(|child| child.tag_name().name() == "ph")?;
    Some(PlaceholderKey {
        kind: placeholder.attribute("type").unwrap_or("body").to_owned(),
        index: placeholder.attribute("idx").map(str::to_owned),
    })
}

fn matching_placeholder<'a>(
    styles: &'a [PlaceholderTextStyles],
    key: &PlaceholderKey,
) -> Option<&'a PlaceholderTextStyles> {
    styles
        .iter()
        .find(|candidate| match (&key.index, &candidate.key.index) {
            (Some(expected), Some(actual)) => expected == actual,
            _ => candidate.key.kind == key.kind,
        })
}

fn relationship_target_with_segment<'a>(
    relationships: &'a HashMap<String, String>,
    segment: &str,
) -> Option<&'a str> {
    relationships
        .values()
        .find(|path| path.contains(segment))
        .map(String::as_str)
}

fn part_relationships(
    parts: &HashMap<String, Vec<u8>>,
    part_path: &str,
) -> Option<HashMap<String, String>> {
    let bytes = parts.get(&relationship_part_path(part_path))?;
    let xml = std::str::from_utf8(bytes).ok()?;
    relationships(xml, part_path).ok()
}

fn background_image_from_part(
    parts: &HashMap<String, Vec<u8>>,
    xml: &str,
    relationships: &HashMap<String, String>,
    dimensions: (f64, f64),
    alt_text: &str,
) -> SlideResult<Option<ImageElement>> {
    let document = parse_xml(xml)?;
    let relationship_id = document
        .descendants()
        .find(|node| node.has_tag_name((PRESENTATION_NS, "bg")))
        .and_then(|background| {
            background
                .descendants()
                .find(|node| node.tag_name().name() == "blip")
        })
        .and_then(|blip| blip.attribute((RELATIONSHIP_NS, "embed")));
    let Some(part_path) = relationship_id.and_then(|id| relationships.get(id)) else {
        return Ok(None);
    };
    let Some(bytes) = parts.get(part_path) else {
        return Ok(None);
    };
    let Some(content_type) = image_content_type(part_path) else {
        return Ok(None);
    };
    let (width, height) = dimensions;
    Ok(Some(ImageElement {
        id: format!(
            "element-{}",
            match alt_text {
                "Master Background Image" => 904_000,
                "Layout Background Image" => 903_000,
                _ => 902_500,
            }
        ),
        x: 0.0,
        y: 0.0,
        w: width,
        h: height,
        src: format!("data:{content_type};base64,{}", STANDARD.encode(bytes)),
        fit: ImageFit::Stretch,
        crop: None,
        alt_text: alt_text.to_owned(),
        opacity: 1.0,
        rotate: 0.0,
        flip_h: false,
        flip_v: false,
        border_radius: 0.0,
    }))
}

fn read_package(source: &[u8]) -> SlideResult<HashMap<String, Vec<u8>>> {
    validate_package(source)?;
    let mut archive = ZipArchive::new(Cursor::new(source)).map_err(|_| invalid_powerpoint())?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(invalid_powerpoint());
    }
    let mut total = 0_u64;
    let mut parts = HashMap::with_capacity(archive.len());
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| invalid_powerpoint())?;
        let name = entry.name().to_owned();
        if !safe_part_name(&name) || entry.size() > MAX_ENTRY_BYTES {
            return Err(if entry.size() > MAX_ENTRY_BYTES {
                powerpoint_too_large()
            } else {
                invalid_powerpoint()
            });
        }
        total = total.saturating_add(entry.size());
        if total > MAX_EXPANDED_BYTES {
            return Err(powerpoint_too_large());
        }
        if entry.is_dir() {
            continue;
        }
        let capacity = usize::try_from(entry.size()).map_err(|_| powerpoint_too_large())?;
        let mut bytes = Vec::with_capacity(capacity);
        entry
            .read_to_end(&mut bytes)
            .map_err(|_| invalid_powerpoint())?;
        parts.insert(name, bytes);
    }
    for required in [
        "[Content_Types].xml",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
    ] {
        if !parts.contains_key(required) {
            return Err(invalid_powerpoint());
        }
    }
    Ok(parts)
}

struct SlideParseContext<'a> {
    parts: &'a HashMap<String, Vec<u8>>,
    relationships: &'a HashMap<String, String>,
    slide_index: usize,
    source_path: &'a str,
    theme_colors: &'a ThemeColors,
    text_styles: &'a InheritedTextStyles,
    warnings: &'a mut Vec<String>,
    inherited: bool,
}

fn parse_slide(
    xml: &str,
    dimensions: (f64, f64),
    context: &mut SlideParseContext<'_>,
) -> SlideResult<Slide> {
    let (width, height) = dimensions;
    let document = parse_xml(xml)?;
    let background_color = document
        .descendants()
        .find(|node| node.has_tag_name((PRESENTATION_NS, "bg")))
        .and_then(|node| descendant_color(node, context.theme_colors))
        .unwrap_or_else(|| "FFFFFF".to_owned());
    let shape_tree = document
        .descendants()
        .find(|node| node.has_tag_name((PRESENTATION_NS, "spTree")));
    let mut elements = Vec::new();
    for child in shape_tree
        .into_iter()
        .flat_map(|tree| tree.children())
        .filter(Node::is_element)
    {
        if should_skip_drawable(child, context.inherited) || is_inherited_metafile(context, child) {
            continue;
        }
        if child.tag_name().name() == "sp" {
            warn_shape_fallbacks(context, child);
        }
        match child.tag_name().name() {
            "sp" => elements.push(parse_shape(
                child,
                elements.len() + 1,
                context.theme_colors,
                context.text_styles,
            )),
            "cxnSp" => elements.push(Element::Line(parse_line(
                child,
                elements.len() + 1,
                context.theme_colors,
            ))),
            "pic" => {
                if let Some(image) = parse_image(
                    context.parts,
                    context.relationships,
                    child,
                    elements.len() + 1,
                )? {
                    elements.push(Element::Image(image));
                } else {
                    warn_unsupported_graphic(context, child);
                }
            }
            "graphicFrame" => {
                if child
                    .descendants()
                    .any(|node| node.tag_name().name() == "tbl")
                {
                    elements.extend(parse_table(
                        child,
                        elements.len() + 1,
                        context.theme_colors,
                        context.text_styles,
                    ));
                } else {
                    if let Some(image) = parse_image(
                        context.parts,
                        context.relationships,
                        child,
                        elements.len() + 1,
                    )? {
                        elements.push(Element::Image(image));
                    } else {
                        warn_unsupported_graphic(context, child);
                    }
                }
            }
            "grpSp" => {
                elements.extend(parse_group(context, child, elements.len() + 1)?);
            }
            _ => {}
        }
    }
    if context.inherited {
        elements.retain(|element| !is_completely_outside_slide(element, width, height));
    }
    for element in &mut elements {
        if let Element::Line(line) = element {
            line.x1 = line.x1.clamp(0.0, width);
            line.y1 = line.y1.clamp(0.0, height);
            line.x2 = line.x2.clamp(0.0, width);
            line.y2 = line.y2.clamp(0.0, height);
        }
    }
    let name = slide_title(&elements, context.slide_index);
    Ok(Slide {
        id: format!("slide-{}", context.slide_index + 1),
        name,
        width,
        height,
        background_color,
        elements,
    })
}

fn drawable_path(context: &SlideParseContext<'_>, node: Node<'_, '_>) -> String {
    let mut nodes = node
        .ancestors()
        .filter(|ancestor| ancestor.tag_name().name() == "grpSp")
        .collect::<Vec<_>>();
    nodes.reverse();
    nodes.push(node);
    let mut path = context.source_path.to_owned();
    if context.inherited {
        path.push_str(if context.source_path.contains("/slideMasters/") {
            "#master"
        } else {
            "#layout"
        });
    }
    for node in nodes {
        let index = node
            .parent()
            .and_then(|parent| {
                parent
                    .children()
                    .filter(Node::is_element)
                    .position(|child| child == node)
            })
            .unwrap_or(0)
            + 1;
        path.push_str(&format!("#p:{}[{index}]", node.tag_name().name()));
    }
    path
}

fn warn_unsupported_graphic(context: &mut SlideParseContext<'_>, node: Node<'_, '_>) {
    context.warnings.push(format!("slide {}: {}: Skipped a graphic frame because it is not a plain PowerPoint drawable element.", context.slide_index + 1, drawable_path(context, node)));
}

fn warn_shape_fallbacks(context: &mut SlideParseContext<'_>, node: Node<'_, '_>) {
    let properties = node
        .children()
        .find(|child| child.tag_name().name() == "spPr");
    let Some(properties) = properties else { return };
    for (tags, message) in [
        (
            &["custGeom"][..],
            "Rendered a custom/freeform PowerPoint geometry as a rectangle fallback.",
        ),
        (
            &["gradFill", "pattFill", "blipFill"][..],
            "This shape uses a gradient, pattern, or picture fill; the compact canvas schema uses a solid-color fallback.",
        ),
    ] {
        if properties
            .children()
            .any(|child| tags.contains(&child.tag_name().name()))
        {
            context.warnings.push(format!(
                "slide {}: {}: {message}",
                context.slide_index + 1,
                drawable_path(context, node)
            ));
        }
    }
}

fn is_inherited_metafile(context: &SlideParseContext<'_>, node: Node<'_, '_>) -> bool {
    context.inherited
        && node.tag_name().name() != "grpSp"
        && node
            .descendants()
            .filter(|node| node.tag_name().name() == "blip")
            .filter_map(|node| {
                node.attribute((RELATIONSHIP_NS, "embed"))
                    .or_else(|| node.attribute((RELATIONSHIP_NS, "link")))
            })
            .filter_map(|id| context.relationships.get(id))
            .any(|path| {
                matches!(
                    path.rsplit('.')
                        .next()
                        .map(str::to_ascii_lowercase)
                        .as_deref(),
                    Some("emf" | "wmf")
                )
            })
}

fn should_skip_drawable(node: Node<'_, '_>, inherited: bool) -> bool {
    if non_visual(node, "hidden").is_some_and(|value| is_true(&value)) {
        return true;
    }
    if !inherited {
        return false;
    }
    if placeholder_key(node).is_some() && node.tag_name().name() != "grpSp" {
        return true;
    }
    let text = node
        .descendants()
        .filter(|child| child.tag_name().name() == "t")
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    [
        "click to edit master",
        "edit master text styles",
        "master title style",
    ]
    .iter()
    .any(|phrase| text.contains(phrase))
}

fn slide_title(elements: &[Element], slide_index: usize) -> String {
    elements
        .iter()
        .filter_map(|element| match element {
            Element::Text(text) if !text.text.trim().is_empty() => Some((text.y, &text.text)),
            Element::Shape(shape) if !shape.text.trim().is_empty() => Some((shape.y, &shape.text)),
            _ => None,
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, text)| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| format!("Slide {}", slide_index + 1))
}

fn inherited_background_color(
    parts: &HashMap<String, Vec<u8>>,
    relationships: &HashMap<String, String>,
    theme: &ThemeColors,
) -> Option<String> {
    let layout = relationship_target_with_segment(relationships, "/slideLayouts/")?;
    let layout_relationships = part_relationships(parts, layout).unwrap_or_default();
    for path in [
        Some(layout),
        relationship_target_with_segment(&layout_relationships, "/slideMasters/"),
    ]
    .into_iter()
    .flatten()
    {
        let Some(document) = parts
            .get(path)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(|xml| Document::parse(xml).ok())
        else {
            continue;
        };
        if let Some(color) = document
            .descendants()
            .find(|node| node.has_tag_name((PRESENTATION_NS, "bg")))
            .and_then(|node| descendant_color(node, theme))
        {
            return Some(color);
        }
    }
    None
}

fn inherited_drawables(
    context: &mut SlideParseContext<'_>,
    slide_xml: &str,
    dimensions: (f64, f64),
) -> SlideResult<Vec<Element>> {
    let mut layers = Vec::<(i32, Element)>::new();
    let slide_document = parse_xml(slide_xml)?;
    let show_master = slide_document.root_element().attribute("showMasterSp") != Some("0");
    let layout_path = relationship_target_with_segment(context.relationships, "/slideLayouts/");
    let layout_relationships = layout_path
        .and_then(|path| part_relationships(context.parts, path))
        .unwrap_or_default();
    let layout_xml = layout_path
        .and_then(|path| context.parts.get(path))
        .and_then(|bytes| std::str::from_utf8(bytes).ok());
    let layout_show_master = layout_xml
        .and_then(|xml| Document::parse(xml).ok())
        .is_none_or(|document| document.root_element().attribute("showMasterSp") != Some("0"));
    let master_path = relationship_target_with_segment(&layout_relationships, "/slideMasters/");
    for (part_path, label, z) in [
        (
            master_path.filter(|_| show_master && layout_show_master),
            "Master",
            -3000,
        ),
        (layout_path, "Layout", -2000),
    ] {
        let Some(part_path) = part_path else { continue };
        let Some(bytes) = context.parts.get(part_path) else {
            continue;
        };
        let xml = std::str::from_utf8(bytes).map_err(|_| invalid_powerpoint())?;
        let relationships = part_relationships(context.parts, part_path).unwrap_or_default();
        if let Some(image) = background_image_from_part(
            context.parts,
            xml,
            &relationships,
            dimensions,
            &format!("{label} Background Image"),
        )?
        .filter(|image| {
            !image.src.starts_with("data:image/emf;") && !image.src.starts_with("data:image/wmf;")
        }) {
            layers.push((z - 1000, Element::Image(image)));
        }
        let mut inherited_context = SlideParseContext {
            parts: context.parts,
            relationships: &relationships,
            slide_index: context.slide_index,
            source_path: part_path,
            theme_colors: context.theme_colors,
            text_styles: context.text_styles,
            warnings: context.warnings,
            inherited: true,
        };
        if parse_xml(xml)?
            .descendants()
            .any(|node| node.has_tag_name((PRESENTATION_NS, "spTree")))
        {
            let related = parse_slide(xml, dimensions, &mut inherited_context)?;
            for element in related.elements {
                if matches!(&element, Element::Image(image) if image.src.starts_with("data:image/emf;") || image.src.starts_with("data:image/wmf;"))
                {
                    continue;
                }
                layers.push((z, element));
            }
        }
    }
    if let Some(image) = background_image_from_part(
        context.parts,
        slide_xml,
        context.relationships,
        dimensions,
        "Slide Background Image",
    )? {
        layers.push((-2500, Element::Image(image)));
    }
    layers.sort_by_key(|(z, _)| *z);
    Ok(layers.into_iter().map(|(_, element)| element).collect())
}

fn shape_border_radius(node: Node<'_, '_>, shape: &str, width: f64, height: f64) -> f64 {
    if shape != "roundRect" {
        return 0.0;
    }
    let ratio = node
        .descendants()
        .find(|child| child.tag_name().name() == "gd")
        .and_then(|guide| guide.attribute("fmla"))
        .and_then(|value| value.strip_prefix("val "))
        .and_then(|value| value.parse::<f64>().ok())
        .map_or(1.0 / 6.0, |value| (value / 100_000.0).clamp(0.0, 0.5));
    width.min(height) * ratio
}

fn parse_group(
    context: &mut SlideParseContext<'_>,
    node: Node<'_, '_>,
    starting_id: usize,
) -> SlideResult<Vec<Element>> {
    parse_group_with_transform(context, node, starting_id, TransformMatrix::IDENTITY)
}

fn parse_group_with_transform(
    context: &mut SlideParseContext<'_>,
    node: Node<'_, '_>,
    starting_id: usize,
    parent: TransformMatrix,
) -> SlideResult<Vec<Element>> {
    let transform = parent.compose(GroupTransform::from_node(node).matrix());
    let mut elements = Vec::new();
    for child in node.children().filter(Node::is_element) {
        if should_skip_drawable(child, context.inherited) || is_inherited_metafile(context, child) {
            continue;
        }
        if child.tag_name().name() == "sp" {
            warn_shape_fallbacks(context, child);
        }
        let fallback_id = starting_id + elements.len();
        let start = elements.len();
        match child.tag_name().name() {
            "sp" => elements.push(parse_shape(
                child,
                fallback_id,
                context.theme_colors,
                context.text_styles,
            )),
            "cxnSp" => elements.push(Element::Line(parse_line(
                child,
                fallback_id,
                context.theme_colors,
            ))),
            "pic" => {
                if let Some(image) =
                    parse_image(context.parts, context.relationships, child, fallback_id)?
                {
                    elements.push(Element::Image(image));
                } else {
                    warn_unsupported_graphic(context, child);
                }
            }
            "graphicFrame" => {
                if child
                    .descendants()
                    .any(|descendant| descendant.tag_name().name() == "tbl")
                {
                    let mut frame = geometry(child);
                    transform.apply_rect(&mut frame);
                    elements.extend(parse_table_with_geometry(
                        child,
                        fallback_id,
                        context.theme_colors,
                        context.text_styles,
                        frame,
                    ));
                    continue;
                } else {
                    if let Some(image) =
                        parse_image(context.parts, context.relationships, child, fallback_id)?
                    {
                        elements.push(Element::Image(image));
                    } else {
                        warn_unsupported_graphic(context, child);
                    }
                }
            }
            "grpSp" => {
                elements.extend(parse_group_with_transform(
                    context,
                    child,
                    fallback_id,
                    transform,
                )?);
                continue;
            }
            _ => {}
        }
        for element in &mut elements[start..] {
            transform.apply(element);
        }
    }
    Ok(elements)
}

fn is_completely_outside_slide(element: &Element, width: f64, height: f64) -> bool {
    let (x, y, w, h) = match element {
        Element::Text(text) => (text.x, text.y, text.w, text.h),
        Element::Shape(shape) => (shape.x, shape.y, shape.w, shape.h),
        Element::Image(image) => (image.x, image.y, image.w, image.h),
        Element::Line(line) => (
            line.x1.min(line.x2),
            line.y1.min(line.y2),
            (line.x2 - line.x1).abs(),
            (line.y2 - line.y1).abs(),
        ),
    };
    x + w < -SLIDE_BOUNDS_TOLERANCE_PX
        || y + h < -SLIDE_BOUNDS_TOLERANCE_PX
        || x > width + SLIDE_BOUNDS_TOLERANCE_PX
        || y > height + SLIDE_BOUNDS_TOLERANCE_PX
}

fn parse_shape(
    node: Node<'_, '_>,
    fallback_id: usize,
    theme_colors: &ThemeColors,
    text_styles: &InheritedTextStyles,
) -> Element {
    if shape_preset(node).and_then(preset_line_type).is_some() {
        return Element::Line(parse_line(node, fallback_id, theme_colors));
    }
    let inherited_shape = text_styles.shape_style(node, theme_colors);
    let geometry = inherited_shape.geometry.unwrap_or_else(|| geometry(node));
    let id = format!(
        "element-{}",
        non_visual(node, "id").unwrap_or_else(|| fallback_id.to_string())
    );
    let name = non_visual(node, "name").unwrap_or_default();
    let runs = text_runs(node, theme_colors, text_styles);
    let text = text_from_runs(&runs);
    let shape = inherited_shape
        .preset
        .clone()
        .unwrap_or_else(|| "rect".to_owned());
    let (fill, fill_opacity) = inherited_shape
        .fill
        .unwrap_or_else(|| ("transparent".to_owned(), 1.0));
    let (stroke, stroke_width, stroke_opacity) = inherited_shape
        .line
        .unwrap_or_else(|| ("transparent".to_owned(), 0.0, 1.0));
    let body_pr = node
        .descendants()
        .find(|child| child.tag_name().name() == "bodyPr");
    let mut body_style = text_styles.body_style(node);
    body_style.overlay(&TextBodyStyle::from_node(body_pr));
    let paragraph = node
        .descendants()
        .find(|child| child.tag_name().name() == "pPr");
    let font_size = runs.first().map_or(16.0, |run| run.font_size);
    let font_face = runs
        .first()
        .map_or_else(|| "Arial".to_owned(), |run| run.font_face.clone());
    let text_color = runs
        .first()
        .map_or_else(|| "111827".to_owned(), |run| run.color.clone());
    let is_text_box = node
        .descendants()
        .find(|child| child.tag_name().name() == "cNvSpPr")
        .and_then(|child| child.attribute("txBox"))
        .is_some_and(|value| value == "1")
        || name.to_ascii_lowercase().contains("textbox")
        || (!text.is_empty()
            && (fill == "transparent" || fill_opacity == 0.0)
            && (stroke == "transparent" || stroke_opacity == 0.0));
    let align = paragraph
        .and_then(|value| value.attribute("algn"))
        .map(parse_align)
        .or_else(|| text_styles.paragraph_style(node, 0).align)
        .unwrap_or(HorizontalAlign::Left);
    let valign = body_style.valign.unwrap_or(VerticalAlign::Middle);
    let padding = body_style.padding.unwrap_or(8.0);
    let bold = !runs.is_empty() && runs.iter().all(|run| run.bold);
    let italic = !runs.is_empty() && runs.iter().all(|run| run.italic);
    if is_text_box {
        Element::Text(TextElement {
            id,
            x: geometry.x,
            y: geometry.y,
            w: geometry.w.max(1.0),
            h: geometry.h.max(1.0),
            text,
            fill,
            stroke,
            stroke_width,
            text_color,
            font_size,
            font_face,
            padding,
            align,
            valign,
            runs,
            opacity: 1.0,
            rotate: geometry.rotate,
            bold,
            italic,
            flip_h: geometry.flip_h,
            flip_v: geometry.flip_v,
            border_radius: shape_border_radius(node, &shape, geometry.w, geometry.h),
            fill_opacity,
            stroke_opacity,
        })
    } else {
        Element::Shape(ShapeElement {
            id,
            x: geometry.x,
            y: geometry.y,
            w: geometry.w,
            h: geometry.h,
            shape: shape.clone(),
            text,
            fill,
            stroke,
            stroke_width,
            text_color,
            font_size,
            font_face,
            padding,
            align,
            valign,
            runs,
            opacity: 1.0,
            rotate: geometry.rotate,
            bold,
            flip_h: geometry.flip_h,
            flip_v: geometry.flip_v,
            border_radius: shape_border_radius(node, &shape, geometry.w, geometry.h),
            fill_opacity,
            stroke_opacity,
        })
    }
}

fn parse_line(node: Node<'_, '_>, fallback_id: usize, theme_colors: &ThemeColors) -> LineElement {
    let geometry = geometry(node);
    let id = format!(
        "element-{}",
        non_visual(node, "id").unwrap_or_else(|| fallback_id.to_string())
    );
    let sp_pr = node
        .children()
        .find(|child| child.tag_name().name() == "spPr");
    let style = node
        .children()
        .find(|child| child.tag_name().name() == "style");
    let (stroke, stroke_width, stroke_opacity) = sp_pr
        .map(|properties| connector_line_style(properties, style, theme_colors))
        .unwrap_or_else(|| ("334155".to_owned(), 1.0, 1.0));
    let preset_geometry = sp_pr.and_then(|properties| {
        properties
            .children()
            .find(|child| child.tag_name().name() == "prstGeom")
    });
    let preset = preset_geometry.and_then(|geometry| geometry.attribute("prst"));
    let line_type = preset
        .and_then(preset_line_type)
        .unwrap_or(LineType::Straight);
    let bend_position = preset_geometry
        .and_then(|geometry| {
            geometry.descendants().find(|child| {
                child.tag_name().name() == "gd" && child.attribute("name") == Some("adj1")
            })
        })
        .and_then(|guide| guide.attribute("fmla"))
        .and_then(|formula| formula.strip_prefix("val "))
        .and_then(|value| value.parse::<f64>().ok())
        .map_or(0.5, |value| (value / 100_000.0).clamp(0.0, 1.0));
    let line = node
        .descendants()
        .find(|child| child.tag_name().name() == "ln");
    let dash = line
        .and_then(|line| {
            line.descendants()
                .find(|child| child.tag_name().name() == "prstDash")
        })
        .and_then(|dash| dash.attribute("val"))
        .map_or(DashStyle::Solid, |value| match value {
            "sysDot" | "dot" => DashStyle::Dot,
            "solid" => DashStyle::Solid,
            _ => DashStyle::Dash,
        });
    let begin_arrow = line
        .and_then(|line| {
            line.descendants()
                .find(|child| child.tag_name().name() == "headEnd")
        })
        .and_then(|node| node.attribute("type"))
        .and_then(parse_arrow);
    let end_arrow = line
        .and_then(|line| {
            line.descendants()
                .find(|child| child.tag_name().name() == "tailEnd")
        })
        .and_then(|node| node.attribute("type"))
        .and_then(parse_arrow);
    let (x1, x2) = if geometry.flip_h || preset == Some("lineInv") {
        (geometry.x + geometry.w, geometry.x)
    } else {
        (geometry.x, geometry.x + geometry.w)
    };
    let invert_vertical = geometry.flip_v || preset == Some("lineInv");
    let (y1, y2) = if invert_vertical {
        (geometry.y + geometry.h, geometry.y)
    } else {
        (geometry.y, geometry.y + geometry.h)
    };
    LineElement {
        id,
        line_type,
        elbow_direction: ElbowDirection::HorizontalFirst,
        bend_position,
        x1,
        y1,
        x2,
        y2,
        stroke,
        stroke_width,
        dash,
        begin_arrow,
        end_arrow,
        opacity: 1.0,
        stroke_opacity,
        rotate: geometry.rotate,
    }
}

fn shape_preset<'a, 'input>(node: Node<'a, 'input>) -> Option<&'a str> {
    node.descendants()
        .find(|child| child.tag_name().name() == "prstGeom")
        .and_then(|child| child.attribute("prst"))
}

fn preset_line_type(preset: &str) -> Option<LineType> {
    if matches!(preset, "line" | "lineInv") || preset.starts_with("straightConnector") {
        Some(LineType::Straight)
    } else if preset.starts_with("bentConnector") {
        Some(LineType::Elbow)
    } else {
        None
    }
}

fn parse_image(
    parts: &HashMap<String, Vec<u8>>,
    relationships: &HashMap<String, String>,
    node: Node<'_, '_>,
    fallback_id: usize,
) -> SlideResult<Option<ImageElement>> {
    let relationship_id = node
        .descendants()
        .find(|child| child.tag_name().name() == "blip")
        .and_then(|child| {
            child
                .attribute((RELATIONSHIP_NS, "embed"))
                .or_else(|| child.attribute((RELATIONSHIP_NS, "link")))
        });
    let Some(part_path) = relationship_id.and_then(|id| relationships.get(id)) else {
        return Ok(None);
    };
    let Some(bytes) = parts.get(part_path) else {
        return Ok(None);
    };
    let Some(content_type) = image_content_type(part_path) else {
        return Ok(None);
    };
    let geometry = geometry(node);
    let id = format!(
        "element-{}",
        non_visual(node, "id").unwrap_or_else(|| fallback_id.to_string())
    );
    let alt_text = non_visual(node, "descr")
        .or_else(|| non_visual(node, "title"))
        .or_else(|| non_visual(node, "name"))
        .unwrap_or_else(|| "Embedded image".to_owned());
    let crop = node
        .descendants()
        .find(|child| child.tag_name().name() == "srcRect")
        .and_then(|node| {
            let crop = ImageCrop {
                top: ratio_attr(node, "t"),
                right: ratio_attr(node, "r"),
                bottom: ratio_attr(node, "b"),
                left: ratio_attr(node, "l"),
            };
            (crop.top + crop.right + crop.bottom + crop.left > 0.0).then_some(crop)
        });
    let opacity = node
        .descendants()
        .find(|child| child.tag_name().name() == "alphaModFix")
        .and_then(|node| node.attribute("amt"))
        .and_then(|value| value.parse::<f64>().ok())
        .map_or(1.0, |value| (value / 100_000.0).clamp(0.0, 1.0));
    Ok(Some(ImageElement {
        id,
        x: geometry.x,
        y: geometry.y,
        w: geometry.w.max(1.0),
        h: geometry.h.max(1.0),
        src: format!("data:{content_type};base64,{}", STANDARD.encode(bytes)),
        fit: ImageFit::Stretch,
        crop,
        alt_text,
        opacity,
        rotate: geometry.rotate,
        flip_h: geometry.flip_h,
        flip_v: geometry.flip_v,
        border_radius: 0.0,
    }))
}

fn parse_table(
    node: Node<'_, '_>,
    starting_id: usize,
    theme_colors: &ThemeColors,
    text_styles: &InheritedTextStyles,
) -> Vec<Element> {
    parse_table_with_geometry(node, starting_id, theme_colors, text_styles, geometry(node))
}

fn parse_table_with_geometry(
    node: Node<'_, '_>,
    starting_id: usize,
    theme_colors: &ThemeColors,
    text_styles: &InheritedTextStyles,
    frame: Geometry,
) -> Vec<Element> {
    let Some(table) = node
        .descendants()
        .find(|child| child.tag_name().name() == "tbl")
    else {
        return Vec::new();
    };
    let rows = table
        .children()
        .filter(|child| child.tag_name().name() == "tr")
        .collect::<Vec<_>>();
    let columns = table
        .descendants()
        .filter(|child| child.tag_name().name() == "gridCol")
        .map(|column| numeric_attribute(column, "w", 0.0))
        .collect::<Vec<_>>();
    let column_count = columns
        .len()
        .max(
            rows.iter()
                .map(|row| {
                    row.children()
                        .filter(|child| child.tag_name().name() == "tc")
                        .count()
                })
                .max()
                .unwrap_or(0),
        )
        .max(1);
    let column_widths = resolve_table_part_sizes(&columns, column_count, frame.w, &[]);
    let weights = table_row_weights(&rows, &column_widths);
    let heights = rows
        .iter()
        .map(|row| numeric_attribute(*row, "h", 0.0))
        .collect::<Vec<_>>();
    let row_heights = resolve_table_part_sizes(&heights, rows.len(), frame.h, &weights);
    let fallback_border = table
        .descendants()
        .filter(|child| child.tag_name().name() == "tcPr")
        .find_map(table_cell_border);
    let id_base = non_visual(node, "id")
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|id| *id != 0)
        .unwrap_or(starting_id)
        .saturating_mul(1000);
    let mut namespace_xml = String::new();
    for (prefix, uri) in [
        ("p", PRESENTATION_NS),
        ("a", DRAWING_NS),
        ("r", RELATIONSHIP_NS),
    ] {
        namespace_xml.push_str(&format!(" xmlns:{prefix}=\"{uri}\""));
    }
    for namespace in node.namespaces() {
        if let Some(prefix) = namespace
            .name()
            .filter(|prefix| !["p", "a", "r", "xml"].contains(prefix))
        {
            namespace_xml.push_str(&format!(" xmlns:{prefix}=\"{}\"", namespace.uri()));
        }
    }
    let mut elements = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let mut column_index = 0_usize;
        let mut horizontal_merge_remaining = 0_usize;
        for cell in row
            .children()
            .filter(|child| child.tag_name().name() == "tc")
        {
            if cell.attribute("hMerge") == Some("1") {
                if horizontal_merge_remaining > 0 {
                    horizontal_merge_remaining -= 1;
                } else {
                    column_index = (column_index + 1).min(column_count);
                }
                continue;
            }
            let column_span = usize_attribute(cell, "gridSpan", 1)
                .max(1)
                .min(column_count.saturating_sub(column_index));
            let row_span = usize_attribute(cell, "rowSpan", 1)
                .max(1)
                .min(rows.len() - row_index);
            if cell.attribute("vMerge") == Some("1") {
                column_index += column_span;
                continue;
            }
            if column_span == 0 {
                continue;
            }
            let properties = cell
                .children()
                .find(|child| child.tag_name().name() == "tcPr");
            let fill = properties.and_then(|properties| {
                properties
                    .children()
                    .find(|child| child.tag_name().name() == "solidFill")
            });
            let border = properties.and_then(table_cell_border).or(fallback_border);
            let border_xml = border
                .map(|border| {
                    let raw = node_xml(border);
                    for name in ["lnB", "lnT", "lnL", "lnR"] {
                        if border.tag_name().name() == name {
                            return raw.replace(&format!("a:{name}"), "a:ln");
                        }
                    }
                    raw.to_owned()
                })
                .unwrap_or_default();
            let text_body = cell
                .children()
                .find(|child| child.tag_name().name() == "txBody")
                .map(node_xml)
                .unwrap_or("");
            let fallback_color = fill
                .and_then(|fill| {
                    if let Some(scheme) = fill
                        .descendants()
                        .find(|node| node.tag_name().name() == "schemeClr")
                    {
                        return Some(
                            scheme_color(scheme.attribute("val").unwrap_or("")).to_owned(),
                        );
                    }
                    descendant_color(fill, theme_colors)
                })
                .and_then(|color| parse_rgb(&color))
                .filter(|rgb| rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114 < 145.0)
                .map_or("111827", |_| "FFFFFF");
            let id = id_base.saturating_add(row_index * 100 + column_index + 1);
            let xml = format!(
                r#"<p:sp{namespace_xml}><p:nvSpPr><p:cNvPr id="{id}" name="Table cell"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:prstGeom prst="rect"/>{}{border_xml}</p:spPr><p:style><a:fontRef><a:srgbClr val="{fallback_color}"/></a:fontRef></p:style>{text_body}</p:sp>"#,
                fill.map(node_xml).unwrap_or("<a:noFill/>")
            );
            if let Ok(document) = Document::parse(&xml) {
                let mut element =
                    parse_shape(document.root_element(), id, theme_colors, text_styles);
                let x = frame.x + column_widths[..column_index].iter().sum::<f64>();
                let y = frame.y + row_heights[..row_index].iter().sum::<f64>();
                let width = column_widths[column_index..column_index + column_span]
                    .iter()
                    .sum::<f64>();
                let height = row_heights[row_index..row_index + row_span]
                    .iter()
                    .sum::<f64>();
                match &mut element {
                    Element::Text(text) => {
                        (text.x, text.y, text.w, text.h, text.rotate) =
                            (x, y, width.max(1.0), height.max(1.0), frame.rotate);
                    }
                    Element::Shape(shape) => {
                        (shape.x, shape.y, shape.w, shape.h, shape.rotate) =
                            (x, y, width, height, frame.rotate);
                    }
                    _ => {}
                }
                elements.push(element);
            }
            column_index += column_span;
            horizontal_merge_remaining = column_span.saturating_sub(1);
        }
    }
    elements
}

fn node_xml<'a, 'input>(node: Node<'a, 'input>) -> &'input str {
    &node.document().input_text()[node.range()]
}

fn numeric_attribute(node: Node<'_, '_>, name: &str, fallback: f64) -> f64 {
    node.attribute(name)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

fn table_cell_border<'a, 'input>(properties: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    ["lnB", "lnT", "lnL", "lnR"].into_iter().find_map(|name| {
        properties
            .children()
            .find(|child| child.tag_name().name() == name)
            .filter(|border| {
                !border
                    .children()
                    .any(|child| child.tag_name().name() == "noFill")
            })
    })
}

fn resolve_table_part_sizes(values: &[f64], count: usize, total: f64, weights: &[f64]) -> Vec<f64> {
    let mut sizes = (0..count)
        .map(|index| values.get(index).copied().unwrap_or(0.0).max(0.0) / EMUS_PER_PIXEL)
        .collect::<Vec<_>>();
    let missing = sizes
        .iter()
        .enumerate()
        .filter_map(|(index, size)| (*size <= 0.0).then_some(index))
        .collect::<Vec<_>>();
    let total = total.max(0.0);
    let weight = |index: usize| {
        weights
            .get(index)
            .copied()
            .filter(|weight| *weight > 0.0)
            .unwrap_or(1.0)
    };
    let remaining = total - sizes.iter().sum::<f64>();
    if !missing.is_empty() && remaining > 0.0 {
        let missing_weight = missing.iter().map(|index| weight(*index)).sum::<f64>();
        for index in missing {
            sizes[index] = remaining * weight(index) / missing_weight;
        }
        return sizes;
    }
    if !missing.is_empty() {
        let mut explicit = sizes
            .iter()
            .enumerate()
            .filter_map(|(index, size)| (*size > 0.0).then_some(*size / weight(index)))
            .collect::<Vec<_>>();
        explicit.sort_by(f64::total_cmp);
        let typical = explicit
            .get(explicit.len() / 2)
            .copied()
            .unwrap_or_else(|| {
                if total > 0.0 {
                    total / count.max(1) as f64
                } else {
                    1.0
                }
            });
        for index in missing {
            sizes[index] = typical * weight(index);
        }
    }
    let sum = sizes.iter().sum::<f64>();
    sizes
        .iter()
        .map(|size| {
            if sum > 0.0 {
                size * total / sum
            } else {
                total / count.max(1) as f64
            }
        })
        .collect()
}

fn table_row_weights(rows: &[Node<'_, '_>], columns: &[f64]) -> Vec<f64> {
    rows.iter()
        .map(|row| {
            let mut column = 0;
            let mut weight = 1.0_f64;
            for cell in row
                .children()
                .filter(|child| child.tag_name().name() == "tc")
            {
                let span = usize_attribute(cell, "gridSpan", 1)
                    .max(1)
                    .min(columns.len().saturating_sub(column).max(1));
                let width = columns.iter().skip(column).take(span).sum::<f64>();
                if cell.attribute("hMerge") != Some("1") && cell.attribute("vMerge") != Some("1") {
                    let size = ["rPr", "defRPr", "endParaRPr"]
                        .iter()
                        .find_map(|tag| {
                            cell.descendants().find(|node| {
                                node.tag_name().name() == *tag
                                    && numeric_attribute(*node, "sz", 0.0) > 0.0
                            })
                        })
                        .map_or(12.0, |node| numeric_attribute(node, "sz", 1200.0) / 100.0);
                    let properties = cell
                        .children()
                        .find(|node| node.tag_name().name() == "tcPr");
                    let inset = |name| {
                        properties.map_or(9.6, |node| {
                            numeric_attribute(node, name, 91_440.0).max(0.0) / EMUS_PER_PIXEL
                        })
                    };
                    let capacity = ((width - inset("marL") - inset("marR")).max(size)
                        / (size * (96.0 / 72.0) * 0.46).max(1.0))
                    .floor()
                    .max(1.0);
                    let lines = cell
                        .descendants()
                        .filter(|node| node.tag_name().name() == "p")
                        .map(|paragraph| {
                            let text = paragraph
                                .children()
                                .filter(|node| matches!(node.tag_name().name(), "r" | "fld" | "br"))
                                .map(|node| {
                                    if node.tag_name().name() == "br" {
                                        "\n".to_owned()
                                    } else {
                                        node.descendants()
                                            .filter(|node| node.tag_name().name() == "t")
                                            .filter_map(|node| node.text())
                                            .collect::<String>()
                                    }
                                })
                                .collect::<String>();
                            text.split('\n')
                                .map(|line| {
                                    ((line.trim().encode_utf16().count().max(1) as f64) / capacity)
                                        .ceil()
                                        .max(1.0)
                                })
                                .sum::<f64>()
                        })
                        .sum::<f64>()
                        .max(1.0);
                    weight = weight.max(
                        (1.0 + (lines - 1.0).max(0.0) * 0.5)
                            / usize_attribute(cell, "rowSpan", 1).max(1) as f64,
                    );
                }
                column += span;
            }
            weight
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Geometry {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    rotate: f64,
    flip_h: bool,
    flip_v: bool,
}

#[derive(Clone, Copy)]
struct GroupTransform {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    child_x: f64,
    child_y: f64,
    scale_x: f64,
    scale_y: f64,
    rotate: f64,
    flip_h: bool,
    flip_v: bool,
}

impl GroupTransform {
    fn from_node(node: Node<'_, '_>) -> Self {
        let xfrm = node
            .children()
            .find(|child| child.tag_name().name() == "grpSpPr")
            .and_then(|properties| {
                properties
                    .children()
                    .find(|child| child.tag_name().name() == "xfrm")
            });
        let child = |name| {
            xfrm.and_then(|transform| {
                transform
                    .children()
                    .find(|child| child.tag_name().name() == name)
            })
        };
        let off = child("off");
        let ext = child("ext");
        let child_off = child("chOff");
        let child_ext = child("chExt");
        let w = emu_attribute_option(ext, "cx") / EMUS_PER_PIXEL;
        let h = emu_attribute_option(ext, "cy") / EMUS_PER_PIXEL;
        let child_w = emu_attribute_option(child_ext, "cx") / EMUS_PER_PIXEL;
        let child_h = emu_attribute_option(child_ext, "cy") / EMUS_PER_PIXEL;
        Self {
            x: emu_attribute_option(off, "x") / EMUS_PER_PIXEL,
            y: emu_attribute_option(off, "y") / EMUS_PER_PIXEL,
            w,
            h,
            child_x: emu_attribute_option(child_off, "x") / EMUS_PER_PIXEL,
            child_y: emu_attribute_option(child_off, "y") / EMUS_PER_PIXEL,
            scale_x: if w.abs() < f64::EPSILON || child_w.abs() < f64::EPSILON {
                1.0
            } else {
                w / child_w
            },
            scale_y: if h.abs() < f64::EPSILON || child_h.abs() < f64::EPSILON {
                1.0
            } else {
                h / child_h
            },
            rotate: xfrm
                .and_then(|transform| transform.attribute("rot"))
                .and_then(|value| value.parse::<f64>().ok())
                .map_or(0.0, |value| value / 60_000.0),
            flip_h: xfrm
                .and_then(|transform| transform.attribute("flipH"))
                .is_some_and(is_true),
            flip_v: xfrm
                .and_then(|transform| transform.attribute("flipV"))
                .is_some_and(is_true),
        }
    }

    fn matrix(self) -> TransformMatrix {
        let (e, f) = self.apply_point(0.0, 0.0);
        let (x, y) = self.apply_point(1.0, 0.0);
        let (x2, y2) = self.apply_point(0.0, 1.0);
        TransformMatrix {
            a: x - e,
            b: y - f,
            c: x2 - e,
            d: y2 - f,
            e,
            f,
        }
    }

    fn apply_point(self, x: f64, y: f64) -> (f64, f64) {
        let mut x = self.x + (x - self.child_x) * self.scale_x;
        let mut y = self.y + (y - self.child_y) * self.scale_y;
        if self.flip_h {
            x = self.x + self.w - (x - self.x);
        }
        if self.flip_v {
            y = self.y + self.h - (y - self.y);
        }
        if self.rotate.abs() < f64::EPSILON {
            return (x, y);
        }

        let radians = self.rotate.to_radians();
        let (sin, cos) = radians.sin_cos();
        let center_x = self.x + self.w / 2.0;
        let center_y = self.y + self.h / 2.0;
        let offset_x = x - center_x;
        let offset_y = y - center_y;
        x = center_x + offset_x * cos - offset_y * sin;
        y = center_y + offset_x * sin + offset_y * cos;
        (x, y)
    }
}

#[derive(Clone, Copy)]
struct TransformMatrix {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl TransformMatrix {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn compose(self, child: Self) -> Self {
        Self {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            e: self.a * child.e + self.c * child.f + self.e,
            f: self.b * child.e + self.d * child.f + self.f,
        }
    }

    fn apply_rect(self, geometry: &mut Geometry) {
        let center_x = geometry.x + geometry.w / 2.0;
        let center_y = geometry.y + geometry.h / 2.0;
        let width = self.a.hypot(self.b) * geometry.w;
        let height = self.c.hypot(self.d) * geometry.h;
        geometry.x = self.a * center_x + self.c * center_y + self.e - width / 2.0;
        geometry.y = self.b * center_x + self.d * center_y + self.f - height / 2.0;
        geometry.w = width.abs();
        geometry.h = height.abs();
        geometry.rotate = (geometry.rotate + self.b.atan2(self.a).to_degrees()).rem_euclid(360.0);
        geometry.flip_v ^= self.a * self.d - self.b * self.c < 0.0;
    }

    fn apply(self, element: &mut Element) {
        let mut value = match &*element {
            Element::Text(value) => Geometry {
                x: value.x,
                y: value.y,
                w: value.w,
                h: value.h,
                rotate: value.rotate,
                flip_h: value.flip_h,
                flip_v: value.flip_v,
            },
            Element::Shape(value) => Geometry {
                x: value.x,
                y: value.y,
                w: value.w,
                h: value.h,
                rotate: value.rotate,
                flip_h: value.flip_h,
                flip_v: value.flip_v,
            },
            Element::Image(value) => Geometry {
                x: value.x,
                y: value.y,
                w: value.w,
                h: value.h,
                rotate: value.rotate,
                flip_h: value.flip_h,
                flip_v: value.flip_v,
            },
            Element::Line(value) => Geometry {
                x: value.x1.min(value.x2),
                y: value.y1.min(value.y2),
                w: (value.x2 - value.x1).abs(),
                h: (value.y2 - value.y1).abs(),
                rotate: value.rotate,
                flip_h: value.x2 < value.x1,
                flip_v: value.y2 < value.y1,
            },
        };
        let min_dimension = value.w.min(value.h);
        self.apply_rect(&mut value);
        let radius_scale = if min_dimension > 0.0 {
            value.w.min(value.h) / min_dimension
        } else {
            1.0
        };
        match element {
            Element::Text(text) => {
                text.border_radius *= radius_scale;
                (text.x, text.y, text.w, text.h) = (value.x, value.y, value.w, value.h);
                (text.rotate, text.flip_h, text.flip_v) =
                    (value.rotate, value.flip_h, value.flip_v);
            }
            Element::Shape(shape) => {
                shape.border_radius *= radius_scale;
                (shape.x, shape.y, shape.w, shape.h) = (value.x, value.y, value.w, value.h);
                (shape.rotate, shape.flip_h, shape.flip_v) =
                    (value.rotate, value.flip_h, value.flip_v);
            }
            Element::Image(image) => {
                (image.x, image.y, image.w, image.h) = (value.x, value.y, value.w, value.h);
                (image.rotate, image.flip_h, image.flip_v) =
                    (value.rotate, value.flip_h, value.flip_v);
            }
            Element::Line(line) => {
                (line.x1, line.x2) = if value.flip_h {
                    (value.x + value.w, value.x)
                } else {
                    (value.x, value.x + value.w)
                };
                (line.y1, line.y2) = if value.flip_v {
                    (value.y + value.h, value.y)
                } else {
                    (value.y, value.y + value.h)
                };
                line.rotate = value.rotate;
            }
        }
    }
}

fn geometry(node: Node<'_, '_>) -> Geometry {
    let xfrm = node
        .descendants()
        .find(|child| child.tag_name().name() == "xfrm");
    let off = xfrm.and_then(|node| {
        node.children()
            .find(|child| child.tag_name().name() == "off")
    });
    let ext = xfrm.and_then(|node| {
        node.children()
            .find(|child| child.tag_name().name() == "ext")
    });
    Geometry {
        x: emu_attribute_option(off, "x") / EMUS_PER_PIXEL,
        y: emu_attribute_option(off, "y") / EMUS_PER_PIXEL,
        w: emu_attribute_option(ext, "cx") / EMUS_PER_PIXEL,
        h: emu_attribute_option(ext, "cy") / EMUS_PER_PIXEL,
        rotate: xfrm
            .and_then(|node| node.attribute("rot"))
            .and_then(|value| value.parse::<f64>().ok())
            .map_or(0.0, |value| (value / 60_000.0).rem_euclid(360.0)),
        flip_h: xfrm
            .and_then(|node| node.attribute("flipH"))
            .is_some_and(is_true),
        flip_v: xfrm
            .and_then(|node| node.attribute("flipV"))
            .is_some_and(is_true),
    }
}

fn text_runs(
    node: Node<'_, '_>,
    theme_colors: &ThemeColors,
    text_styles: &InheritedTextStyles,
) -> Vec<TextRun> {
    let text_body = node
        .descendants()
        .find(|child| child.tag_name().name() == "txBody");
    let paragraphs = text_body
        .map(|text_body| {
            text_body
                .children()
                .filter(|child| child.tag_name().name() == "p")
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let list_style = text_body.and_then(|text_body| {
        text_body
            .children()
            .find(|child| child.tag_name().name() == "lstStyle")
    });
    let mut shape_style = text_styles.shape_style(node, theme_colors).font;
    if shape_style.font_face.is_none() {
        shape_style.font_face.clone_from(&theme_colors.minor_font);
    }
    let mut runs: Vec<TextRun> = Vec::new();
    for (paragraph_index, paragraph) in paragraphs.iter().enumerate() {
        let paragraph_properties = paragraph
            .children()
            .find(|child| child.tag_name().name() == "pPr");
        let level = paragraph_properties
            .and_then(|properties| properties.attribute("lvl"))
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0)
            .min(8);
        let mut paragraph_style = ParagraphStyle {
            run: shape_style.clone(),
            bullet: None,
            align: None,
        };
        paragraph_style.overlay(&text_styles.paragraph_style(node, level));
        let level_tag = format!("lvl{}pPr", level + 1);
        let local_level = list_style.and_then(|list_style| {
            list_style
                .children()
                .find(|child| child.tag_name().name() == level_tag)
        });
        paragraph_style.overlay(&ParagraphStyle::from_node(local_level, theme_colors));
        paragraph_style.overlay(&ParagraphStyle::from_node(
            paragraph_properties,
            theme_colors,
        ));
        let paragraph_start = runs.len();
        for run in paragraph.children().filter(Node::is_element) {
            if !matches!(run.tag_name().name(), "r" | "fld" | "br") {
                continue;
            }
            let run_properties = run
                .children()
                .find(|child| matches!(child.tag_name().name(), "rPr" | "defRPr"));
            let text = run
                .descendants()
                .filter(|child| child.tag_name().name() == "t")
                .filter_map(|node| node.text())
                .collect::<String>();
            let text = if run.tag_name().name() == "br" {
                "\n".to_owned()
            } else {
                text
            };
            let mut style = paragraph_style.run.clone();
            style.overlay(&RunStyle::from_node(run_properties, theme_colors));
            runs.push(style.text_run(text));
        }
        let local_bullet = ParagraphStyle::from_node(paragraph_properties, theme_colors).bullet;
        let prefix = match local_bullet {
            Some(BulletStyle::Character(character)) => Some(format!("{character} ")),
            Some(BulletStyle::Automatic { kind, start_at }) => {
                let suffix = if kind.contains("ParenR") { ") " } else { ". " };
                Some(format!("{}{suffix}", start_at + paragraph_index))
            }
            _ => None,
        };
        if let Some(prefix) = prefix {
            if let Some(first) = runs.get_mut(paragraph_start) {
                first.text.insert_str(0, &prefix);
            } else {
                runs.push(paragraph_style.run.text_run(prefix.trim_end().to_owned()));
            }
        }
        if runs.len() > paragraph_start {
            if let Some(last) = runs.last_mut() {
                last.break_line = true;
            }
        }
    }
    runs
}

fn text_from_runs(runs: &[TextRun]) -> String {
    let mut text = String::new();
    for run in runs {
        text.push_str(&run.text);
        if run.break_line {
            text.push('\n');
        }
    }
    while text.ends_with('\n') {
        text.pop();
    }
    text
}

fn fill_option(node: Node<'_, '_>, theme_colors: &ThemeColors) -> Option<(String, f64)> {
    if node
        .children()
        .any(|child| child.tag_name().name() == "noFill")
    {
        return Some(("transparent".to_owned(), 1.0));
    }
    node.children()
        .find(|child| child.tag_name().name() == "solidFill")
        .map(|solid| {
            (
                descendant_color(solid, theme_colors).unwrap_or_else(|| "FFFFFF".to_owned()),
                color_opacity(solid),
            )
        })
}

fn line_style(node: Node<'_, '_>, theme_colors: &ThemeColors) -> (String, f64, f64) {
    let Some(line) = node
        .children()
        .find(|child| child.tag_name().name() == "ln")
    else {
        return ("transparent".to_owned(), 0.0, 1.0);
    };
    if line
        .children()
        .any(|child| child.tag_name().name() == "noFill")
    {
        return ("transparent".to_owned(), 0.0, 1.0);
    }
    let width = line
        .attribute("w")
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .map_or(1.0, |value| value / 12_700.0);
    let solid = line
        .children()
        .find(|child| child.tag_name().name() == "solidFill");
    let Some(solid) = solid else {
        return ("transparent".to_owned(), 0.0, 1.0);
    };
    (
        descendant_color(solid, theme_colors).unwrap_or_else(|| "334155".to_owned()),
        width,
        color_opacity(solid),
    )
}

fn connector_line_style(
    properties: Node<'_, '_>,
    style: Option<Node<'_, '_>>,
    theme_colors: &ThemeColors,
) -> (String, f64, f64) {
    let line = properties
        .children()
        .find(|child| child.tag_name().name() == "ln");
    if line.is_some_and(|line| {
        line.children()
            .any(|child| child.tag_name().name() == "noFill")
    }) {
        return ("transparent".to_owned(), 0.0, 1.0);
    }

    let line_reference = style.and_then(|style| {
        style
            .descendants()
            .find(|child| child.tag_name().name() == "lnRef")
    });
    let style_index = line_reference
        .and_then(|reference| reference.attribute("idx"))
        .and_then(|value| value.parse::<usize>().ok());
    let width = line
        .and_then(|line| line.attribute("w"))
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value / 12_700.0)
        .or_else(|| style_index.and_then(|index| theme_colors.line_width(index)))
        .unwrap_or(1.0);
    let direct_fill = line.and_then(|line| {
        line.children()
            .find(|child| child.tag_name().name() == "solidFill")
    });
    let color_source = direct_fill.or(line_reference);
    (
        color_source
            .and_then(|source| descendant_color(source, theme_colors))
            .unwrap_or_else(|| "334155".to_owned()),
        width,
        color_source.map_or(1.0, color_opacity),
    )
}

fn descendant_color(node: Node<'_, '_>, theme_colors: &ThemeColors) -> Option<String> {
    let color_node = node
        .descendants()
        .find(|child| matches!(child.tag_name().name(), "srgbClr" | "sysClr" | "schemeClr"))?;
    let color = match color_node.tag_name().name() {
        "srgbClr" => color_node.attribute("val").map(clean_color),
        "sysClr" => color_node.attribute("lastClr").map(clean_color),
        "schemeClr" => Some(theme_colors.resolve(color_node.attribute("val").unwrap_or("tx1"))),
        _ => None,
    }?;
    Some(apply_color_transforms(&color, color_node))
}

fn apply_color_transforms(color: &str, node: Node<'_, '_>) -> String {
    let Some(mut rgb) = parse_rgb(color) else {
        return color.to_owned();
    };
    for transform in node.children().filter(Node::is_element) {
        let amount = transform
            .attribute("val")
            .and_then(|value| value.parse::<f64>().ok())
            .map(|value| (value / 100_000.0).clamp(0.0, 1.0));
        match (transform.tag_name().name(), amount) {
            ("tint", Some(amount)) => {
                for channel in &mut rgb {
                    *channel += (255.0 - *channel) * amount;
                }
            }
            ("shade" | "lumMod", Some(amount)) => {
                for channel in &mut rgb {
                    *channel *= amount;
                }
            }
            ("lumOff", Some(amount)) => {
                for channel in &mut rgb {
                    *channel += 255.0 * amount;
                }
            }
            _ => {}
        }
    }
    format!(
        "{:02X}{:02X}{:02X}",
        rgb[0].round().clamp(0.0, 255.0) as u8,
        rgb[1].round().clamp(0.0, 255.0) as u8,
        rgb[2].round().clamp(0.0, 255.0) as u8
    )
}

fn parse_rgb(color: &str) -> Option<[f64; 3]> {
    if color.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&color[0..2], 16).ok()? as f64,
        u8::from_str_radix(&color[2..4], 16).ok()? as f64,
        u8::from_str_radix(&color[4..6], 16).ok()? as f64,
    ])
}

fn color_opacity(node: Node<'_, '_>) -> f64 {
    node.descendants()
        .find(|child| child.tag_name().name() == "alpha")
        .and_then(|child| child.attribute("val"))
        .and_then(|value| value.parse::<f64>().ok())
        .map_or(1.0, |value| (value / 100_000.0).clamp(0.0, 1.0))
}

fn relationships(xml: &str, owning_part: &str) -> SlideResult<HashMap<String, String>> {
    let document = parse_xml(xml)?;
    let mut result = HashMap::new();
    for node in document
        .descendants()
        .filter(|node| node.tag_name().name() == "Relationship")
    {
        let (Some(id), Some(target)) = (node.attribute("Id"), node.attribute("Target")) else {
            continue;
        };
        let target = resolve_target(owning_part, target).map_err(|_| invalid_powerpoint())?;
        result.insert(id.to_owned(), target);
    }
    Ok(result)
}

fn resolve_target(owning_part: &str, target: &str) -> Result<String, ()> {
    if target.contains('\0') || target.contains("\\") {
        return Err(());
    }
    let mut components = if target.starts_with('/') {
        Vec::new()
    } else {
        owning_part
            .rsplit_once('/')
            .map_or(Vec::new(), |(directory, _)| {
                directory.split('/').collect::<Vec<_>>()
            })
    };
    for component in target.trim_start_matches('/').split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop().ok_or(())?;
            }
            value => components.push(value),
        }
    }
    Ok(components.join("/"))
}

fn relationship_part_path(part: &str) -> String {
    let (directory, file) = part.rsplit_once('/').unwrap_or(("", part));
    format!("{directory}/_rels/{file}.rels")
}

fn safe_part_name(name: &str) -> bool {
    !name.starts_with('/')
        && !name.contains('\0')
        && !name.contains('\\')
        && !name.split('/').any(|component| component == "..")
}

fn utf8_part<'a>(parts: &'a HashMap<String, Vec<u8>>, path: &str) -> SlideResult<&'a str> {
    std::str::from_utf8(parts.get(path).ok_or_else(invalid_powerpoint)?)
        .map_err(|_| invalid_powerpoint())
}

fn parse_xml(xml: &str) -> SlideResult<Document<'_>> {
    Document::parse(xml).map_err(|_| invalid_powerpoint())
}
fn non_visual(node: Node<'_, '_>, attribute: &str) -> Option<String> {
    node.descendants()
        .find(|child| child.tag_name().name() == "cNvPr")
        .and_then(|node| node.attribute(attribute))
        .map(str::to_owned)
}
fn emu_attribute(node: Node<'_, '_>, attribute: &str, fallback: i64) -> f64 {
    node.attribute(attribute)
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(fallback) as f64
}
fn emu_attribute_option(node: Option<Node<'_, '_>>, attribute: &str) -> f64 {
    node.and_then(|node| node.attribute(attribute))
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0)
}
fn usize_attribute(node: Node<'_, '_>, attribute: &str, fallback: usize) -> usize {
    node.attribute(attribute)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(fallback)
}
fn boolean_attribute(node: Node<'_, '_>, attribute: &str) -> Option<bool> {
    node.attribute(attribute).map(is_true)
}
fn ratio_attr(node: Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .and_then(|value| value.parse::<f64>().ok())
        .map_or(0.0, |value| (value / 100_000.0).clamp(0.0, 1.0))
}
fn is_true(value: &str) -> bool {
    matches!(value, "1" | "true" | "on")
}
fn clean_color(value: &str) -> String {
    value
        .trim_start_matches('#')
        .get(..6)
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map_or_else(|| "111827".to_owned(), str::to_ascii_uppercase)
}

fn parse_align(value: &str) -> HorizontalAlign {
    match value {
        "ctr" | "center" => HorizontalAlign::Center,
        "r" | "right" => HorizontalAlign::Right,
        _ => HorizontalAlign::Left,
    }
}
fn parse_valign(value: &str) -> VerticalAlign {
    match value {
        "t" | "top" => VerticalAlign::Top,
        "b" | "bottom" => VerticalAlign::Bottom,
        _ => VerticalAlign::Middle,
    }
}
fn parse_arrow(value: &str) -> Option<ArrowType> {
    match value {
        "triangle" => Some(ArrowType::Triangle),
        "arrow" => Some(ArrowType::Arrow),
        "diamond" => Some(ArrowType::Diamond),
        "oval" => Some(ArrowType::Oval),
        "stealth" => Some(ArrowType::Stealth),
        _ => None,
    }
}
fn image_content_type(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "svg" => Some("image/svg+xml"),
        "emf" => Some("image/emf"),
        _ => Some("image/png"),
    }
}
fn scheme_color(value: &str) -> &'static str {
    match value {
        "dk1" | "tx1" => "070154",
        "lt1" | "bg1" => "FFFFFF",
        "dk2" | "tx2" => "0047FF",
        "lt2" | "bg2" => "F6EB20",
        "accent1" => "F900D3",
        "accent2" => "50658E",
        "accent3" => "CED7E6",
        "accent4" => "E8EEF8",
        "accent5" => "00E8FA",
        "accent6" => "00A3FF",
        "hlink" => "0563C1",
        "folHlink" => "954F72",
        _ => "111827",
    }
}

fn invalid_powerpoint() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::Invalid,
        "invalid_powerpoint",
        "The uploaded file is not a supported PowerPoint OOXML presentation.",
    )
}
fn powerpoint_too_large() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::TooLarge,
        "powerpoint_too_large",
        "The expanded PowerPoint package exceeds the processing limit.",
    )
}

const PRESENTATION_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const RELATIONSHIP_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/pptx/reader_tests.rs"]
mod tests;

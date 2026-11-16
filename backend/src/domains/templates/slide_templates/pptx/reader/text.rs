//! Inherited text styles and paragraph/run conversion.

use super::super::super::{
    HorizontalAlign, TextParagraph, TextRun, TextSpacing, TextTabStop, VerticalAlign,
};
use super::geometry::{geometry, Geometry};
use super::package::{part_relationships, relationship_target_with_segment};
use super::shape_preset;
use super::theme::{color_opacity, descendant_color, fill_option, line_style, ThemeColors};
use super::{boolean_attribute, usize_attribute, EMUS_PER_PIXEL};
use roxmltree::{Document, Node};
use std::collections::HashMap;

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
            paragraph: None,
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
pub(super) struct ParagraphStyle {
    run: RunStyle,
    bullet: Option<BulletStyle>,
    pub(super) align: Option<HorizontalAlign>,
    layout: TextParagraph,
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
            layout: TextParagraph {
                margin_left: optional_emu(node, "marL"),
                margin_right: optional_emu(node, "marR"),
                indent: optional_emu(node, "indent"),
                default_tab_size: optional_emu(node, "defTabSz"),
                tab_stops: node
                    .children()
                    .find(|n| n.tag_name().name() == "tabLst")
                    .map(|tabs| {
                        tabs.children()
                            .filter(|n| n.tag_name().name() == "tab")
                            .filter_map(|tab| {
                                Some(TextTabStop {
                                    position: optional_emu(tab, "pos")?,
                                    align: parse_align(tab.attribute("algn").unwrap_or("l")),
                                })
                            })
                            .collect()
                    }),
                line_spacing: paragraph_spacing(node, "lnSpc"),
                space_before: paragraph_spacing(node, "spcBef"),
                space_after: paragraph_spacing(node, "spcAft"),
                align: node.attribute("algn").map(parse_align),
            },
        }
    }

    fn overlay(&mut self, other: &Self) {
        self.run.overlay(&other.run);
        macro_rules! overlay_layout {
            ($($field:ident),*) => { $(if other.layout.$field.is_some() {
                self.layout.$field.clone_from(&other.layout.$field);
            })* };
        }
        overlay_layout!(
            margin_left,
            margin_right,
            indent,
            default_tab_size,
            tab_stops,
            line_spacing,
            space_before,
            space_after,
            align
        );
        if other.align.is_some() {
            self.align = other.align;
        }
        if other.bullet.is_some() {
            self.bullet.clone_from(&other.bullet);
        }
    }
}

fn optional_emu(node: Node<'_, '_>, attribute: &str) -> Option<f64> {
    node.attribute(attribute)?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v / EMUS_PER_PIXEL)
}

fn paragraph_spacing(node: Node<'_, '_>, tag: &str) -> Option<TextSpacing> {
    let spacing = node.children().find(|n| n.tag_name().name() == tag)?;
    let value = spacing.children().find(Node::is_element)?;
    let number = value
        .attribute("val")?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)?;
    match value.tag_name().name() {
        "spcPts" => Some(TextSpacing::Points(number / 100.0)),
        "spcPct" => Some(TextSpacing::Percent(number / 100_000.0)),
        _ => None,
    }
}

struct TextStyleLevels {
    levels: [ParagraphStyle; 9],
}

#[derive(Clone, Default)]
pub(super) struct TextBodyStyle {
    pub(super) valign: Option<VerticalAlign>,
    pub(super) padding: Option<f64>,
}

impl TextBodyStyle {
    pub(super) fn from_node(node: Option<Node<'_, '_>>) -> Self {
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

    pub(super) fn overlay(&mut self, other: &Self) {
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
pub(super) struct PlaceholderKey {
    kind: String,
    index: Option<String>,
}

#[derive(Clone, Default)]
pub(super) struct ShapeDefaults {
    pub(super) geometry: Option<Geometry>,
    pub(super) fill: Option<(String, f64)>,
    pub(super) line: Option<(String, f64, f64)>,
    pub(super) preset: Option<String>,
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
pub(super) struct InheritedTextStyles {
    layout: Vec<PlaceholderTextStyles>,
    master: Vec<PlaceholderTextStyles>,
    title: TextStyleLevels,
    body: TextStyleLevels,
    other: TextStyleLevels,
}

impl InheritedTextStyles {
    pub(super) fn for_slide(
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

    pub(super) fn shape_style(&self, node: Node<'_, '_>, theme: &ThemeColors) -> ShapeDefaults {
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

    pub(super) fn paragraph_style(&self, node: Node<'_, '_>, level: usize) -> ParagraphStyle {
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

    pub(super) fn body_style(&self, node: Node<'_, '_>) -> TextBodyStyle {
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

pub(super) fn placeholder_key(node: Node<'_, '_>) -> Option<PlaceholderKey> {
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

pub(super) fn text_runs(
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
            ..ParagraphStyle::default()
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
        let local_bullet = paragraph_style.bullet.clone();
        let prefix = match local_bullet {
            Some(BulletStyle::Character(character)) => Some(format!("{character}\t")),
            Some(BulletStyle::Automatic { kind, start_at }) => {
                let suffix = if kind.contains("ParenR") {
                    ")\t"
                } else {
                    ".\t"
                };
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
        if runs.len() == paragraph_start {
            runs.push(paragraph_style.run.text_run(String::new()));
        }
        if paragraph_style.layout != TextParagraph::default() {
            runs[paragraph_start].paragraph = Some(paragraph_style.layout);
        }
        if runs.len() > paragraph_start {
            if let Some(last) = runs.last_mut() {
                last.break_line = true;
            }
        }
    }
    runs
}

pub(super) fn text_from_runs(runs: &[TextRun]) -> String {
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

pub(super) fn parse_align(value: &str) -> HorizontalAlign {
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

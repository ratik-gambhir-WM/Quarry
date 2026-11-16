//! Theme colors, fonts, fills and line styling.

use super::package::{part_relationships, relationship_target_with_segment};
use roxmltree::{Document, Node};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ThemeColors {
    values: HashMap<String, String>,
    pub(super) major_font: Option<String>,
    pub(super) minor_font: Option<String>,
    line_widths: Vec<f64>,
}

impl ThemeColors {
    pub(super) fn for_slide(
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

    pub(super) fn resolve(&self, value: &str) -> String {
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

    pub(super) fn resolve_font(&self, value: &str) -> String {
        match value {
            "+mj-lt" | "+mj-ea" | "+mj-cs" => self.major_font.as_deref().unwrap_or("Arial"),
            "+mn-lt" | "+mn-ea" | "+mn-cs" => self.minor_font.as_deref().unwrap_or("Arial"),
            value => value,
        }
        .to_owned()
    }

    pub(super) fn line_width(&self, style_index: usize) -> Option<f64> {
        style_index
            .checked_sub(1)
            .and_then(|index| self.line_widths.get(index))
            .copied()
    }
}

pub(super) fn fill_option(node: Node<'_, '_>, theme_colors: &ThemeColors) -> Option<(String, f64)> {
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

pub(super) fn line_style(node: Node<'_, '_>, theme_colors: &ThemeColors) -> (String, f64, f64) {
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

pub(super) fn connector_line_style(
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

pub(super) fn descendant_color(node: Node<'_, '_>, theme_colors: &ThemeColors) -> Option<String> {
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

pub(super) fn parse_rgb(color: &str) -> Option<[f64; 3]> {
    if color.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&color[0..2], 16).ok()? as f64,
        u8::from_str_radix(&color[2..4], 16).ok()? as f64,
        u8::from_str_radix(&color[4..6], 16).ok()? as f64,
    ])
}

pub(super) fn color_opacity(node: Node<'_, '_>) -> f64 {
    node.descendants()
        .find(|child| child.tag_name().name() == "alpha")
        .and_then(|child| child.attribute("val"))
        .and_then(|value| value.parse::<f64>().ok())
        .map_or(1.0, |value| (value / 100_000.0).clamp(0.0, 1.0))
}

fn clean_color(value: &str) -> String {
    value
        .trim_start_matches('#')
        .get(..6)
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map_or_else(|| "111827".to_owned(), str::to_ascii_uppercase)
}

pub(super) fn scheme_color(value: &str) -> &'static str {
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

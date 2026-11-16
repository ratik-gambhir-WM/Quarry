//! Shape, line, image, group and table conversion.

use super::super::super::{
    ArrowType, DashStyle, ElbowDirection, Element, HorizontalAlign, ImageCrop, ImageElement,
    ImageFit, LineElement, LineType, ShapeElement, SlideResult, TextElement, VerticalAlign,
};
use super::geometry::{geometry, Geometry, GroupTransform, TransformMatrix};
use super::package::parse_xml;
use super::text::{parse_align, text_from_runs, text_runs, InheritedTextStyles, TextBodyStyle};
use super::theme::{connector_line_style, descendant_color, parse_rgb, scheme_color, ThemeColors};
use super::{
    is_inherited_metafile, non_visual, ratio_attr, shape_preset, should_skip_drawable,
    usize_attribute, warn_shape_fallbacks, warn_unsupported_graphic, SlideParseContext, DRAWING_NS,
    EMUS_PER_PIXEL, PRESENTATION_NS, RELATIONSHIP_NS, SLIDE_BOUNDS_TOLERANCE_PX,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use roxmltree::{Document, Node};
use std::collections::HashMap;

pub(super) fn background_image_from_part(
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

pub(super) fn parse_group(
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

pub(super) fn is_completely_outside_slide(element: &Element, width: f64, height: f64) -> bool {
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

pub(super) fn parse_shape(
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

pub(super) fn parse_line(
    node: Node<'_, '_>,
    fallback_id: usize,
    theme_colors: &ThemeColors,
) -> LineElement {
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

fn preset_line_type(preset: &str) -> Option<LineType> {
    if matches!(preset, "line" | "lineInv") || preset.starts_with("straightConnector") {
        Some(LineType::Straight)
    } else if preset.starts_with("bentConnector") {
        Some(LineType::Elbow)
    } else {
        None
    }
}

pub(super) fn parse_image(
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

pub(super) fn parse_table(
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
            let id = id_base.saturating_add(row_index * 100 + column_index + 1);
            let xml = table_cell_shape_xml(cell, fallback_border, theme_colors, &namespace_xml, id);
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

/// Reuse shape parsing for table-cell fills, borders, and text styling.
fn table_cell_shape_xml(
    cell: Node<'_, '_>,
    fallback_border: Option<Node<'_, '_>>,
    theme_colors: &ThemeColors,
    namespace_xml: &str,
    id: usize,
) -> String {
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
                return Some(scheme_color(scheme.attribute("val").unwrap_or("")).to_owned());
            }
            descendant_color(fill, theme_colors)
        })
        .and_then(|color| parse_rgb(&color))
        .filter(|rgb| rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114 < 145.0)
        .map_or("111827", |_| "FFFFFF");
    format!(
        r#"<p:sp{namespace_xml}><p:nvSpPr><p:cNvPr id="{id}" name="Table cell"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:prstGeom prst="rect"/>{}{border_xml}</p:spPr><p:style><a:fontRef><a:srgbClr val="{fallback_color}"/></a:fontRef></p:style>{text_body}</p:sp>"#,
        fill.map(node_xml).unwrap_or("<a:noFill/>")
    )
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

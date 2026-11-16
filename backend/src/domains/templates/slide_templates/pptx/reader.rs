//! PowerPoint package orchestration and slide traversal.

mod drawables;
mod geometry;
mod package;
mod text;
mod theme;

use super::super::error::FailureKind;
use super::super::{
    CanvasDocument, Conversion, Element, Presentation, Slide, SlideResult, SlideTemplateError,
};
use drawables::{
    background_image_from_part, is_completely_outside_slide, parse_group, parse_image, parse_line,
    parse_shape, parse_table,
};
use package::{
    parse_xml, part_relationships, read_package, relationship_part_path,
    relationship_target_with_segment, relationships, utf8_part,
};
pub(super) use package::{read_part, validate_package};
use roxmltree::{Document, Node};
use std::collections::HashMap;
use text::{placeholder_key, InheritedTextStyles};
use theme::{descendant_color, ThemeColors};

const DRAWING_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const EMUS_PER_PIXEL: f64 = 9_525.0;
const MAX_ENTRY_BYTES: u64 = 50 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 250 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const SLIDE_BOUNDS_TOLERANCE_PX: f64 = 1.0;

pub(super) fn read(source: &[u8]) -> SlideResult<Conversion> {
    read_inner(source, false, None)
}

pub(super) fn read_parallel(
    source: &[u8],
    cancellation: &super::super::RequestCancellation,
) -> SlideResult<Conversion> {
    read_inner(source, true, Some(cancellation))
}

fn read_inner(
    source: &[u8],
    parallel: bool,
    cancellation: Option<&super::super::RequestCancellation>,
) -> SlideResult<Conversion> {
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

    // A deck is bounded separately from the ZIP entry/expansion limits.
    if parallel && slide_paths.len() > 100 {
        return Err(SlideTemplateError::new(
            FailureKind::TooLarge,
            "too_many_slides",
            "Classified deck imports support at most 100 slides.",
        ));
    }
    let parse = |(index, slide_path): (usize, String)| {
        parse_slide_part(&parts, &slide_path, index, (width, height), cancellation)
    };
    let indexed = slide_paths.into_iter().enumerate().collect::<Vec<_>>();
    let parsed = if parallel {
        super::super::parallel::map(indexed, parse)?
    } else {
        indexed
            .into_iter()
            .map(parse)
            .collect::<SlideResult<Vec<_>>>()?
    };
    let (slides, warnings): (Vec<_>, Vec<_>) = parsed.into_iter().unzip();
    let warnings = warnings.into_iter().flatten().collect();
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

/// Resolve one slide's relationships and inheritance independently of deck scheduling.
fn parse_slide_part(
    parts: &HashMap<String, Vec<u8>>,
    slide_path: &str,
    index: usize,
    size: (f64, f64),
    cancellation: Option<&super::super::RequestCancellation>,
) -> SlideResult<(Slide, Vec<String>)> {
    super::super::check_cancelled(cancellation)?;
    let mut warnings = Vec::new();
    let slide_xml = utf8_part(parts, slide_path)?;
    let rels_path = relationship_part_path(slide_path);
    let slide_relationships = match parts.get(&rels_path) {
        Some(bytes) => relationships(
            std::str::from_utf8(bytes).map_err(|_| invalid_powerpoint())?,
            slide_path,
        )?,
        None => HashMap::new(),
    };
    let theme_colors = ThemeColors::for_slide(parts, &slide_relationships);
    let text_styles = InheritedTextStyles::for_slide(parts, &slide_relationships, &theme_colors);
    let mut context = SlideParseContext {
        parts,
        relationships: &slide_relationships,
        slide_index: index,
        source_path: slide_path,
        theme_colors: &theme_colors,
        text_styles: &text_styles,
        warnings: &mut warnings,
        inherited: false,
    };
    let mut slide = parse_slide(slide_xml, size, &mut context)?;
    let slide_document = parse_xml(slide_xml)?;
    if !slide_document
        .descendants()
        .any(|node| node.has_tag_name((PRESENTATION_NS, "bg")))
    {
        if let Some(color) = inherited_background_color(parts, &slide_relationships, &theme_colors)
        {
            slide.background_color = color;
        }
    }
    let local_warnings = std::mem::take(context.warnings);
    let mut inherited = inherited_drawables(&mut context, slide_xml, size)?;
    context.warnings.extend(local_warnings);
    inherited.append(&mut slide.elements);
    slide.elements = inherited;
    if slide.elements.is_empty() {
        context.warnings.push(format!("slide {}: shapeTree.elements: No exportable slide elements were found. The PowerPoint will be empty.", index + 1));
    }
    slide.name = slide_title(&slide.elements, index);
    Ok((slide, warnings))
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

fn shape_preset<'a, 'input>(node: Node<'a, 'input>) -> Option<&'a str> {
    node.descendants()
        .find(|child| child.tag_name().name() == "prstGeom")
        .and_then(|child| child.attribute("prst"))
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

//! Flexible native slide JSON normalization compatible with the TypeScript endpoint.

use serde_json::{Map, Value};

use super::error::FailureKind;
use super::{
    ArrowType, CanvasDocument, DashStyle, Element, HorizontalAlign, ImageCrop, ImageElement,
    ImageFit, LineElement, LineType, Presentation, ShapeElement, Slide, SlideTemplateError,
    TextElement, TextRun, VerticalAlign,
};

const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 720.0;
const DEFAULT_FONT: &str = "Aptos";

pub(super) fn normalize(
    input: &Value,
) -> Result<(CanvasDocument, Vec<String>), SlideTemplateError> {
    let mut warnings = Vec::new();
    let (presentation_node, slides_source): (&Map<String, Value>, Vec<&Value>) = match input {
        Value::Array(slides) => {
            let empty = empty_map();
            return normalize_parts(&empty, slides.iter().collect(), &mut warnings);
        }
        Value::Object(root) => {
            let presentation = root
                .get("presentation")
                .and_then(Value::as_object)
                .unwrap_or(root);
            let slides = presentation
                .get("slides")
                .and_then(Value::as_array)
                .or_else(|| root.get("slides").and_then(Value::as_array))
                .map(|items| items.iter().collect())
                .or_else(|| {
                    presentation
                        .get("slide")
                        .filter(|slide| slide.get("elements").is_some())
                        .map(|slide| vec![slide])
                })
                .or_else(|| root.get("elements").map(|_| vec![input]));
            let Some(slides) = slides else {
                return Err(invalid_presentation());
            };
            (presentation, slides)
        }
        _ => return Err(invalid_presentation()),
    };
    normalize_parts(presentation_node, slides_source, &mut warnings)
}

fn empty_map() -> Map<String, Value> {
    Map::new()
}

fn normalize_parts(
    presentation_node: &Map<String, Value>,
    slides_source: Vec<&Value>,
    warnings: &mut Vec<String>,
) -> Result<(CanvasDocument, Vec<String>), SlideTemplateError> {
    if slides_source.is_empty() {
        return Err(invalid_presentation());
    }
    let width = dimension(presentation_node, "width", "w", DEFAULT_WIDTH);
    let height = dimension(presentation_node, "height", "h", DEFAULT_HEIGHT);
    let preserve = boolean(presentation_node.get("preserveElementOrder"), false);
    let slides = slides_source
        .into_iter()
        .enumerate()
        .filter_map(|(index, value)| normalize_slide(value, index, width, height, warnings))
        .collect::<Vec<_>>();
    if slides.is_empty() {
        return Err(invalid_presentation());
    }
    let title = string(presentation_node.get("title"))
        .unwrap_or("Generated Presentation")
        .to_owned();
    Ok((
        CanvasDocument {
            presentation: Presentation {
                title,
                preserve_element_order: preserve,
                show_branding: boolean(presentation_node.get("showBranding"), true),
                slides,
            },
        },
        std::mem::take(warnings),
    ))
}

fn normalize_slide(
    value: &Value,
    index: usize,
    default_width: f64,
    default_height: f64,
    warnings: &mut Vec<String>,
) -> Option<Slide> {
    let Some(source) = value.as_object() else {
        warnings.push(format!(
            "slides[{index}]: Skipped a non-object slide entry."
        ));
        return None;
    };
    let width = dimension(source, "width", "w", default_width);
    let height = dimension(source, "height", "h", default_height);
    let elements = source
        .get("elements")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .enumerate()
        .filter_map(|(element_index, element)| {
            normalize_element(
                element,
                width,
                height,
                &format!("slides[{index}].elements[{element_index}]"),
                warnings,
            )
        })
        .collect();
    Some(Slide {
        id: string(source.get("id"))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("slide-{}", index + 1)),
        name: string(source.get("name"))
            .or_else(|| string(source.get("title")))
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Slide {}", index + 1)),
        width,
        height,
        background_color: color(source.get("backgroundColor"), "FFFFFF"),
        elements,
    })
}

fn normalize_element(
    value: &Value,
    width: f64,
    height: f64,
    path: &str,
    warnings: &mut Vec<String>,
) -> Option<Element> {
    let Some(source) = value.as_object() else {
        warnings.push(format!("{path}: Skipped a non-object element."));
        return None;
    };
    let raw_kind = ["kind", "type", "elementType", "shape"]
        .iter()
        .find_map(|key| string(source.get(*key)))
        .unwrap_or("");
    let kind = raw_kind.to_ascii_lowercase();
    let id = string(source.get("id")).unwrap_or(path).to_owned();
    let opacity = clamp01(number(source.get("opacity"), 1.0));
    let rotate = number(source.get("rotate"), 0.0);

    if matches!(kind.as_str(), "line" | "connector" | "arrow" | "elbow") {
        let x1 = position(first(source, &["x1", "x", "left"]), width);
        let y1 = position(first(source, &["y1", "y", "top"]), height);
        let x2 = source.get("x2").map_or_else(
            || x1 + position(first(source, &["w", "width"]), width),
            |value| position(Some(value), width),
        );
        let y2 = source.get("y2").map_or_else(
            || y1 + position(first(source, &["h", "height"]), height),
            |value| position(Some(value), height),
        );
        return Some(Element::Line(LineElement {
            elbow_direction: elbow_direction(string(source.get("elbowDirection")), x1, y1, x2, y2),
            id,
            line_type: line_type(string(source.get("lineType")), x1, y1, x2, y2),
            bend_position: number(source.get("bendPosition"), 0.5).clamp(0.0, 1.0),
            x1,
            y1,
            x2,
            y2,
            stroke: color(first(source, &["stroke", "color"]), "334155"),
            stroke_width: number(source.get("strokeWidth"), 1.5),
            dash: dash(string(source.get("dash"))),
            begin_arrow: arrow(
                string(source.get("beginArrow")).or_else(|| string(source.get("startArrow"))),
            ),
            end_arrow: arrow(
                string(source.get("endArrow")).or_else(|| string(source.get("arrow"))),
            ),
            opacity,
            stroke_opacity: clamp01(number(source.get("strokeOpacity"), 1.0)),
            rotate,
        }));
    }

    if matches!(kind.as_str(), "image" | "img" | "picture" | "photo") {
        let Some(src) = first(source, &["src", "path", "data"]).and_then(Value::as_str) else {
            warnings.push(format!(
                "{path}.src: Skipped an image element because it did not include a usable image source."
            ));
            return None;
        };
        return Some(Element::Image(ImageElement {
            id,
            x: position(first(source, &["x", "left"]), width),
            y: position(first(source, &["y", "top"]), height),
            w: position(first(source, &["w", "width"]), width),
            h: position(first(source, &["h", "height"]), height),
            src: src.to_owned(),
            fit: image_fit(string(source.get("fit"))),
            crop: image_crop(source.get("crop")),
            alt_text: string(source.get("altText")).unwrap_or("").to_owned(),
            opacity,
            rotate,
            flip_h: boolean(source.get("flipH"), false),
            flip_v: boolean(source.get("flipV"), false),
            border_radius: number(source.get("borderRadius"), 0.0),
        }));
    }

    let is_text = matches!(kind.as_str(), "text" | "textbox" | "text-box");
    let known_shape = matches!(
        kind.as_str(),
        "shape"
            | "rect"
            | "rectangle"
            | "roundrect"
            | "roundedrectangle"
            | "ellipse"
            | "circle"
            | "triangle"
            | "diamond"
            | "hexagon"
            | "chevron"
            | "arc"
    );
    if !is_text && !known_shape {
        warnings.push(format!(
            "{path}: Unsupported element type \"{}\".",
            if raw_kind.is_empty() {
                "unknown"
            } else {
                raw_kind
            }
        ));
        return None;
    }

    let text = first(source, &["text", "label"])
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let font_face = string(source.get("fontFace"))
        .unwrap_or(DEFAULT_FONT)
        .to_owned();
    let font_size = number(source.get("fontSize"), 18.0);
    let text_color = color(first(source, &["color", "textColor"]), "111827");
    let runs = text_runs(
        source.get("runs"),
        &text,
        &font_face,
        font_size,
        &text_color,
        boolean(source.get("bold"), false),
        boolean(source.get("italic"), false),
    );
    let x = position(first(source, &["x", "left"]), width);
    let y = position(first(source, &["y", "top"]), height);
    let w = position(first(source, &["w", "width"]), width);
    let h = position(first(source, &["h", "height"]), height);
    let align = align(string(source.get("align")));
    let valign = valign(string(source.get("valign")));

    if is_text {
        Some(Element::Text(TextElement {
            id,
            x,
            y,
            w,
            h,
            text,
            fill: color(source.get("fill"), "FFFFFF"),
            stroke: color(source.get("stroke"), "FFFFFF"),
            stroke_width: number(source.get("strokeWidth"), 0.0),
            text_color,
            font_size,
            font_face,
            padding: number(source.get("padding"), 8.0),
            align,
            valign,
            runs,
            opacity,
            rotate,
            bold: boolean(source.get("bold"), false),
            italic: boolean(source.get("italic"), false),
            flip_h: boolean(source.get("flipH"), false),
            flip_v: boolean(source.get("flipV"), false),
            border_radius: number(source.get("borderRadius"), 0.0),
            fill_opacity: clamp01(number(source.get("fillOpacity"), 1.0)),
            stroke_opacity: clamp01(number(source.get("strokeOpacity"), 1.0)),
        }))
    } else {
        Some(Element::Shape(ShapeElement {
            id,
            x,
            y,
            w,
            h,
            shape: shape_name(string(source.get("shape")).unwrap_or(raw_kind)),
            text,
            fill: color(source.get("fill"), "E5EEF8"),
            stroke: color(source.get("stroke"), "334155"),
            stroke_width: number(source.get("strokeWidth"), 1.0),
            text_color,
            font_size,
            font_face,
            padding: number(source.get("padding"), 8.0),
            align,
            valign,
            runs,
            opacity,
            rotate,
            bold: boolean(source.get("bold"), false),
            flip_h: boolean(source.get("flipH"), false),
            flip_v: boolean(source.get("flipV"), false),
            border_radius: number(source.get("borderRadius"), 0.0),
            fill_opacity: clamp01(number(source.get("fillOpacity"), 1.0)),
            stroke_opacity: clamp01(number(source.get("strokeOpacity"), 1.0)),
        }))
    }
}

fn text_runs(
    value: Option<&Value>,
    fallback_text: &str,
    fallback_font: &str,
    fallback_size: f64,
    fallback_color: &str,
    fallback_bold: bool,
    fallback_italic: bool,
) -> Vec<TextRun> {
    let mut runs = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .map(|run| TextRun {
            text: string(run.get("text")).unwrap_or("").to_owned(),
            color: color(run.get("color"), fallback_color),
            font_face: string(run.get("fontFace"))
                .unwrap_or(fallback_font)
                .to_owned(),
            font_size: number(run.get("fontSize"), fallback_size),
            bold: boolean(run.get("bold"), false),
            italic: boolean(run.get("italic"), false),
            underline: boolean(run.get("underline"), false),
            break_line: boolean(run.get("breakLine"), false),
        })
        .collect::<Vec<_>>();
    if runs.is_empty() && !fallback_text.is_empty() {
        runs.push(TextRun {
            text: fallback_text.to_owned(),
            color: fallback_color.to_owned(),
            font_face: fallback_font.to_owned(),
            font_size: fallback_size,
            bold: fallback_bold,
            italic: fallback_italic,
            underline: false,
            break_line: false,
        });
    }
    runs
}

fn dimension(source: &Map<String, Value>, key: &str, short: &str, fallback: f64) -> f64 {
    if let Some(value) = source.get(key).and_then(Value::as_f64) {
        return value;
    }
    for container in ["canvas", "size", "layout"] {
        if let Some(object) = source.get(container).and_then(Value::as_object) {
            if let Some(value) = object
                .get(key)
                .or_else(|| object.get(short))
                .and_then(Value::as_f64)
            {
                return value;
            }
            let inches = if key == "width" {
                "widthInches"
            } else {
                "heightInches"
            };
            if let Some(value) = object.get(inches).and_then(Value::as_f64) {
                return value * 96.0;
            }
        }
    }
    fallback
}

fn first<'a>(source: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| source.get(*key))
}

fn string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn number(value: Option<&Value>, fallback: f64) -> f64 {
    match value {
        Some(Value::Number(number)) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .unwrap_or(fallback),
        Some(Value::String(value)) => value
            .parse()
            .ok()
            .filter(|number: &f64| number.is_finite())
            .unwrap_or(fallback),
        _ => fallback,
    }
}

fn position(value: Option<&Value>, canvas_size: f64) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(0.0),
        Some(Value::String(value)) => {
            let value = super::retrieval::js_trim(value);
            if let Some(percent) = value.strip_suffix('%') {
                return percent
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map_or(0.0, |number| canvas_size * number / 100.0);
            }
            if let Some(inches) = value.strip_suffix("in") {
                return inches
                    .parse::<f64>()
                    .ok()
                    .filter(|n| n.is_finite())
                    .map_or(0.0, |number| number * 96.0);
            }
            value
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

fn boolean(value: Option<&Value>, fallback: bool) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_f64().is_some_and(|v| {
            v.is_finite() && v.fract() == 0.0 && v.abs() <= 9_223_372_036_854_776_000.0 && v != 0.0
        }),
        Some(Value::String(value)) => {
            matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
        }
        _ => fallback,
    }
}

fn color(value: Option<&Value>, fallback: &str) -> String {
    let Some(value) = value.and_then(Value::as_str) else {
        return fallback.to_owned();
    };
    if value.eq_ignore_ascii_case("transparent") {
        return "transparent".to_owned();
    }
    let clean = super::retrieval::js_trim(value).trim_start_matches('#');
    if (clean.len() == 6 || clean.len() == 8) && clean.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        clean[..6].to_ascii_uppercase()
    } else {
        fallback.to_owned()
    }
}

fn align(value: Option<&str>) -> HorizontalAlign {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("center" | "ctr") => HorizontalAlign::Center,
        Some("right" | "r") => HorizontalAlign::Right,
        _ => HorizontalAlign::Left,
    }
}

fn valign(value: Option<&str>) -> VerticalAlign {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("top" | "t") => VerticalAlign::Top,
        Some("bottom" | "b") => VerticalAlign::Bottom,
        _ => VerticalAlign::Middle,
    }
}

fn dash(value: Option<&str>) -> DashStyle {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("dash" | "dashed") => DashStyle::Dash,
        Some("dot" | "dotted") => DashStyle::Dot,
        _ => DashStyle::Solid,
    }
}

fn line_type(value: Option<&str>, x1: f64, y1: f64, x2: f64, y2: f64) -> LineType {
    let value = value.unwrap_or("").to_ascii_lowercase();
    if matches!(
        value.as_str(),
        "elbow" | "angle" | "angled" | "anglebracket"
    ) || value.starts_with("bentconnector")
    {
        return LineType::Elbow;
    }
    if matches!(value.as_str(), "straight" | "line" | "lineinv")
        || value.starts_with("straightconnector")
    {
        return LineType::Straight;
    }
    if (x2 - x1).abs() > 0.5 && (y2 - y1).abs() > 0.5 {
        LineType::Elbow
    } else {
        LineType::Straight
    }
}
fn elbow_direction(
    value: Option<&str>,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) -> super::ElbowDirection {
    use super::ElbowDirection::*;
    let value = value
        .unwrap_or("")
        .to_lowercase()
        .chars()
        .filter(|c| !super::retrieval::js_whitespace(*c) && *c != '_' && *c != '-')
        .collect::<String>();
    match value.as_str() {
        "horizontal" | "horizontalfirst" | "xfirst" => HorizontalFirst,
        "vertical" | "verticalfirst" | "yfirst" => VerticalFirst,
        _ if (x2 - x1).abs() >= (y2 - y1).abs() => VerticalFirst,
        _ => HorizontalFirst,
    }
}

fn arrow(value: Option<&str>) -> Option<ArrowType> {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("triangle") => Some(ArrowType::Triangle),
        Some("arrow") => Some(ArrowType::Arrow),
        Some("diamond") => Some(ArrowType::Diamond),
        Some("oval") => Some(ArrowType::Oval),
        Some("stealth") => Some(ArrowType::Stealth),
        _ => None,
    }
}

fn image_fit(value: Option<&str>) -> ImageFit {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("cover") => ImageFit::Cover,
        Some("stretch") => ImageFit::Stretch,
        _ => ImageFit::Contain,
    }
}

fn image_crop(value: Option<&Value>) -> Option<ImageCrop> {
    let crop = value?.as_object()?;
    let result = ImageCrop {
        top: clamp01(number(crop.get("top"), 0.0)),
        right: clamp01(number(crop.get("right"), 0.0)),
        bottom: clamp01(number(crop.get("bottom"), 0.0)),
        left: clamp01(number(crop.get("left"), 0.0)),
    };
    (result.top + result.right + result.bottom + result.left > 0.0).then_some(result)
}

fn shape_name(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "rectangle" | "shape" => "rect",
        "roundedrectangle" | "rounded-rectangle" => "roundRect",
        "circle" | "oval" => "ellipse",
        "database" | "cylinder" | "flowchartmagneticdisk" => "flowChartMagneticDisk",
        _ => value,
    }
    .to_owned()
}

fn clamp01(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn invalid_presentation() -> SlideTemplateError {
    SlideTemplateError::new(
        FailureKind::Invalid,
        "invalid_presentation_json",
        "The JSON could not be converted into a PowerPoint presentation.",
    )
}

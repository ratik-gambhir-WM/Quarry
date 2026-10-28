//! The reference exposes compact imported documents and fully normalized built-in documents.
use super::{CanvasDocument, Element};
use serde_json::{json, Value};

pub fn compact_document(document: &CanvasDocument) -> Value {
    let mut value = document_value(document);
    for slide in value["presentation"]["slides"]
        .as_array_mut()
        .into_iter()
        .flatten()
    {
        for key in ["width", "height"] {
            round_number(&mut slide[key]);
        }
        for element in slide["elements"].as_array_mut().into_iter().flatten() {
            let kind = element["type"].as_str().unwrap_or("").to_owned();
            let has_text = element["text"]
                .as_str()
                .is_some_and(|text| !text.is_empty());
            let map = element.as_object_mut().expect("canvas element is object");
            for key in [
                "x",
                "y",
                "w",
                "h",
                "x1",
                "y1",
                "x2",
                "y2",
                "strokeWidth",
                "fontSize",
                "padding",
                "borderRadius",
            ] {
                if let Some(value) = map.get_mut(key) {
                    round_number(value);
                }
            }
            for (key, default) in [
                ("opacity", 1.0),
                ("rotate", 0.0),
                ("fillOpacity", 1.0),
                ("strokeOpacity", 1.0),
                ("borderRadius", 0.0),
            ] {
                if map.get(key).and_then(Value::as_f64) == Some(default) {
                    map.remove(key);
                }
            }
            map.remove("bendPosition");
            if kind == "line" {
                if map.get("lineType") == Some(&json!("straight")) {
                    map.remove("elbowDirection");
                }
                if map.get("dash") == Some(&json!("solid")) {
                    map.remove("dash");
                }
            }
            if kind == "text" || kind == "shape" {
                if !has_text {
                    map.remove("text");
                }
                if kind == "shape" && !has_text {
                    for key in [
                        "padding",
                        "align",
                        "valign",
                        "fontSize",
                        "fontFace",
                        "bold",
                        "textColor",
                    ] {
                        map.remove(key);
                    }
                }
                if map.get("padding") == Some(&json!(8.0)) {
                    map.remove("padding");
                }
                if map.get("align") == Some(&json!("left")) {
                    map.remove("align");
                }
                if map.get("valign") == Some(&json!("middle")) {
                    map.remove("valign");
                }
                if map
                    .get("runs")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
                {
                    map.remove("runs");
                }
            }
            if let Some(runs) = map.get_mut("runs").and_then(Value::as_array_mut) {
                for run in runs {
                    round_number(&mut run["fontSize"]);
                    if run.get("text") == Some(&json!("")) {
                        if let Some(run) = run.as_object_mut() {
                            run.remove("text");
                        }
                    }
                }
            }
            if kind == "image" {
                if map.get("altText") == Some(&json!("")) {
                    map.remove("altText");
                }
                if let Some(crop) = map.get_mut("crop").and_then(Value::as_object_mut) {
                    for value in crop.values_mut() {
                        round_number(value);
                    }
                    crop.retain(|_, v| v.as_f64() != Some(0.0));
                }
            }
        }
    }
    value
}

pub fn normalized_document(document: &CanvasDocument) -> Value {
    let mut value = document_value(document);
    for slide in value["presentation"]["slides"]
        .as_array_mut()
        .into_iter()
        .flatten()
    {
        for element in slide["elements"].as_array_mut().into_iter().flatten() {
            let kind = element["type"].as_str().unwrap_or("").to_owned();
            let map = element.as_object_mut().expect("canvas element is object");
            map.remove("bendPosition");
            for (key, default) in [("opacity", json!(1)), ("rotate", json!(0))] {
                map.entry(key).or_insert(default);
            }
            if kind == "line" {
                map.entry("strokeOpacity").or_insert(json!(1));
                if map.get("lineType") == Some(&json!("straight")) {
                    map.remove("elbowDirection");
                }
            } else {
                if kind == "image" {
                    map.entry("altText").or_insert(json!(""));
                }
                for (key, default) in [
                    ("flipH", json!(false)),
                    ("flipV", json!(false)),
                    ("borderRadius", json!(0)),
                ] {
                    map.entry(key).or_insert(default);
                }
                if kind == "text" || kind == "shape" {
                    for (key, default) in [
                        ("fillOpacity", json!(1)),
                        ("strokeOpacity", json!(1)),
                        ("bold", json!(false)),
                    ] {
                        map.entry(key).or_insert(default);
                    }
                    if kind == "text" {
                        map.entry("italic").or_insert(json!(false));
                    }
                    for run in map
                        .get_mut("runs")
                        .and_then(Value::as_array_mut)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(run) = run.as_object_mut() {
                            for key in ["bold", "italic", "underline", "breakLine"] {
                                run.entry(key).or_insert(json!(false));
                            }
                        }
                    }
                }
            }
        }
    }
    value
}
fn round_number(value: &mut Value) {
    if let Some(number) = value.as_f64() {
        let scaled = number * 100.0;
        let floor = scaled.floor();
        let rounded = if scaled - floor < 0.5 {
            floor
        } else {
            floor + 1.0
        } / 100.0;
        *value = if rounded == 0.0 {
            json!(0)
        } else {
            json!(rounded)
        };
    }
}

/// Preserve exact non-default floating values that the model's tolerant serde omissions may hide.
fn document_value(document: &CanvasDocument) -> Value {
    let mut value = serde_json::to_value(document).expect("finite canvas values");
    for (slide, source) in value["presentation"]["slides"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .zip(&document.presentation.slides)
    {
        for (value, source) in slide["elements"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .zip(&source.elements)
        {
            let fields: Vec<(&str, f64)> = match source {
                Element::Text(e) => vec![
                    ("opacity", e.opacity),
                    ("rotate", e.rotate),
                    ("borderRadius", e.border_radius),
                    ("fillOpacity", e.fill_opacity),
                    ("strokeOpacity", e.stroke_opacity),
                ],
                Element::Shape(e) => vec![
                    ("opacity", e.opacity),
                    ("rotate", e.rotate),
                    ("borderRadius", e.border_radius),
                    ("fillOpacity", e.fill_opacity),
                    ("strokeOpacity", e.stroke_opacity),
                ],
                Element::Line(e) => vec![
                    ("opacity", e.opacity),
                    ("rotate", e.rotate),
                    ("strokeOpacity", e.stroke_opacity),
                ],
                Element::Image(e) => vec![
                    ("opacity", e.opacity),
                    ("rotate", e.rotate),
                    ("borderRadius", e.border_radius),
                ],
            };
            for (key, number) in fields {
                value[key] = json!(number);
            }
        }
    }
    normalize_zero(&mut value);
    value
}

fn normalize_zero(value: &mut Value) {
    match value {
        Value::Number(number) if number.as_f64() == Some(0.0) => *value = json!(0),
        Value::Array(values) => values.iter_mut().for_each(normalize_zero),
        Value::Object(values) => values.values_mut().for_each(normalize_zero),
        _ => {}
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/templates/slide_templates/json_contract_tests.rs"]
mod tests;

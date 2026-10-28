use super::super::{CanvasDocument, StoredTemplatePreview, TemplateKind};
use super::metadata::{normalize_whitespace, utf16_len};
use super::{
    SlideRetrievalMetadata, SLIDE_CLASSIFICATION_PROMPT_VERSION,
    SLIDE_CLASSIFICATION_SCHEMA_VERSION,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

pub const SLIDE_CLASSIFICATION_INPUT_BUILDER_VERSION: u32 = 2;
pub const SLIDE_EMBEDDING_DOCUMENT_VERSION: u32 = 2;

#[derive(Clone, Debug)]
pub struct SlideClassificationInput {
    pub digest: String,
    pub preview: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct SlideClassificationInputLimits {
    pub max_digest_bytes: usize,
    pub max_elements: usize,
    pub max_preview_bytes: usize,
    pub max_preview_dimension: u32,
    pub max_text_chars: usize,
}

impl Default for SlideClassificationInputLimits {
    fn default() -> Self {
        Self {
            max_digest_bytes: 32_000,
            max_elements: 250,
            max_preview_bytes: 5 * 1024 * 1024,
            max_preview_dimension: 4096,
            max_text_chars: 12_000,
        }
    }
}

/// Build the same bounded digest as the reference without including asset URLs or binary image data.
pub fn build_slide_classification_input(
    document: &CanvasDocument,
    kind: TemplateKind,
    title: Option<&str>,
    preview: Option<&StoredTemplatePreview>,
    limits: &SlideClassificationInputLimits,
) -> anyhow::Result<SlideClassificationInput> {
    anyhow::ensure!(
        document.presentation.slides.len() == 1,
        "Classification input requires exactly one slide."
    );
    let slide = &document.presentation.slides[0];
    let values: Vec<_> = slide
        .elements
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<_, _>>()?;
    let mut counts = BTreeMap::<&str, usize>::new();
    for value in &values {
        *counts.entry(string(value, "type")).or_default() += 1;
    }
    let visible = &values[..values.len().min(limits.max_elements)];
    let mut xs = HashSet::new();
    let mut ys = HashSet::new();
    let mut connections = 0;
    for element in visible {
        if string(element, "type") == "line" {
            if element.get("beginArrow").is_some() || element.get("endArrow").is_some() {
                connections += 1;
            }
        } else {
            xs.insert(((number(element, "x") / slide.width * 20.0) + 0.5).floor() as i64);
            ys.insert(((number(element, "y") / slide.height * 20.0) + 0.5).floor() as i64);
        }
    }
    let counts = counts
        .iter()
        .map(|(kind, count)| format!("{kind}={count}"))
        .collect::<Vec<_>>()
        .join(" | ");
    let mut lines = vec![
        format!(
            "title: {}",
            normalize_whitespace(title.unwrap_or(&document.presentation.title))
        ),
        format!("kind: {}", kind.as_str()),
        format!("slide: {}", normalize_whitespace(&slide.name)),
        format!(
            "dimensions: {} x {}",
            round(slide.width),
            round(slide.height)
        ),
        format!(
            "background: {}",
            normalize_whitespace(&slide.background_color)
        ),
        format!(
            "element counts: {}",
            if counts.is_empty() { "none" } else { &counts }
        ),
        format!(
            "layout hints: x-buckets={} | y-buckets={} | connected-lines={connections}",
            xs.len(),
            ys.len()
        ),
    ];
    let mut remaining = limits.max_text_chars;
    for (index, element) in visible.iter().enumerate() {
        let kind = string(element, "type");
        let geometry = if kind == "line" {
            ["x1", "y1", "x2", "y2"]
        } else {
            ["x", "y", "w", "h"]
        };
        let ratios = [slide.width, slide.height, slide.width, slide.height];
        let box_text = geometry
            .iter()
            .zip(ratios)
            .map(|(key, dimension)| round(number(element, key) / dimension))
            .collect::<Vec<_>>()
            .join(",");
        let identifier = serde_json::to_string(element.get("id").unwrap_or(&Value::Null))?;
        let description = match kind {
            "line" => format!(
                "line; route={}; arrows={}->{}; box={box_text}",
                string(element, "lineType"),
                element
                    .get("beginArrow")
                    .and_then(Value::as_str)
                    .unwrap_or("none"),
                element
                    .get("endArrow")
                    .and_then(Value::as_str)
                    .unwrap_or("none")
            ),
            "image" => {
                let alt = normalize_whitespace(string(element, "altText"));
                format!(
                    "image; alt={}; fit={}; crop={}; box={box_text}",
                    if alt.is_empty() { "none" } else { &alt },
                    string(element, "fit"),
                    stable_stringify(element.get("crop").unwrap_or(&serde_json::json!({})))
                )
            }
            _ => {
                let runs = element["runs"]
                    .as_array()
                    .map(|runs| {
                        runs.iter()
                            .filter_map(|r| r["text"].as_str())
                            .collect::<String>()
                    })
                    .unwrap_or_default();
                let text = normalize_whitespace(element["text"].as_str().unwrap_or(&runs));
                let text = truncate_utf16(&text, remaining);
                remaining = remaining.saturating_sub(utf16_len(&text));
                let shape = if kind == "shape" {
                    format!("; shape={}", normalize_whitespace(string(element, "shape")))
                } else {
                    String::new()
                };
                format!(
                    "{kind}{shape}; text={}; box={box_text}",
                    if text.is_empty() { "none" } else { &text }
                )
            }
        };
        lines.push(format!(
            "element {}: id={identifier}; {description}",
            index + 1
        ));
    }
    if values.len() > visible.len() {
        lines.push(format!(
            "truncated elements: {}",
            values.len() - visible.len()
        ));
    }
    let digest = truncate_utf8(&lines.join("\n"), limits.max_digest_bytes);
    anyhow::ensure!(
        !digest.trim().is_empty(),
        "Classification digest cannot be empty."
    );
    let preview = preview
        .filter(|p| {
            p.content_type == "image/png"
                && !p.bytes.is_empty()
                && p.bytes.len() <= limits.max_preview_bytes
                && p.width > 0
                && p.height > 0
                && p.width <= limits.max_preview_dimension
                && p.height <= limits.max_preview_dimension
        })
        .map(|p| p.bytes.clone());
    Ok(SlideClassificationInput { digest, preview })
}

pub fn build_slide_classification_input_fingerprint(
    document: &CanvasDocument,
    preview_checksum: Option<&str>,
) -> anyhow::Result<String> {
    let canonical = stable_stringify(&super::super::compact_document(document));
    Ok(checksum_bytes(format!("{SLIDE_CLASSIFICATION_SCHEMA_VERSION}\n{SLIDE_CLASSIFICATION_PROMPT_VERSION}\n{SLIDE_CLASSIFICATION_INPUT_BUILDER_VERSION}\n{}\n{canonical}",preview_checksum.unwrap_or("")).as_bytes()))
}

pub fn checksum_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub struct SlideEmbeddingDocuments {
    pub subject: String,
    pub capability: String,
}

/// Project the subject represented by a slide separately from the reusable template capability.
pub fn build_slide_embedding_documents(
    title: &str,
    kind: TemplateKind,
    metadata: &SlideRetrievalMetadata,
    maximum_bytes: usize,
) -> anyhow::Result<SlideEmbeddingDocuments> {
    let domain_topics = metadata
        .subject
        .domains
        .iter()
        .map(|domain| {
            format!(
                "{} ({}): {}",
                domain.id,
                domain.relevance,
                if domain.topics.is_empty() {
                    "general".to_owned()
                } else {
                    domain.topics.join(" | ")
                }
            )
        })
        .collect::<Vec<_>>();
    let subject = build_embedding_document(
        &[
            ("title", vec![title]),
            ("summary", vec![&metadata.subject.summary]),
            (
                "domain topics",
                domain_topics.iter().map(String::as_str).collect(),
            ),
            ("other topics", strings(&metadata.subject.other_topics)),
            ("claims", strings(&metadata.subject.claims)),
            ("technologies", strings(&metadata.subject.technologies)),
            ("entities", strings(&metadata.subject.entities)),
            ("synonyms", strings(&metadata.subject.synonyms)),
            ("retrieval keywords", strings(&metadata.retrieval_keywords)),
        ],
        maximum_bytes,
    )?;
    let slots = metadata
        .template_fit
        .content_slots
        .iter()
        .map(|slot| format!("{}: {}", slot.role, slot.capacity))
        .collect::<Vec<_>>();
    let capability = build_embedding_document(
        &[
            ("kind", vec![kind.as_str()]),
            (
                "communication intents",
                strings(&metadata.communication.intents),
            ),
            (
                "information types",
                strings(&metadata.communication.information_types),
            ),
            ("audience", strings(&metadata.communication.audience)),
            ("archetype", vec![&metadata.template_fit.archetype]),
            ("content slots", slots.iter().map(String::as_str).collect()),
            ("layout", vec![&metadata.visual.layout_type]),
            ("visual elements", strings(&metadata.visual.visual_elements)),
            ("content density", vec![&metadata.visual.content_density]),
            (
                "structural features",
                strings(&metadata.visual.structural_features),
            ),
        ],
        maximum_bytes,
    )?;
    Ok(SlideEmbeddingDocuments {
        subject,
        capability,
    })
}

fn build_embedding_document(
    fields: &[(&str, Vec<&str>)],
    maximum_bytes: usize,
) -> anyhow::Result<String> {
    let mut lines = Vec::new();
    for (label, values) in fields {
        let mut seen = HashSet::new();
        let normalized = values
            .iter()
            .map(|value| normalize_whitespace(value))
            .filter(|v| !v.is_empty() && seen.insert(v.to_lowercase()))
            .collect::<Vec<_>>()
            .join(" | ");
        if !normalized.is_empty() {
            lines.push(format!("{label}: {normalized}"));
        }
    }
    let document = lines.join("\n");
    anyhow::ensure!(!document.is_empty(), "Embedding document cannot be empty.");
    anyhow::ensure!(
        document.len() <= maximum_bytes,
        "Embedding document exceeds the configured byte limit."
    );
    Ok(document)
}

pub fn build_slide_embedding_fingerprint(
    role: &str,
    document: &str,
    model: &str,
    dimensions: Option<usize>,
) -> String {
    checksum_bytes(
        format!(
            "{SLIDE_EMBEDDING_DOCUMENT_VERSION}\n{role}\n{model}\n{}\n{document}",
            dimensions.map(|n| n.to_string()).unwrap_or_default()
        )
        .as_bytes(),
    )
}

fn strings(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or_default()
}
fn number(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap_or(0.0)
}
fn round(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    if value.abs() >= 1e21 {
        return js_number(value);
    }
    // toFixed rounds the exact represented binary value, before decimal scaling.
    // Multiplying a float by 10000 first can introduce a new tie and change the digest.
    let bits = value.abs().to_bits();
    let encoded_exponent = ((bits >> 52) & 0x7ff) as i32;
    let significand = (bits & ((1_u64 << 52) - 1))
        | if encoded_exponent == 0 {
            0
        } else {
            1_u64 << 52
        };
    let exponent = if encoded_exponent == 0 {
        -1074
    } else {
        encoded_exponent - 1023 - 52
    };
    let numerator = u128::from(significand) * 625;
    let shift = exponent + 4;
    let integer = if shift >= 0 {
        numerator << shift
    } else if -shift >= 128 {
        0
    } else {
        let denominator = 1_u128 << -shift;
        numerator / denominator + u128::from(numerator % denominator >= denominator / 2)
    };
    let decimal = format!("{}.{:04}", integer / 10_000, integer % 10_000);
    let rounded = decimal
        .parse::<f64>()
        .expect("finite decimal digits")
        .copysign(value);
    js_number(rounded)
}

fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_owned();
    }
    if value.abs() >= 1e21 || value.abs() < 1e-6 {
        let scientific = format!("{value:e}");
        let (mantissa, exponent) = scientific
            .split_once('e')
            .expect("scientific format exponent");
        let exponent: i32 = exponent
            .parse()
            .expect("scientific format numeric exponent");
        format!("{mantissa}e{exponent:+}")
    } else {
        value.to_string()
    }
}
fn truncate_utf8(value: &str, maximum: usize) -> String {
    let mut end = value.len().min(maximum);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
fn truncate_utf16(value: &str, maximum: usize) -> String {
    let units: Vec<_> = value.encode_utf16().take(maximum).collect();
    String::from_utf16_lossy(&units)
}
pub fn stable_stringify(value: &Value) -> String {
    match value {
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(stable_stringify)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(values) => {
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            format!(
                "{{{}}}",
                keys.into_iter()
                    .map(|key| format!(
                        "{}:{}",
                        Value::String(key.clone()),
                        stable_stringify(&values[key])
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Number(n) => js_number(n.as_f64().unwrap_or_default()),
        _ => value.to_string(),
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/retrieval/input_tests.rs"]
mod tests;

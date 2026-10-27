//! Versioned, normalized metadata used to retrieve reusable slide templates.

use std::collections::HashSet;

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SLIDE_CLASSIFICATION_SCHEMA_VERSION: u32 = 2;
pub const SLIDE_CLASSIFICATION_PROMPT_VERSION: &str = "2026-09-14.2";

pub const SLIDE_DOMAIN_TAXONOMY: &[&str] = &[
    "cybersecurity",
    "software-architecture",
    "data-ai",
    "infrastructure",
    "cloud",
    "product-engineering",
    "it-operations",
    "business-applications",
    "governance-risk-compliance",
    "other",
];
pub const COMMUNICATION_INTENT_TAXONOMY: &[&str] = &[
    "finding",
    "evidence",
    "risk",
    "recommendation",
    "current-state",
    "target-state",
    "comparison",
    "process",
    "timeline",
    "metric",
    "explanation",
];
pub const SLIDE_LAYOUT_TAXONOMY: &[&str] = &[
    "title-only",
    "title-body",
    "single-column",
    "two-column",
    "three-column",
    "grid",
    "flow",
    "timeline",
    "chart-led",
    "table-led",
    "diagram-led",
    "mixed",
];
pub const CONTENT_SLOT_CAPACITY_TAXONOMY: &[&str] = &[
    "short-text",
    "paragraph",
    "three-to-five-bullets",
    "number",
    "table",
    "diagram-label",
    "image",
    "chart",
    "flexible",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideRetrievalMetadata {
    pub subject: SlideSubject,
    pub communication: SlideCommunication,
    pub template_fit: SlideTemplateFit,
    pub visual: SlideVisual,
    pub retrieval_keywords: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideSubject {
    pub summary: String,
    pub domains: Vec<SlideDomain>,
    pub other_topics: Vec<String>,
    pub technologies: Vec<String>,
    pub entities: Vec<String>,
    pub claims: Vec<String>,
    pub synonyms: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideDomain {
    pub id: String,
    pub relevance: String,
    pub topics: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideCommunication {
    pub intents: Vec<String>,
    pub information_types: Vec<String>,
    pub audience: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideTemplateFit {
    pub archetype: String,
    pub content_slots: Vec<SlideContentSlot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideContentSlot {
    pub element_id: String,
    pub role: String,
    pub capacity: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideVisual {
    pub layout_type: String,
    pub visual_elements: Vec<String>,
    pub content_density: String,
    pub structural_features: Vec<String>,
}

/// Parse untrusted classifier output and apply the reference's canonical normalization.
pub fn normalize_slide_retrieval_metadata(value: Value) -> anyhow::Result<SlideRetrievalMetadata> {
    let parsed: SlideRetrievalMetadata = serde_json::from_value(value)?;
    validate_metadata(&parsed)?;
    let normalized = SlideRetrievalMetadata {
        subject: SlideSubject {
            summary: normalize_whitespace(&parsed.subject.summary),
            domains: parsed
                .subject
                .domains
                .iter()
                .map(|domain| SlideDomain {
                    id: domain.id.clone(),
                    relevance: domain.relevance.clone(),
                    topics: deduplicate(
                        domain
                            .topics
                            .iter()
                            .map(|topic| normalize_taxonomy_value(topic)),
                    ),
                })
                .collect(),
            other_topics: deduplicate(
                parsed
                    .subject
                    .other_topics
                    .iter()
                    .map(|topic| normalize_taxonomy_value(topic)),
            ),
            technologies: deduplicate(
                parsed
                    .subject
                    .technologies
                    .iter()
                    .map(|text| normalize_whitespace(text)),
            ),
            entities: deduplicate(
                parsed
                    .subject
                    .entities
                    .iter()
                    .map(|text| normalize_whitespace(text)),
            ),
            claims: deduplicate(
                parsed
                    .subject
                    .claims
                    .iter()
                    .map(|text| normalize_whitespace(text)),
            ),
            synonyms: deduplicate(
                parsed
                    .subject
                    .synonyms
                    .iter()
                    .map(|text| normalize_whitespace(text)),
            ),
        },
        communication: SlideCommunication {
            intents: deduplicate(parsed.communication.intents.iter().cloned()),
            information_types: deduplicate(
                parsed
                    .communication
                    .information_types
                    .iter()
                    .map(|text| normalize_taxonomy_value(text)),
            ),
            audience: deduplicate(
                parsed
                    .communication
                    .audience
                    .iter()
                    .map(|text| normalize_whitespace(text)),
            ),
        },
        template_fit: SlideTemplateFit {
            archetype: normalize_taxonomy_value(&parsed.template_fit.archetype),
            content_slots: parsed
                .template_fit
                .content_slots
                .iter()
                .map(|slot| SlideContentSlot {
                    element_id: normalize_whitespace(&slot.element_id),
                    role: normalize_taxonomy_value(&slot.role),
                    capacity: slot.capacity.clone(),
                })
                .collect(),
        },
        visual: SlideVisual {
            layout_type: parsed.visual.layout_type.clone(),
            visual_elements: deduplicate(
                parsed
                    .visual
                    .visual_elements
                    .iter()
                    .map(|text| normalize_taxonomy_value(text)),
            ),
            content_density: parsed.visual.content_density.clone(),
            structural_features: deduplicate(
                parsed
                    .visual
                    .structural_features
                    .iter()
                    .map(|text| normalize_whitespace(text)),
            ),
        },
        retrieval_keywords: deduplicate(
            parsed
                .retrieval_keywords
                .iter()
                .map(|text| normalize_whitespace(text)),
        ),
    };
    // Zod parses again after transforming, so canonicalization cannot create invalid duplicates.
    validate_metadata(&normalized)?;
    Ok(normalized)
}

/// Reject model-proposed slots that do not correspond to editable slide elements.
pub fn assert_content_slots_exist(
    metadata: &SlideRetrievalMetadata,
    element_ids: &HashSet<String>,
) -> anyhow::Result<()> {
    ensure!(
        metadata
            .template_fit
            .content_slots
            .iter()
            .all(|slot| element_ids.contains(&slot.element_id)),
        "Classification content slots must reference stored slide elements."
    );
    Ok(())
}

fn validate_metadata(metadata: &SlideRetrievalMetadata) -> anyhow::Result<()> {
    required_text(&metadata.subject.summary, 1_500)?;
    ensure!(
        (1..=8).contains(&metadata.subject.domains.len()),
        "invalid subject domains"
    );
    let mut domain_ids = HashSet::new();
    let mut primary_domains = 0_usize;
    for domain in &metadata.subject.domains {
        ensure!(
            SLIDE_DOMAIN_TAXONOMY.contains(&domain.id.as_str()),
            "invalid domain"
        );
        ensure!(
            matches!(domain.relevance.as_str(), "primary" | "secondary"),
            "invalid domain relevance"
        );
        primary_domains += usize::from(domain.relevance == "primary");
        ensure!(
            domain_ids.insert(domain.id.clone()),
            "Subject domains must be unique."
        );
        validate_list(&domain.topics, 16, 120)?;
    }
    ensure!(
        primary_domains == 1,
        "Exactly one subject domain must be primary."
    );
    validate_list(&metadata.subject.other_topics, 16, 120)?;
    validate_list(&metadata.subject.technologies, 20, 120)?;
    validate_list(&metadata.subject.entities, 20, 160)?;
    validate_list(&metadata.subject.claims, 16, 300)?;
    validate_list(&metadata.subject.synonyms, 24, 120)?;

    ensure!(
        (1..=8).contains(&metadata.communication.intents.len()),
        "invalid communication intents"
    );
    for intent in &metadata.communication.intents {
        ensure!(
            COMMUNICATION_INTENT_TAXONOMY.contains(&intent.as_str()),
            "invalid communication intent"
        );
    }
    validate_list(&metadata.communication.information_types, 16, 120)?;
    validate_list(&metadata.communication.audience, 12, 120)?;

    required_text(&metadata.template_fit.archetype, 120)?;
    ensure!(
        metadata.template_fit.content_slots.len() <= 40,
        "too many content slots"
    );
    let mut slot_ids = HashSet::new();
    for slot in &metadata.template_fit.content_slots {
        required_text(&slot.element_id, 200)?;
        required_text(&slot.role, 100)?;
        ensure!(
            CONTENT_SLOT_CAPACITY_TAXONOMY.contains(&slot.capacity.as_str()),
            "invalid content slot capacity"
        );
        ensure!(
            slot_ids.insert(slot.element_id.clone()),
            "Content slots must reference unique element IDs."
        );
    }
    ensure!(
        SLIDE_LAYOUT_TAXONOMY.contains(&metadata.visual.layout_type.as_str()),
        "invalid slide layout"
    );
    ensure!(
        matches!(
            metadata.visual.content_density.as_str(),
            "low" | "medium" | "high"
        ),
        "invalid content density"
    );
    validate_list(&metadata.visual.visual_elements, 20, 120)?;
    validate_list(&metadata.visual.structural_features, 20, 160)?;
    validate_list(&metadata.retrieval_keywords, 30, 120)?;
    Ok(())
}

fn required_text(value: &str, maximum: usize) -> anyhow::Result<()> {
    let trimmed = js_trim(value);
    ensure!(
        !trimmed.is_empty() && utf16_len(trimmed) <= maximum,
        "invalid metadata text length"
    );
    Ok(())
}

fn validate_list(
    values: &[String],
    maximum_items: usize,
    maximum_item_length: usize,
) -> anyhow::Result<()> {
    ensure!(values.len() <= maximum_items, "too many metadata items");
    for value in values {
        required_text(value, maximum_item_length)?;
    }
    Ok(())
}

/// Convert an evolving classifier label to stable lower-case ASCII kebab case.
pub fn normalize_taxonomy_value(value: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in normalize_whitespace(value).to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            if separator && !result.is_empty() {
                result.push('-');
            }
            result.push(character);
            separator = false;
        } else {
            separator = true;
        }
    }
    if result.is_empty() {
        "other".to_owned()
    } else {
        result
    }
}

pub(super) fn normalize_whitespace(value: &str) -> String {
    value
        .split(js_whitespace)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn deduplicate(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|value| seen.insert(value.to_lowercase()))
        .collect()
}

pub(crate) fn js_trim(value: &str) -> &str {
    value.trim_matches(js_whitespace)
}

pub(crate) fn js_whitespace(value: char) -> bool {
    matches!(value, '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

pub(super) fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Strict structured-output schema for the Responses API.
pub fn slide_retrieval_metadata_schema() -> Value {
    let text = |maximum| json!({"type":"string","minLength":1,"maxLength":maximum});
    let list = |maximum_items, maximum_length| json!({"type":"array","maxItems":maximum_items,"items":text(maximum_length)});
    let domain = json!({"type":"object","additionalProperties":false,"properties":{
        "id":{"type":"string","enum":SLIDE_DOMAIN_TAXONOMY}, "relevance":{"type":"string","enum":["primary","secondary"]}, "topics":list(16,120)
    },"required":["id","relevance","topics"]});
    let slot = json!({"type":"object","additionalProperties":false,"properties":{
        "element_id":text(200), "role":text(100), "capacity":{"type":"string","enum":CONTENT_SLOT_CAPACITY_TAXONOMY}
    },"required":["element_id","role","capacity"]});
    json!({"$schema":"http://json-schema.org/draft-07/schema#","type":"object","additionalProperties":false,"properties":{
        "subject":{"type":"object","additionalProperties":false,"properties":{
            "summary":text(1500), "domains":{"type":"array","minItems":1,"maxItems":8,"items":domain}, "other_topics":list(16,120), "technologies":list(20,120), "entities":list(20,160), "claims":list(16,300), "synonyms":list(24,120)
        },"required":["summary","domains","other_topics","technologies","entities","claims","synonyms"]},
        "communication":{"type":"object","additionalProperties":false,"properties":{
            "intents":{"type":"array","minItems":1,"maxItems":8,"items":{"type":"string","enum":COMMUNICATION_INTENT_TAXONOMY}}, "information_types":list(16,120), "audience":list(12,120)
        },"required":["intents","information_types","audience"]},
        "template_fit":{"type":"object","additionalProperties":false,"properties":{"archetype":text(120),"content_slots":{"type":"array","maxItems":40,"items":slot}},"required":["archetype","content_slots"]},
        "visual":{"type":"object","additionalProperties":false,"properties":{"layout_type":{"type":"string","enum":SLIDE_LAYOUT_TAXONOMY},"visual_elements":list(20,120),"content_density":{"type":"string","enum":["low","medium","high"]},"structural_features":list(20,160)},"required":["layout_type","visual_elements","content_density","structural_features"]},
        "retrieval_keywords":list(30,120)
    },"required":["subject","communication","template_fit","visual","retrieval_keywords"]})
}

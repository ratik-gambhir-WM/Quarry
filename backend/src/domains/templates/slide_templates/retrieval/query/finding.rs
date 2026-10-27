//! Finding-oriented retrieval built on the common query ranking primitives.

use super::{
    all_topics, compatible_vectors, equal, percent_encode_component, rank_by_vector,
    reciprocal_rank_scores, SlideRetrievalError, SlideRetrievalService, VectorRole,
};
use crate::domains::templates::slide_templates::retrieval::RetrievalClassificationRecord;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Clone, Debug)]
struct FindingAnalysis {
    domains: Vec<String>,
    intents: Vec<String>,
    required_slot_roles: Vec<String>,
    topics: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FindingInput {
    markdown: String,
    #[serde(default = "default_finding_limit")]
    limit: usize,
}

fn default_finding_limit() -> usize {
    5
}

fn parse_finding_input(value: &Value) -> Result<FindingInput, SlideRetrievalError> {
    let mut input: FindingInput = serde_json::from_value(value.clone())
        .map_err(|_| SlideRetrievalError::new("invalid_query"))?;
    input.markdown = super::js_trim(&input.markdown).to_owned();
    if input.markdown.is_empty()
        || super::utf16_len(&input.markdown) > 20_000
        || !(1..=20).contains(&input.limit)
    {
        return Err(SlideRetrievalError::new("invalid_query"));
    }
    Ok(input)
}

/// Retrieve templates for a complete finding using content and layout signals independently.
pub(super) async fn find_for_finding(
    retrieval: &SlideRetrievalService,
    app_id: &str,
    untrusted_input: &Value,
) -> anyhow::Result<Value> {
    let input = parse_finding_input(untrusted_input)?;
    if retrieval.embedder.is_none() || retrieval.embedding_model.is_none() {
        return Err(SlideRetrievalError::new("semantic_unavailable").into());
    }
    let repository = retrieval.repository.clone();
    let app = app_id.to_owned();
    let records =
        tokio::task::spawn_blocking(move || repository.list_retrieval_classifications(&app))
            .await??;
    let analysis = analyze_finding(&input.markdown, &records);
    let compatible: Vec<_> = records
        .into_iter()
        .filter(|record| {
            compatible_vectors(
                record,
                retrieval.embedding_model.as_deref(),
                retrieval
                    .embedding_dimensions
                    .or(record.classification.embedding_dimensions),
            )
        })
        .collect();
    if compatible.is_empty() {
        return Ok(json!({"results":[],"warnings":[]}));
    }
    let capability_query = format!(
        "communication intents: {}\nrequired content slots: {}",
        analysis.intents.join(" | "),
        analysis.required_slot_roles.join(" | ")
    );
    let (subject_vector, capability_vector) = tokio::try_join!(
        retrieval.embed(&input.markdown),
        retrieval.embed(&capability_query)
    )?;
    let lexical_terms = analysis
        .domains
        .iter()
        .chain(&analysis.topics)
        .chain(&analysis.intents)
        .cloned()
        .collect::<Vec<_>>();
    let lexical = if lexical_terms.is_empty() {
        Vec::new()
    } else {
        retrieval
            .lexical(app_id, Some(&lexical_terms.join(" ")), &compatible, "OR")
            .await?
    };
    let subject = rank_by_vector(&subject_vector, &compatible, VectorRole::Subject)?;
    let capability = rank_by_vector(&capability_vector, &compatible, VectorRole::Capability)?;
    let mut ranked = reciprocal_rank_scores(&[lexical, subject, capability], 60.0)
        .into_iter()
        .map(|(record, score)| {
            (
                record.clone(),
                score + f64::from(exact_finding_boost(&record, &analysis)) * 0.005,
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_record, left_score), (right_record, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| super::locale_compare(&right_record.created_at, &left_record.created_at))
            .then_with(|| {
                super::locale_compare(&left_record.template_id, &right_record.template_id)
            })
    });
    let results = ranked
        .into_iter()
        .take(input.limit)
        .map(|(record, _)| explain_finding_match(&record, &analysis, app_id))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(json!({"results":results,"warnings":[]}))
}

fn analyze_finding(markdown: &str, records: &[RetrievalClassificationRecord]) -> FindingAnalysis {
    const DOMAIN_TERMS: &[(&str, &[&str])] = &[
        (
            "cybersecurity",
            &[
                "cyber",
                "cybersecurity",
                "security",
                "vulnerability",
                "iam",
                "penetration test",
            ],
        ),
        (
            "software-architecture",
            &[
                "software architecture",
                "application architecture",
                "extensibility",
                "integration",
            ],
        ),
        (
            "data-ai",
            &[
                "data",
                "analytics",
                "artificial intelligence",
                "machine learning",
                "ai",
            ],
        ),
        (
            "infrastructure",
            &[
                "infrastructure",
                "network",
                "server",
                "compute",
                "storage",
                "datacenter",
            ],
        ),
        ("cloud", &["cloud", "aws", "azure", "gcp"]),
        (
            "product-engineering",
            &["product engineering", "sdlc", "developer", "code quality"],
        ),
        (
            "it-operations",
            &[
                "it operations",
                "service management",
                "incident",
                "monitoring",
            ],
        ),
        (
            "business-applications",
            &["erp", "crm", "business application"],
        ),
        (
            "governance-risk-compliance",
            &["governance", "compliance", "regulatory", "risk management"],
        ),
    ];
    let searchable = normalize_search_text(markdown);
    let domains = DOMAIN_TERMS
        .iter()
        .filter(|(_, terms)| terms.iter().any(|term| contains_phrase(&searchable, term)))
        .map(|(domain, _)| (*domain).to_owned())
        .collect();
    let mut intents = super::super::COMMUNICATION_INTENT_TAXONOMY
        .iter()
        .filter(|intent| contains_phrase(&searchable, intent))
        .map(|intent| (*intent).to_owned())
        .collect::<Vec<_>>();
    if intents.is_empty() {
        intents = vec!["finding".into(), "evidence".into(), "recommendation".into()];
    }
    let mut seen_topics = HashSet::new();
    let topics = records
        .iter()
        .filter_map(|record| record.classification.metadata.as_ref())
        .flat_map(all_topics)
        .filter(|topic| seen_topics.insert(topic.clone()) && contains_phrase(&searchable, topic))
        .collect();
    let mut required_slot_roles = vec!["headline".to_owned()];
    for intent in &intents {
        for role in match intent.as_str() {
            "finding" => &["finding"][..],
            "evidence" => &["evidence"],
            "recommendation" => &["recommendation"],
            "comparison" => &["comparison-left", "comparison-right"],
            "metric" => &["metric"],
            "process" => &["process-step"],
            "timeline" => &["timeline-event"],
            _ => &[],
        } {
            if !required_slot_roles.iter().any(|existing| existing == role) {
                required_slot_roles.push((*role).to_owned());
            }
        }
    }
    FindingAnalysis {
        domains,
        intents,
        required_slot_roles,
        topics,
    }
}

fn exact_finding_boost(record: &RetrievalClassificationRecord, analysis: &FindingAnalysis) -> u32 {
    let Some(metadata) = record.classification.metadata.as_ref() else {
        return 0;
    };
    let domain_score = metadata
        .subject
        .domains
        .iter()
        .filter(|domain| analysis.domains.contains(&domain.id))
        .map(|domain| if domain.relevance == "primary" { 6 } else { 3 })
        .sum::<u32>();
    let topics = all_topics(metadata);
    let topic_score = u32::try_from(
        analysis
            .topics
            .iter()
            .filter(|topic| topics.iter().any(|candidate| equal(candidate, topic)))
            .count(),
    )
    .unwrap_or(u32::MAX)
    .saturating_mul(4);
    let intent_score = u32::try_from(
        analysis
            .intents
            .iter()
            .filter(|intent| metadata.communication.intents.contains(*intent))
            .count(),
    )
    .unwrap_or(u32::MAX)
    .saturating_mul(2);
    let slot_roles = metadata
        .template_fit
        .content_slots
        .iter()
        .map(|slot| slot.role.clone())
        .collect::<Vec<_>>();
    let slot_score = u32::try_from(
        analysis
            .required_slot_roles
            .iter()
            .filter(|role| slot_roles.iter().any(|slot| equal(slot, role)))
            .count(),
    )
    .unwrap_or(u32::MAX)
    .saturating_mul(2);
    domain_score + topic_score + intent_score + slot_score
}

fn explain_finding_match(
    record: &RetrievalClassificationRecord,
    analysis: &FindingAnalysis,
    app_id: &str,
) -> anyhow::Result<Value> {
    let metadata = record
        .classification
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Retrieval metadata is missing."))?;
    let matched_domains = metadata
        .subject
        .domains
        .iter()
        .filter(|domain| analysis.domains.contains(&domain.id))
        .map(|domain| domain.id.clone())
        .collect::<Vec<_>>();
    let topics = all_topics(metadata);
    let matched_topics = analysis
        .topics
        .iter()
        .filter(|topic| topics.iter().any(|candidate| equal(candidate, topic)))
        .cloned()
        .collect::<Vec<_>>();
    let matched_intents = analysis
        .intents
        .iter()
        .filter(|intent| metadata.communication.intents.contains(*intent))
        .cloned()
        .collect::<Vec<_>>();
    let slot_roles = metadata
        .template_fit
        .content_slots
        .iter()
        .map(|slot| slot.role.clone())
        .collect::<Vec<_>>();
    let matched_slots = analysis
        .required_slot_roles
        .iter()
        .filter(|role| slot_roles.iter().any(|slot| equal(slot, role)))
        .cloned()
        .collect::<Vec<_>>();
    let mut match_reasons = matched_domains
        .iter()
        .map(|domain| format!("{domain} subject match"))
        .chain(
            matched_topics
                .iter()
                .map(|topic| format!("{topic} topic match")),
        )
        .collect::<Vec<_>>();
    if !matched_intents.is_empty() {
        match_reasons.push(format!(
            "Supports {} communication",
            matched_intents.join(", ")
        ));
    }
    if !matched_slots.is_empty() {
        match_reasons.push(format!(
            "Provides {} content slots",
            matched_slots.join(", ")
        ));
    }
    if match_reasons.is_empty() {
        match_reasons.push("Semantic subject and template-capability similarity".into());
    }
    Ok(
        json!({"kind":record.kind,"matchedDomains":matched_domains,"matchedIntents":matched_intents,"matchedTopics":matched_topics,"matchReasons":match_reasons,"previewUrl":record.preview_available.then(|| format!("/api/v1/templates/{}/preview?appId={}", record.template_id, percent_encode_component(app_id))),"templateId":record.template_id,"title":record.title}),
    )
}

fn normalize_search_text(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_lowercase() || character.is_ascii_digit() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_phrase(normalized_text: &str, phrase: &str) -> bool {
    let normalized_phrase = normalize_search_text(phrase);
    !normalized_phrase.is_empty()
        && format!(" {normalized_text} ").contains(&format!(" {normalized_phrase} "))
}

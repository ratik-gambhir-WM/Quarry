use super::super::{providers::SlideEmbedder, TemplateKind, TemplateRepository};
use super::metadata::{js_trim, normalize_taxonomy_value, utf16_len};
use super::{
    cosine_similarity, validate_embedding, RetrievalClassificationRecord, SlideRetrievalMetadata,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

mod finding;

const FACET_NAMES: &[&str] = &[
    "domains",
    "topics",
    "intents",
    "archetype",
    "layout_type",
    "content_density",
    "slot_roles",
];
const SELECT_NAMES: &[&str] = &[
    "all",
    "subject",
    "communication",
    "template_fit",
    "visual",
    "keywords",
];

#[derive(Debug, thiserror::Error)]
#[error("The slide retrieval request could not be completed.")]
pub struct SlideRetrievalError {
    pub code: &'static str,
}
impl SlideRetrievalError {
    pub const fn new(code: &'static str) -> Self {
        Self { code }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlideQueryFilters {
    pub kinds: Vec<TemplateKind>,
    pub domains: Vec<String>,
    pub topics: Vec<String>,
    pub intents: Vec<String>,
    pub archetypes: Vec<String>,
    pub slot_roles: Vec<String>,
    pub content_density: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlideQueryInput {
    pub mode: String,
    pub query: Option<String>,
    pub semantic_target: String,
    pub template_ids: Vec<String>,
    pub similar_to_template_id: Option<String>,
    pub facet_name: Option<String>,
    pub filters: SlideQueryFilters,
    pub select: Vec<String>,
    pub limit: usize,
}

#[derive(Clone, Copy)]
enum VectorRole {
    Subject,
    Capability,
}

pub fn parse_slide_query_input(value: &Value) -> Result<SlideQueryInput, SlideRetrievalError> {
    let invalid = || SlideRetrievalError::new("invalid_query");
    let required = [
        "mode",
        "query",
        "semantic_target",
        "template_ids",
        "similar_to_template_id",
        "facet_name",
        "filters",
        "select",
        "limit",
    ];
    let filter_required = [
        "kinds",
        "domains",
        "topics",
        "intents",
        "archetypes",
        "slot_roles",
        "content_density",
    ];
    if required.iter().any(|key| value.get(key).is_none())
        || filter_required
            .iter()
            .any(|key| value["filters"].get(key).is_none())
    {
        return Err(invalid());
    }
    let mut input: SlideQueryInput =
        serde_json::from_value(value.clone()).map_err(|_| invalid())?;
    if ![
        "text", "semantic", "hybrid", "filter", "similar", "by_id", "facets",
    ]
    .contains(&input.mode.as_str())
        || !["subject", "capability", "both"].contains(&input.semantic_target.as_str())
        || input
            .query
            .as_ref()
            .is_some_and(|query| utf16_len(query) > 2_000)
        || !(1..=20).contains(&input.limit)
        || input.select.is_empty()
        || input.select.len() > SELECT_NAMES.len()
        || input
            .select
            .iter()
            .any(|select| !SELECT_NAMES.contains(&select.as_str()))
        || input
            .facet_name
            .as_ref()
            .is_some_and(|facet| !FACET_NAMES.contains(&facet.as_str()))
    {
        return Err(invalid());
    }
    let text_mode = matches!(input.mode.as_str(), "text" | "semantic" | "hybrid");
    if text_mode
        != input
            .query
            .as_ref()
            .is_some_and(|query| !js_trim(query).is_empty())
        || (input.mode == "by_id") == input.template_ids.is_empty()
        || (input.mode == "similar") != input.similar_to_template_id.is_some()
        || (input.mode == "facets") != input.facet_name.is_some()
        || (input.select.iter().any(|select| select == "all") && input.select.len() != 1)
    {
        return Err(invalid());
    }
    validate_strings(&mut input.template_ids, 20, 200)?;
    if let Some(id) = input.similar_to_template_id.as_mut() {
        *id = js_trim(id).to_owned();
        if id.is_empty() || utf16_len(id) > 200 {
            return Err(invalid());
        }
    }
    validate_taxonomy(&mut input.filters.domains, super::SLIDE_DOMAIN_TAXONOMY, 10)?;
    validate_strings(&mut input.filters.topics, 20, 120)?;
    validate_taxonomy(
        &mut input.filters.intents,
        super::COMMUNICATION_INTENT_TAXONOMY,
        10,
    )?;
    validate_strings(&mut input.filters.archetypes, 10, 120)?;
    validate_strings(&mut input.filters.slot_roles, 20, 100)?;
    if input.filters.kinds.len() > 2
        || input.filters.content_density.len() > 3
        || input
            .filters
            .content_density
            .iter()
            .any(|density| !["low", "medium", "high"].contains(&density.as_str()))
    {
        return Err(invalid());
    }
    Ok(input)
}

fn validate_strings(
    values: &mut [String],
    maximum: usize,
    length: usize,
) -> Result<(), SlideRetrievalError> {
    if values.len() > maximum {
        return Err(SlideRetrievalError::new("invalid_query"));
    }
    for value in values {
        *value = js_trim(value).to_owned();
        if value.is_empty() || utf16_len(value) > length {
            return Err(SlideRetrievalError::new("invalid_query"));
        }
    }
    Ok(())
}

fn validate_taxonomy(
    values: &mut [String],
    taxonomy: &[&str],
    maximum: usize,
) -> Result<(), SlideRetrievalError> {
    validate_strings(values, maximum, 120)?;
    if values
        .iter()
        .any(|value| !taxonomy.contains(&value.as_str()))
    {
        return Err(SlideRetrievalError::new("invalid_query"));
    }
    Ok(())
}

pub struct SlideRetrievalService {
    pub repository: Arc<dyn TemplateRepository>,
    pub embedder: Option<Arc<dyn SlideEmbedder>>,
    pub embedding_model: Option<String>,
    pub embedding_dimensions: Option<usize>,
}

impl SlideRetrievalService {
    /// Validate and execute one app-scoped retrieval mode without exposing vectors.
    pub async fn query(&self, app_id: &str, untrusted_input: &Value) -> anyhow::Result<Value> {
        let input = parse_slide_query_input(untrusted_input)?;
        let repository = self.repository.clone();
        let app = app_id.to_owned();
        let app_records =
            tokio::task::spawn_blocking(move || repository.list_retrieval_classifications(&app))
                .await??;
        let records = apply_filters(&app_records, &input.filters);
        if input.mode == "facets" {
            return build_facet_response(
                &records,
                input
                    .facet_name
                    .as_deref()
                    .ok_or_else(|| SlideRetrievalError::new("invalid_query"))?,
            );
        }
        let mut warnings = Vec::new();
        let ranked = match input.mode.as_str() {
            "by_id" => input
                .template_ids
                .iter()
                .filter_map(|id| {
                    records
                        .iter()
                        .find(|record| record.template_id == *id)
                        .cloned()
                })
                .collect(),
            "filter" => records,
            "text" => {
                self.lexical(app_id, input.query.as_deref(), &records, "AND")
                    .await?
            }
            "semantic" => {
                self.semantic(input.query.as_deref(), &records, &input.semantic_target)
                    .await?
            }
            "hybrid" => {
                let lexical = self
                    .lexical(app_id, input.query.as_deref(), &records, "AND")
                    .await?;
                match self
                    .semantic(input.query.as_deref(), &records, &input.semantic_target)
                    .await
                {
                    Ok(semantic) if semantic.is_empty() && !records.is_empty() => {
                        warnings.push("semantic_unavailable");
                        lexical
                    }
                    Ok(semantic) => reciprocal_rank_fusion(&lexical, &semantic, 60.0),
                    Err(error)
                        if error
                            .downcast_ref::<SlideRetrievalError>()
                            .is_some_and(|retrieval| retrieval.code == "semantic_unavailable") =>
                    {
                        warnings.push("semantic_unavailable");
                        lexical
                    }
                    Err(error) => return Err(error),
                }
            }
            "similar" => {
                let source = app_records
                    .iter()
                    .find(|record| {
                        Some(&record.template_id) == input.similar_to_template_id.as_ref()
                    })
                    .filter(|record| has_ready_vectors(record))
                    .ok_or_else(|| SlideRetrievalError::new("slide_not_available"))?;
                let candidates: Vec<_> = records
                    .into_iter()
                    .filter(|record| {
                        record.template_id != source.template_id
                            && compatible_vectors(
                                record,
                                source.classification.embedding_model.as_deref(),
                                source.classification.embedding_dimensions,
                            )
                    })
                    .collect();
                let subject = rank_by_vector(
                    source
                        .classification
                        .subject_vector
                        .as_deref()
                        .unwrap_or_default(),
                    &candidates,
                    VectorRole::Subject,
                )?;
                let capability = rank_by_vector(
                    source
                        .classification
                        .capability_vector
                        .as_deref()
                        .unwrap_or_default(),
                    &candidates,
                    VectorRole::Capability,
                )?;
                reciprocal_rank_fusion(&subject, &capability, 60.0)
            }
            _ => return Err(SlideRetrievalError::new("invalid_query").into()),
        };
        let results = ranked
            .iter()
            .take(input.limit)
            .map(|record| project_record(record, &input.select, app_id))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(json!({"mode_used":input.mode,"results":results,"warnings":warnings}))
    }

    /// Retrieve templates for a complete finding using content and layout signals independently.
    pub async fn find_for_finding(
        &self,
        app_id: &str,
        untrusted_input: &Value,
    ) -> anyhow::Result<Value> {
        finding::find_for_finding(self, app_id, untrusted_input).await
    }

    async fn lexical(
        &self,
        app_id: &str,
        query: Option<&str>,
        records: &[RetrievalClassificationRecord],
        operator: &str,
    ) -> anyhow::Result<Vec<RetrievalClassificationRecord>> {
        let expression = build_safe_fts_query(
            query.ok_or_else(|| SlideRetrievalError::new("invalid_query"))?,
            operator,
        )?;
        let available: HashMap<_, _> = records
            .iter()
            .map(|record| (record.template_id.as_str(), record))
            .collect();
        let repository = self.repository.clone();
        let app = app_id.to_owned();
        let ids = tokio::task::spawn_blocking(move || {
            repository.search_slide_classifications(&app, &expression, 200)
        })
        .await??;
        Ok(ids
            .iter()
            .filter_map(|id| available.get(id.as_str()).cloned().cloned())
            .collect())
    }

    async fn semantic(
        &self,
        query: Option<&str>,
        records: &[RetrievalClassificationRecord],
        target: &str,
    ) -> anyhow::Result<Vec<RetrievalClassificationRecord>> {
        let query = query.ok_or_else(|| SlideRetrievalError::new("invalid_query"))?;
        let compatible: Vec<_> = records
            .iter()
            .filter(|record| {
                compatible_vectors(
                    record,
                    self.embedding_model.as_deref(),
                    self.embedding_dimensions
                        .or(record.classification.embedding_dimensions),
                )
            })
            .cloned()
            .collect();
        if compatible.is_empty() {
            return Ok(Vec::new());
        }
        let vector = self.embed(js_trim(query)).await?;
        match target {
            "subject" => rank_by_vector(&vector, &compatible, VectorRole::Subject),
            "capability" => rank_by_vector(&vector, &compatible, VectorRole::Capability),
            "both" => Ok(reciprocal_rank_fusion(
                &rank_by_vector(&vector, &compatible, VectorRole::Subject)?,
                &rank_by_vector(&vector, &compatible, VectorRole::Capability)?,
                60.0,
            )),
            _ => Err(SlideRetrievalError::new("invalid_query").into()),
        }
    }

    async fn embed(&self, document: &str) -> anyhow::Result<Vec<f64>> {
        let (Some(embedder), Some(_)) = (self.embedder.as_ref(), self.embedding_model.as_ref())
        else {
            return Err(SlideRetrievalError::new("semantic_unavailable").into());
        };
        let vector = embedder
            .embed(document)
            .await
            .map_err(|_| SlideRetrievalError::new("semantic_unavailable"))?;
        validate_embedding(&vector, self.embedding_dimensions)
            .map_err(|_| SlideRetrievalError::new("semantic_unavailable"))?;
        Ok(vector)
    }
}

pub fn build_safe_fts_query(query: &str, operator: &str) -> Result<String, SlideRetrievalError> {
    if !matches!(operator, "AND" | "OR")
        || query
            .chars()
            .any(|character| "\"*():^{}[]".contains(character))
    {
        return Err(SlideRetrievalError::new("invalid_query"));
    }
    static WORDS: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"[\p{L}\p{N}]+").expect("constant Unicode token pattern")
    });
    let lowercase = query.to_lowercase();
    let tokens = WORDS
        .find_iter(&lowercase)
        .map(|word| word.as_str())
        .collect::<Vec<_>>();
    if tokens.is_empty() || tokens.len() > 200 {
        return Err(SlideRetrievalError::new("invalid_query"));
    }
    let mut seen = HashSet::new();
    Ok(tokens
        .into_iter()
        .filter(|token| seen.insert(*token))
        .map(|token| format!("\"{token}\"*"))
        .collect::<Vec<_>>()
        .join(&format!(" {operator} ")))
}

fn has_ready_vectors(record: &RetrievalClassificationRecord) -> bool {
    let classification = &record.classification;
    classification.embedding_status == "ready"
        && classification.subject_vector.is_some()
        && classification.capability_vector.is_some()
}
fn compatible_vectors(
    record: &RetrievalClassificationRecord,
    model: Option<&str>,
    dimensions: Option<usize>,
) -> bool {
    has_ready_vectors(record)
        && record.classification.embedding_model.as_deref() == model
        && record.classification.embedding_dimensions == dimensions
}
fn rank_by_vector(
    vector: &[f64],
    records: &[RetrievalClassificationRecord],
    role: VectorRole,
) -> anyhow::Result<Vec<RetrievalClassificationRecord>> {
    let mut scored = records
        .iter()
        .map(|record| {
            let candidate = match role {
                VectorRole::Subject => record.classification.subject_vector.as_deref(),
                VectorRole::Capability => record.classification.capability_vector.as_deref(),
            }
            .unwrap_or_default();
            Ok((record, cosine_similarity(vector, candidate)?))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    scored.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| locale_compare(&right.created_at, &left.created_at))
            .then_with(|| locale_compare(&left.template_id, &right.template_id))
    });
    Ok(scored
        .into_iter()
        .map(|(record, _)| record.clone())
        .collect())
}
/// Fuse ordinal ranks without combining lexical and vector scores directly.
pub fn reciprocal_rank_fusion(
    first: &[RetrievalClassificationRecord],
    second: &[RetrievalClassificationRecord],
    rank_constant: f64,
) -> Vec<RetrievalClassificationRecord> {
    reciprocal_rank_scores(&[first.to_vec(), second.to_vec()], rank_constant)
        .into_iter()
        .map(|(record, _)| record)
        .collect()
}
fn reciprocal_rank_scores(
    rankings: &[Vec<RetrievalClassificationRecord>],
    rank_constant: f64,
) -> Vec<(RetrievalClassificationRecord, f64)> {
    let mut positions: HashMap<&str, usize> = HashMap::new();
    let mut records: Vec<(RetrievalClassificationRecord, f64)> = Vec::new();
    for ranking in rankings {
        for (index, record) in ranking.iter().enumerate() {
            let position = *positions.entry(&record.template_id).or_insert_with(|| {
                records.push((record.clone(), 0.0));
                records.len() - 1
            });
            records[position].1 += 1.0 / (rank_constant + index as f64 + 1.0);
        }
    }
    records.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .total_cmp(left_score)
            .then_with(|| locale_compare(&right.created_at, &left.created_at))
            .then_with(|| locale_compare(&left.template_id, &right.template_id))
    });
    records
}

fn includes(values: &[String], requested: &[String]) -> bool {
    requested.is_empty()
        || requested
            .iter()
            .any(|requested| values.iter().any(|candidate| equal(candidate, requested)))
}
fn equal(left: &str, right: &str) -> bool {
    normalize_taxonomy_value(left) == normalize_taxonomy_value(right)
}
fn all_topics(metadata: &SlideRetrievalMetadata) -> Vec<String> {
    metadata
        .subject
        .domains
        .iter()
        .flat_map(|domain| domain.topics.iter())
        .chain(metadata.subject.other_topics.iter())
        .cloned()
        .collect()
}
fn apply_filters(
    records: &[RetrievalClassificationRecord],
    filters: &SlideQueryFilters,
) -> Vec<RetrievalClassificationRecord> {
    records
        .iter()
        .filter(|record| {
            let Some(metadata) = record.classification.metadata.as_ref() else {
                return false;
            };
            let domains = metadata
                .subject
                .domains
                .iter()
                .map(|domain| domain.id.clone())
                .collect::<Vec<_>>();
            let slot_roles = metadata
                .template_fit
                .content_slots
                .iter()
                .map(|slot| slot.role.clone())
                .collect::<Vec<_>>();
            (filters.kinds.is_empty() || filters.kinds.contains(&record.kind))
                && includes(&domains, &filters.domains)
                && includes(&all_topics(metadata), &filters.topics)
                && includes(&metadata.communication.intents, &filters.intents)
                && includes(
                    std::slice::from_ref(&metadata.template_fit.archetype),
                    &filters.archetypes,
                )
                && includes(&slot_roles, &filters.slot_roles)
                && (filters.content_density.is_empty()
                    || filters
                        .content_density
                        .contains(&metadata.visual.content_density))
        })
        .cloned()
        .collect()
}

fn project_record(
    record: &RetrievalClassificationRecord,
    select: &[String],
    app_id: &str,
) -> anyhow::Result<Value> {
    let metadata = record
        .classification
        .metadata
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Retrieval metadata is missing."))?;
    let all = select.iter().any(|section| section == "all");
    let mut sections = serde_json::Map::new();
    if all || select.iter().any(|section| section == "subject") {
        sections.insert("subject".into(), serde_json::to_value(&metadata.subject)?);
    }
    if all || select.iter().any(|section| section == "communication") {
        sections.insert(
            "communication".into(),
            serde_json::to_value(&metadata.communication)?,
        );
    }
    if all || select.iter().any(|section| section == "template_fit") {
        sections.insert(
            "template_fit".into(),
            serde_json::to_value(&metadata.template_fit)?,
        );
    }
    if all || select.iter().any(|section| section == "visual") {
        sections.insert("visual".into(), serde_json::to_value(&metadata.visual)?);
    }
    if all || select.iter().any(|section| section == "keywords") {
        sections.insert(
            "keywords".into(),
            json!({"retrieval_keywords":metadata.retrieval_keywords}),
        );
    }
    Ok(
        json!({"kind":record.kind,"previewUrl":record.preview_available.then(|| format!("/api/v1/templates/{}/preview?appId={}", record.template_id, percent_encode_component(app_id))),"sections":sections,"templateId":record.template_id,"title":record.title}),
    )
}
fn build_facet_response(
    records: &[RetrievalClassificationRecord],
    facet: &str,
) -> anyhow::Result<Value> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for record in records {
        if let Some(metadata) = record.classification.metadata.as_ref() {
            for value in facet_values(metadata, facet)?
                .into_iter()
                .collect::<HashSet<_>>()
            {
                *counts.entry(value).or_default() += 1;
            }
        }
    }
    let mut values = counts.into_iter().collect::<Vec<_>>();
    values.sort_by(|(left_value, left_count), (right_value, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| locale_compare(left_value, right_value))
    });
    Ok(
        json!({"facet":facet,"mode_used":"facets","values":values.into_iter().map(|(value,count)| json!({"count":count,"value":value})).collect::<Vec<_>>() }),
    )
}
fn facet_values(metadata: &SlideRetrievalMetadata, facet: &str) -> anyhow::Result<Vec<String>> {
    match facet {
        "domains" => Ok(metadata
            .subject
            .domains
            .iter()
            .map(|domain| domain.id.clone())
            .collect()),
        "topics" => Ok(all_topics(metadata)),
        "intents" => Ok(metadata.communication.intents.clone()),
        "archetype" => Ok(vec![metadata.template_fit.archetype.clone()]),
        "layout_type" => Ok(vec![metadata.visual.layout_type.clone()]),
        "content_density" => Ok(vec![metadata.visual.content_density.clone()]),
        "slot_roles" => Ok(metadata
            .template_fit
            .content_slots
            .iter()
            .map(|slot| slot.role.clone())
            .collect()),
        _ => Err(SlideRetrievalError::new("invalid_query").into()),
    }
}

fn percent_encode_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}
fn locale_compare(left: &str, right: &str) -> std::cmp::Ordering {
    static COLLATOR: std::sync::LazyLock<icu_collator::CollatorBorrowed<'static>> =
        std::sync::LazyLock::new(|| {
            icu_collator::Collator::try_new(icu_locale::locale!("en-US").into(), Default::default())
                .expect("compiled English collation data")
        });
    COLLATOR.compare(left, right)
}

/// App identity is captured by trusted context and never accepted in tool input.
pub struct SlideAgentTools {
    pub app_id: String,
    pub retrieval: Arc<SlideRetrievalService>,
    pub templates: super::super::ImportService,
}
impl SlideAgentTools {
    pub fn get_slide(&self, input: &Value) -> anyhow::Result<Value> {
        let id = Self::parse_get_slide_input(input)?;
        match self.templates.for_app(&self.app_id)?.find(id)? {
            Some(slide) => Ok(serde_json::to_value(slide)?),
            None => Ok(json!({"error":{"code":"slide_not_available"}})),
        }
    }
    pub async fn find_slides_for_finding(&self, input: &Value) -> anyhow::Result<Value> {
        self.retrieval.find_for_finding(&self.app_id, input).await
    }
    pub async fn query_slides(&self, input: &Value) -> anyhow::Result<Value> {
        self.retrieval.query(&self.app_id, input).await
    }
    pub fn parse_get_slide_input(input: &Value) -> Result<&str, SlideRetrievalError> {
        let object = input
            .as_object()
            .filter(|object| object.len() == 1)
            .ok_or_else(|| SlideRetrievalError::new("invalid_query"))?;
        object
            .get("template_id")
            .and_then(Value::as_str)
            .map(js_trim)
            .filter(|id| !id.is_empty() && utf16_len(id) <= 200)
            .ok_or_else(|| SlideRetrievalError::new("invalid_query"))
    }
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/retrieval/query_tests.rs"]
mod tests;

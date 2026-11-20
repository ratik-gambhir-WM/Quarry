//! Request construction and provisional normalization policy (`POLICY_VERSION` 1).
//!
//! Thresholds are starting values to calibrate with labeled data, not quality claims.
use std::collections::{HashMap, HashSet};

use super::{
    model::{
        ClassificationResult, ResultStatus, SecondaryCategory, MODEL, POLICY_VERSION,
        PROMPT_VERSION, SAMPLE_VERSION,
    },
    taxonomy::{Category, TAXONOMY_VERSION, UNCLASSIFIED},
};
use crate::adapters::openai::decisions::{
    Answer, Choice, ChoiceValue, DecisionRequest, InputMessage, InputRole, Question,
};

pub const PRIMARY_QUESTION: &str = "primary_category";
const PRIMARY_INSTRUCTIONS: &str = "Choose the category that best describes the document's \
dominant purpose. Use unclassified when the evidence cannot support a primary category.";
const UNCLASSIFIED_DESCRIPTION: &str = "Insufficient, ambiguous, irrelevant, or unreadable \
evidence; no supported primary category.";

pub const MIN_PRIMARY_PROBABILITY: f64 = 0.65;
pub const MIN_PRIMARY_CONFIDENCE: f64 = 0.60;
pub const MIN_SECONDARY_PROBABILITY: f64 = 0.80;
pub const MAX_SECONDARIES: usize = 3;
const DISTRIBUTION_TOLERANCE: f64 = 0.01;

/// The response could not be validated; never presented as a refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidResponse;

/// One primary choice over 28 categories plus the sentinel, and 28 coverage predicates.
pub fn request<'a>(instructions: &'a str, evidence: &'a str) -> DecisionRequest<'a> {
    let mut choices: Vec<Choice> = Category::ALL
        .into_iter()
        .map(|category| Choice {
            value: category.id().to_string(),
            description: category.choice_description(),
        })
        .collect();
    choices.push(Choice {
        value: UNCLASSIFIED.to_string(),
        description: UNCLASSIFIED_DESCRIPTION.to_string(),
    });
    let mut questions = Vec::with_capacity(Category::ALL.len() + 1);
    questions.push(Question::Choice {
        name: PRIMARY_QUESTION.to_string(),
        instructions: PRIMARY_INSTRUCTIONS.to_string(),
        choices,
    });
    questions.extend(
        Category::ALL
            .into_iter()
            .map(|category| Question::Predicate {
                name: category.coverage_question(),
                instructions: category.coverage_instructions(),
            }),
    );
    DecisionRequest {
        model: MODEL,
        input: vec![
            InputMessage {
                role: InputRole::Developer,
                content: instructions,
            },
            InputMessage {
                role: InputRole::User,
                content: evidence,
            },
        ],
        questions,
    }
}

/// Provenance shared by every result for one claimed request.
#[derive(Clone, Debug)]
pub struct Provenance {
    pub fingerprint: String,
    pub sampled: bool,
}

/// Local abstention for empty evidence; no provider request took place.
pub fn insufficient(provenance: &Provenance) -> ClassificationResult {
    base(ResultStatus::InsufficientInformation, None, provenance)
}

/// Validate every named answer and apply the provisional thresholds.
pub fn normalize(
    answers: &[Answer],
    reported_model: Option<&str>,
    provenance: &Provenance,
) -> Result<ClassificationResult, InvalidResponse> {
    let model = match reported_model {
        Some(model) if !model.trim().is_empty() && model.len() <= 200 => model.to_string(),
        Some(_) => return Err(InvalidResponse),
        None => MODEL.to_string(),
    };
    let by_name = index_by_name(answers)?;
    let primary = by_name.get(PRIMARY_QUESTION).ok_or(InvalidResponse)?;
    let (selected, probability, confidence) = match primary {
        Answer::Refusal { .. } => {
            return Ok(base(ResultStatus::Refused, Some(model), provenance));
        }
        Answer::Choice {
            choice,
            confidence,
            probabilities,
            ..
        } => {
            let ChoiceValue::Text(selected) = choice else {
                return Err(InvalidResponse);
            };
            let probability = validate_distribution(probabilities, selected)?;
            if !unit(*confidence) {
                return Err(InvalidResponse);
            }
            (selected.as_str(), probability, *confidence)
        }
        _ => return Err(InvalidResponse),
    };
    let mut coverage = Vec::with_capacity(Category::ALL.len());
    let mut incomplete = false;
    for category in Category::ALL {
        match by_name
            .get(category.coverage_question().as_str())
            .ok_or(InvalidResponse)?
        {
            Answer::Predicate { probability, .. } if unit(*probability) => {
                coverage.push((category, *probability));
            }
            Answer::Refusal { .. } => incomplete = true,
            _ => return Err(InvalidResponse),
        }
    }
    if selected == UNCLASSIFIED {
        let mut result = base(
            ResultStatus::InsufficientInformation,
            Some(model),
            provenance,
        );
        result.confidence = Some(confidence);
        result.primary_probability = Some(probability);
        return Ok(result);
    }
    let category = Category::from_id(selected).ok_or(InvalidResponse)?;
    if probability < MIN_PRIMARY_PROBABILITY || confidence < MIN_PRIMARY_CONFIDENCE {
        let mut result = base(ResultStatus::NeedsReview, Some(model), provenance);
        result.confidence = Some(confidence);
        result.primary_probability = Some(probability);
        result.suggested_primary_category = Some(category);
        result.suggested_primary_workstream = Some(category.workstream());
        result.secondary_assessment_incomplete = incomplete;
        return Ok(result);
    }
    let mut secondaries: Vec<_> = coverage
        .into_iter()
        .filter(|(candidate, probability)| {
            *candidate != category && *probability >= MIN_SECONDARY_PROBABILITY
        })
        .collect();
    secondaries.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.id().cmp(b.0.id())));
    secondaries.truncate(MAX_SECONDARIES);
    let mut result = base(ResultStatus::Classified, Some(model), provenance);
    result.primary_category = Some(category);
    result.primary_workstream = Some(category.workstream());
    result.confidence = Some(confidence);
    result.primary_probability = Some(probability);
    result.secondary_assessment_incomplete = incomplete;
    result.secondary_categories = secondaries
        .into_iter()
        .map(|(category, probability)| SecondaryCategory {
            workstream: category.workstream(),
            category,
            probability,
        })
        .collect();
    Ok(result)
}

/// Exactly the requested names, each once; unnamed or unexpected answers are invalid.
fn index_by_name(answers: &[Answer]) -> Result<HashMap<&str, &Answer>, InvalidResponse> {
    let expected: HashSet<String> = std::iter::once(PRIMARY_QUESTION.to_string())
        .chain(Category::ALL.into_iter().map(Category::coverage_question))
        .collect();
    let mut by_name = HashMap::with_capacity(answers.len());
    for answer in answers {
        let name = answer.name().ok_or(InvalidResponse)?;
        if !expected.contains(name) || by_name.insert(name, answer).is_some() {
            return Err(InvalidResponse);
        }
    }
    if by_name.len() != expected.len() {
        return Err(InvalidResponse);
    }
    Ok(by_name)
}

/// The complete, unique supplied option set with finite unit probabilities summing to one.
fn validate_distribution(
    probabilities: &[crate::adapters::openai::decisions::ChoiceProbability],
    selected: &str,
) -> Result<f64, InvalidResponse> {
    let options: HashSet<&str> = Category::ALL
        .iter()
        .map(|category| category.id())
        .chain(std::iter::once(UNCLASSIFIED))
        .collect();
    if !options.contains(selected) || probabilities.len() != options.len() {
        return Err(InvalidResponse);
    }
    let mut seen = HashSet::with_capacity(options.len());
    let mut sum = 0.0;
    let mut selected_probability = None;
    for entry in probabilities {
        let ChoiceValue::Text(value) = &entry.value else {
            return Err(InvalidResponse);
        };
        if !options.contains(value.as_str())
            || !seen.insert(value.as_str())
            || !unit(entry.probability)
        {
            return Err(InvalidResponse);
        }
        sum += entry.probability;
        if value == selected {
            selected_probability = Some(entry.probability);
        }
    }
    if (sum - 1.0).abs() > DISTRIBUTION_TOLERANCE {
        return Err(InvalidResponse);
    }
    selected_probability.ok_or(InvalidResponse)
}

fn base(
    status: ResultStatus,
    model: Option<String>,
    provenance: &Provenance,
) -> ClassificationResult {
    ClassificationResult {
        status,
        primary_workstream: None,
        primary_category: None,
        secondary_categories: Vec::new(),
        confidence: None,
        primary_probability: None,
        suggested_primary_category: None,
        suggested_primary_workstream: None,
        secondary_assessment_incomplete: false,
        model,
        taxonomy_version: TAXONOMY_VERSION,
        prompt_version: PROMPT_VERSION.to_string(),
        sample_version: SAMPLE_VERSION.to_string(),
        policy_version: POLICY_VERSION,
        input_fingerprint: provenance.fingerprint.clone(),
        classified_at: None,
        sampled: provenance.sampled,
    }
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// Fingerprint over content, evidence and the pinned profile tuple.
pub fn fingerprint(content_sha256: &str, sample_hash: &str) -> String {
    crate::shared::ids::sha256_hex(
        format!(
            "{content_sha256}\0{sample_hash}\0{TAXONOMY_VERSION}\0{PROMPT_VERSION}\0{POLICY_VERSION}\0{SAMPLE_VERSION}\0{MODEL}"
        )
        .as_bytes(),
    )
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/classification/policy_tests.rs"]
mod tests;

use super::*;
use crate::adapters::openai::decisions::ChoiceProbability;

fn provenance() -> Provenance {
    Provenance {
        fingerprint: "f".repeat(64),
        sampled: true,
    }
}

fn distribution(selected: &str, probability: f64) -> Vec<ChoiceProbability> {
    let others = Category::ALL.len(); // 28 others besides `selected` among 29 options
    let rest = (1.0 - probability) / others as f64;
    Category::ALL
        .iter()
        .map(|c| c.id())
        .chain(std::iter::once(UNCLASSIFIED))
        .map(|value| ChoiceProbability {
            value: ChoiceValue::Text(value.into()),
            probability: if value == selected { probability } else { rest },
        })
        .collect()
}

fn answers(
    selected: &str,
    probability: f64,
    confidence: f64,
    coverage: &[(Category, f64)],
) -> Vec<Answer> {
    let mut answers = vec![Answer::Choice {
        name: Some(PRIMARY_QUESTION.into()),
        choice: ChoiceValue::Text(selected.into()),
        confidence,
        probabilities: distribution(selected, probability),
    }];
    for category in Category::ALL {
        let probability = coverage
            .iter()
            .find(|(c, _)| *c == category)
            .map_or(0.05, |(_, p)| *p);
        answers.push(Answer::Predicate {
            name: Some(category.coverage_question()),
            probability,
        });
    }
    answers
}

#[test]
fn request_has_one_primary_choice_and_twenty_eight_predicates_with_no_responses_fields() {
    let request = request("instructions", "evidence");
    let value = serde_json::to_value(&request).unwrap();
    assert_eq!(value["model"], "gpt-6-luna");
    let questions = value["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 29);
    assert_eq!(questions[0]["type"], "choice");
    assert_eq!(questions[0]["name"], PRIMARY_QUESTION);
    let choices = questions[0]["choices"].as_array().unwrap();
    assert_eq!(choices.len(), 29);
    assert_eq!(choices[28]["value"], UNCLASSIFIED);
    assert!(questions[1..].iter().all(|q| q["type"] == "predicate"));
    assert_eq!(value["input"][0]["role"], "developer");
    assert_eq!(value["input"][1]["role"], "user");
    for forbidden in ["text", "response_format", "tools", "instructions"] {
        assert!(value.get(forbidden).is_none(), "{forbidden}");
    }
}

#[test]
fn confident_primary_is_classified_with_ordered_capped_secondaries() {
    let coverage = [
        (Category::ProductArchitecture, 0.99),
        (Category::DataArchitectureInfrastructure, 0.87),
        (Category::Sdlc, 0.87),
        (Category::ProductSecurity, 0.95),
        (Category::Cybersecurity, 0.81),
        (Category::CompliancePrivacy, 0.79),
    ];
    let result = normalize(
        &answers("product_architecture", 0.92, 0.89, &coverage),
        Some("gpt-6-luna"),
        &provenance(),
    )
    .unwrap();
    assert_eq!(result.status, ResultStatus::Classified);
    assert_eq!(result.primary_category, Some(Category::ProductArchitecture));
    assert!(result.is_consistent());
    let secondaries: Vec<_> = result
        .secondary_categories
        .iter()
        .map(|s| s.category)
        .collect();
    // Primary excluded, probability descending then ID, capped at three.
    assert_eq!(
        secondaries,
        vec![
            Category::ProductSecurity,
            Category::DataArchitectureInfrastructure,
            Category::Sdlc
        ]
    );
    assert_eq!(result.confidence, Some(0.89));
    assert_eq!(result.primary_probability, Some(0.92));
}

#[test]
fn threshold_boundaries_route_low_signals_to_review_without_labels() {
    let accepted = normalize(
        &answers(
            "cybersecurity",
            MIN_PRIMARY_PROBABILITY,
            MIN_PRIMARY_CONFIDENCE,
            &[],
        ),
        None,
        &provenance(),
    )
    .unwrap();
    assert_eq!(accepted.status, ResultStatus::Classified);
    let review = normalize(
        &answers(
            "cybersecurity",
            0.64,
            0.95,
            &[(Category::ItOperations, 0.99)],
        ),
        None,
        &provenance(),
    )
    .unwrap();
    assert_eq!(review.status, ResultStatus::NeedsReview);
    assert_eq!(review.primary_category, None);
    assert!(review.secondary_categories.is_empty());
    assert_eq!(
        review.suggested_primary_category,
        Some(Category::Cybersecurity)
    );
    assert!(review.is_consistent());
}

#[test]
fn unclassified_and_refusals_are_explicit_completed_outcomes() {
    let insufficient =
        normalize(&answers(UNCLASSIFIED, 0.9, 0.9, &[]), None, &provenance()).unwrap();
    assert_eq!(insufficient.status, ResultStatus::InsufficientInformation);
    assert_eq!(insufficient.primary_category, None);

    let mut refused = answers("sdlc", 0.9, 0.9, &[]);
    refused[0] = Answer::Refusal {
        name: Some(PRIMARY_QUESTION.into()),
    };
    assert_eq!(
        normalize(&refused, None, &provenance()).unwrap().status,
        ResultStatus::Refused
    );

    let mut partial = answers("sdlc", 0.9, 0.9, &[(Category::ProductSecurity, 0.9)]);
    partial[5] = Answer::Refusal {
        name: Some(Category::ALL[4].coverage_question()),
    };
    let result = normalize(&partial, None, &provenance()).unwrap();
    assert!(result.secondary_assessment_incomplete);
    assert_eq!(result.secondary_categories.len(), 1);
}

#[test]
fn malformed_answers_are_invalid_rather_than_refusals() {
    let valid = answers("sdlc", 0.9, 0.9, &[]);
    let mut cases: Vec<Vec<Answer>> = Vec::new();
    cases.push(valid[1..].to_vec());
    let mut duplicate = answers("sdlc", 0.9, 0.9, &[]);
    duplicate.push(Answer::Predicate {
        name: Some(Category::Sdlc.coverage_question()),
        probability: 0.1,
    });
    cases.push(duplicate);
    let mut unknown = answers("sdlc", 0.9, 0.9, &[]);
    unknown.push(Answer::Predicate {
        name: Some("covers_unknown".into()),
        probability: 0.1,
    });
    cases.push(unknown);
    let mut unnamed = answers("sdlc", 0.9, 0.9, &[]);
    unnamed[3] = Answer::Predicate {
        name: None,
        probability: 0.1,
    };
    cases.push(unnamed);
    let mut boolean = answers("sdlc", 0.9, 0.9, &[]);
    boolean[0] = Answer::Choice {
        name: Some(PRIMARY_QUESTION.into()),
        choice: ChoiceValue::Bool(true),
        confidence: 0.9,
        probabilities: distribution("sdlc", 0.9),
    };
    cases.push(boolean);
    cases.push(answers("not_a_category", 0.9, 0.9, &[]));
    let mut bad_sum = answers("sdlc", 0.9, 0.9, &[]);
    if let Answer::Choice { probabilities, .. } = &mut bad_sum[0] {
        probabilities[0].probability = 0.5;
    }
    cases.push(bad_sum);
    cases.push(answers("sdlc", 0.9, f64::NAN, &[]));
    let mut wrong_kind = answers("sdlc", 0.9, 0.9, &[]);
    wrong_kind[2] = Answer::Score {
        name: Some(Category::ALL[1].coverage_question()),
    };
    cases.push(wrong_kind);
    let mut out_of_range = answers("sdlc", 0.9, 0.9, &[]);
    out_of_range[1] = Answer::Predicate {
        name: Some(Category::ALL[0].coverage_question()),
        probability: 1.5,
    };
    cases.push(out_of_range);
    for (index, case) in cases.iter().enumerate() {
        assert_eq!(
            normalize(case, None, &provenance()),
            Err(InvalidResponse),
            "case {index}"
        );
    }
    // Reordered named answers are still valid.
    let mut reordered = answers("sdlc", 0.9, 0.9, &[]);
    reordered.reverse();
    assert!(normalize(&reordered, None, &provenance()).is_ok());
}

#[test]
fn fingerprint_pins_content_evidence_and_profile() {
    let a = fingerprint(&"a".repeat(64), &"b".repeat(64));
    assert_eq!(a, fingerprint(&"a".repeat(64), &"b".repeat(64)));
    assert_ne!(a, fingerprint(&"a".repeat(64), &"c".repeat(64)));
    assert_eq!(a.len(), 64);
    let local = insufficient(&provenance());
    assert_eq!(local.model, None);
    assert!(local.is_consistent());
}

#[test]
fn every_policy_outcome_preserves_the_existing_json_contract() {
    let cases = [
        ("sdlc", 0.9, "classified", Some("sdlc"), None),
        ("sdlc", 0.5, "needs_review", None, Some("sdlc")),
        (UNCLASSIFIED, 0.9, "insufficient_information", None, None),
    ];
    for (selected, probability, status, primary, suggested) in cases {
        let result = normalize(
            &answers(selected, probability, 0.85, &[]),
            None,
            &provenance(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "status": status,
                "primaryWorkstream": primary.map(|_| "product_engineering"),
                "primaryCategory": primary,
                "secondaryCategories": [],
                "confidence": 0.85,
                "primaryProbability": probability,
                "suggestedPrimaryCategory": suggested,
                "suggestedPrimaryWorkstream": suggested.map(|_| "product_engineering"),
                "secondaryAssessmentIncomplete": false,
                "model": "gpt-6-luna",
                "taxonomyVersion": 1,
                "promptVersion": "document-classification-v1",
                "sampleVersion": "sample-v1",
                "policyVersion": 1,
                "inputFingerprint": "f".repeat(64),
                "sampled": true,
            })
        );
    }
    // Both local abstention and provider refusal remain label-free and unscored.
    let mut refused = answers("sdlc", 0.9, 0.85, &[]);
    refused[0] = Answer::Refusal {
        name: Some(PRIMARY_QUESTION.into()),
    };
    for (result, status, model) in [
        (
            insufficient(&provenance()),
            "insufficient_information",
            None,
        ),
        (
            normalize(&refused, None, &provenance()).unwrap(),
            "refused",
            Some("gpt-6-luna"),
        ),
    ] {
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "status": status, "primaryWorkstream": null, "primaryCategory": null,
                "secondaryCategories": [], "confidence": null, "primaryProbability": null,
                "suggestedPrimaryCategory": null, "suggestedPrimaryWorkstream": null,
                "secondaryAssessmentIncomplete": false, "model": model,
                "taxonomyVersion": 1, "promptVersion": "document-classification-v1",
                "sampleVersion": "sample-v1", "policyVersion": 1,
                "inputFingerprint": "f".repeat(64), "sampled": true,
            })
        );
    }
}

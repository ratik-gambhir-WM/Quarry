use crate::domains::templates::slide_templates::*;
use serde_json::json;

fn insert(id: &str) -> TemplateInsert {
    let template_json: CanvasDocument = serde_json::from_value(json!({
        "presentation":{"title":"Cloud Architecture","showBranding":false,"preserveElementOrder":true,"slides":[{
            "id":"slide","name":"Cloud","width":320,"height":180,"backgroundColor":"FFFFFF",
            "elements":[{"id":"headline","type":"text","x":0,"y":0,"w":100,"h":30,"text":"Cloud","fill":"transparent","stroke":"transparent","strokeWidth":0,"textColor":"000000","fontSize":18,"fontFace":"Arial","padding":0,"align":"left","valign":"top","runs":[]}]
        }]}
    }))
    .unwrap();
    TemplateInsert {
        template: StoredTemplate {
            template_id: id.into(),
            template_json,
        },
        assets: vec![],
        metadata: StoredTemplateMetadata {
            template_id: id.into(),
            kind: TemplateKind::Diagram,
            title: "Cloud Architecture".into(),
            description: "Imported PowerPoint template".into(),
            source: TemplateSource::Import,
            created_at: 0,
            catalog_version: None,
            catalog_checksum: None,
        },
        preview: None,
    }
}

fn pending(fingerprint: &str) -> PendingSlideClassification {
    PendingSlideClassification {
        input_fingerprint: fingerprint.into(),
        prompt_version: SLIDE_CLASSIFICATION_PROMPT_VERSION.into(),
        schema_version: SLIDE_CLASSIFICATION_SCHEMA_VERSION,
    }
}

fn classified(id: &str, fingerprint: &str) -> CompleteSlideClassification {
    let metadata = serde_json::from_value(json!({
        "subject":{"summary":"A cloud architecture.","domains":[{"id":"cloud","relevance":"primary","topics":["cloud"]}],"otherTopics":[],"technologies":[],"entities":[],"claims":[],"synonyms":[]},
        "communication":{"intents":["explanation"],"informationTypes":[],"audience":[]},
        "templateFit":{"archetype":"architecture","contentSlots":[{"elementId":"headline","role":"headline","capacity":"short-text"}]},
        "visual":{"layoutType":"diagram-led","visualElements":[],"contentDensity":"low","structuralFeatures":[]},"retrievalKeywords":["cloud"]
    }))
    .unwrap();
    CompleteSlideClassification {
        template_id: id.into(),
        classified_at: "2026-09-14T01:00:00.000Z".into(),
        classified_fingerprint: fingerprint.into(),
        metadata,
        model: "fixture-model".into(),
        subject_embedding_document: "Cloud architecture subject".into(),
        subject_embedding_fingerprint: "subject-v1".into(),
        capability_embedding_document: "Cloud architecture capability".into(),
        capability_embedding_fingerprint: "capability-v1".into(),
    }
}

#[test]
fn manual_retry_is_atomic_app_scoped_and_survives_reopen() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("retry.sqlite");
    let repository = crate::slide_template_test_support::repository(&path).unwrap();
    repository
        .insert_with_pending_classification(&insert("retry"), &pending("v1"))
        .unwrap();
    assert_eq!(
        repository.retry_failed_slide_processing("retry").unwrap(),
        RetrySlideProcessingOutcome::NotFailed
    );
    repository
        .claim_next_slide_classification("2026-11-04T00:00:00Z", "2026-11-04T00:01:00Z")
        .unwrap()
        .unwrap();
    assert_eq!(
        repository.retry_failed_slide_processing("retry").unwrap(),
        RetrySlideProcessingOutcome::NotFailed
    );
    repository
        .record_slide_classification_failure("retry", "classification_timeout", None)
        .unwrap();
    let other = repository.for_app("Another_App").unwrap().unwrap();
    assert_eq!(
        other.retry_failed_slide_processing("retry").unwrap(),
        RetrySlideProcessingOutcome::NotFound
    );
    assert_eq!(
        repository
            .find_slide_classification("retry")
            .unwrap()
            .unwrap()
            .status,
        "failed"
    );
    let repository = std::sync::Arc::new(repository);
    let results = std::thread::scope(|scope| {
        let first = scope.spawn(|| repository.retry_failed_slide_processing("retry").unwrap());
        let second = scope.spawn(|| repository.retry_failed_slide_processing("retry").unwrap());
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(
        results
            .iter()
            .filter(|result| **result
                == RetrySlideProcessingOutcome::Queued(SlideProcessingStage::Classification))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == RetrySlideProcessingOutcome::NotFailed)
            .count(),
        1
    );
    drop(repository);
    let reopened = crate::slide_template_test_support::repository(&path).unwrap();
    let state = reopened
        .find_slide_classification("retry")
        .unwrap()
        .unwrap();
    assert_eq!(state.status, "pending");
    assert_eq!(state.attempt_count, 0);
    assert!(state.last_error_code.is_none());
    let job = reopened
        .claim_next_slide_classification("2026-11-04T00:02:00Z", "2026-11-04T00:03:00Z")
        .unwrap()
        .unwrap();
    assert_eq!(job.attempt_count, 1);
    assert_eq!(job.input_fingerprint, "v1");
}

#[test]
fn manual_embedding_retry_preserves_completed_artifacts_and_resets_only_embedding_budget() {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap();
    repository
        .insert_with_processing_classification(&insert("retry"), &pending("v1"))
        .unwrap();
    repository
        .complete_slide_classification(&classified("retry", "v1"))
        .unwrap();
    repository
        .claim_next_slide_embedding("2026-11-04T00:00:00Z", "2026-11-04T00:01:00Z")
        .unwrap()
        .unwrap();
    repository
        .record_slide_embedding_failure("retry", "embedding_timeout", None)
        .unwrap();
    let before = repository
        .find_slide_classification("retry")
        .unwrap()
        .unwrap();
    assert_eq!(
        repository.retry_failed_slide_processing("retry").unwrap(),
        RetrySlideProcessingOutcome::Queued(SlideProcessingStage::Embedding)
    );
    assert_eq!(
        repository.retry_failed_slide_processing("retry").unwrap(),
        RetrySlideProcessingOutcome::NotFailed
    );
    let after = repository
        .find_slide_classification("retry")
        .unwrap()
        .unwrap();
    assert_eq!(after.status, "ready");
    assert_eq!(after.metadata, before.metadata);
    assert_eq!(after.classified_at, before.classified_at);
    assert_eq!(after.attempt_count, before.attempt_count);
    assert_eq!(
        after.subject_embedding_document,
        before.subject_embedding_document
    );
    assert_eq!(after.embedding_status, "pending");
    assert_eq!(after.embedding_attempt_count, 0);
    assert!(after.embedding_last_error_code.is_none());
    assert_eq!(
        repository
            .claim_next_slide_embedding("2026-11-04T00:02:00Z", "2026-11-04T00:03:00Z")
            .unwrap()
            .unwrap()
            .attempt_count,
        1
    );
}

#[test]
fn dual_vector_leases_retries_and_refreshes_are_durable() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("lifecycle.sqlite");
    let repository = crate::slide_template_test_support::repository(&path).unwrap();
    repository
        .insert_with_pending_classification(&insert("cloud"), &pending("class-v1"))
        .unwrap();
    assert_eq!(
        repository
            .claim_next_slide_classification("2026-09-14T00:00:00Z", "2026-09-14T00:00:10Z")
            .unwrap()
            .unwrap()
            .attempt_count,
        1
    );
    repository
        .record_slide_classification_failure(
            "cloud",
            "classification_timeout",
            Some("2026-09-14T00:00:30Z"),
        )
        .unwrap();
    assert!(repository
        .claim_next_slide_classification("2026-09-14T00:00:29Z", "2026-09-14T00:00:39Z")
        .unwrap()
        .is_none());
    let job = repository
        .claim_next_slide_classification("2026-09-14T00:00:30Z", "2026-09-14T00:00:40Z")
        .unwrap()
        .unwrap();
    assert_eq!(job.attempt_count, 2);
    repository
        .complete_slide_classification(&classified("cloud", "class-v1"))
        .unwrap();
    let embedding = repository
        .claim_next_slide_embedding("2026-09-14T00:00:31Z", "2026-09-14T00:00:41Z")
        .unwrap()
        .unwrap();
    assert_eq!(embedding.subject_fingerprint, "subject-v1");
    assert_eq!(embedding.capability_fingerprint, "capability-v1");
    assert!(repository
        .complete_slide_embedding(&CompleteSlideEmbedding {
            template_id: "cloud".into(),
            subject_fingerprint: "stale".into(),
            capability_fingerprint: "stale".into(),
            subject_vector: vec![1.0, 0.0],
            capability_vector: vec![0.0, 1.0],
            model: "embedder".into(),
            dimensions: 2,
        })
        .is_err());
    repository
        .complete_slide_embedding(&CompleteSlideEmbedding {
            template_id: "cloud".into(),
            subject_fingerprint: "subject-v1".into(),
            capability_fingerprint: "capability-v1".into(),
            subject_vector: vec![1.0, 0.0],
            capability_vector: vec![0.0, 1.0],
            model: "embedder".into(),
            dimensions: 2,
        })
        .unwrap();
    assert!(repository
        .refresh_pending_slide_embedding(
            "cloud",
            "Updated subject",
            "subject-v2",
            "Updated capability",
            "capability-v2"
        )
        .unwrap());
    assert!(!repository
        .refresh_pending_slide_embedding(
            "cloud",
            "Updated subject",
            "subject-v2",
            "Updated capability",
            "capability-v2"
        )
        .unwrap());
    repository
        .claim_next_slide_embedding("2026-09-14T00:00:32Z", "2026-09-14T00:00:42Z")
        .unwrap()
        .unwrap();
    repository
        .record_slide_embedding_failure("cloud", "embedding_unavailable", None)
        .unwrap();
    drop(repository);

    let repository = crate::slide_template_test_support::repository(&path).unwrap();
    let stored = repository
        .find_slide_classification("cloud")
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, "ready");
    assert_eq!(stored.embedding_status, "failed");
    assert_eq!(
        stored.embedding_last_error_code.as_deref(),
        Some("embedding_unavailable")
    );
    assert_eq!(
        repository
            .search_slide_classifications("Quarry_WestMonroe", "cloud", 10)
            .unwrap(),
        ["cloud"]
    );
}

#[test]
fn invalid_synchronous_embedding_does_not_publish_metadata_or_fts() {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap();
    repository
        .insert_with_processing_classification(&insert("cloud"), &pending("class-v1"))
        .unwrap();
    let result = CompleteSlideProcessing {
        classification: classified("cloud", "class-v1"),
        embedding_dimensions: 2,
        embedding_model: "embedder".into(),
        subject_vector: vec![0.0, 0.0],
        capability_vector: vec![0.0, 0.0],
    };
    assert!(repository.complete_slide_processing(&result).is_err());
    let stored = repository
        .find_slide_classification("cloud")
        .unwrap()
        .unwrap();
    assert_eq!(stored.status, "processing");
    assert!(stored.metadata.is_none());
    assert!(stored.subject_vector.is_none());
    assert!(stored.capability_vector.is_none());
    assert!(repository
        .search_slide_classifications("Quarry_WestMonroe", "cloud", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn shared_database_reopens_import_assets_preview_metadata_and_both_vectors() {
    use crate::{adapters::sqlite::client::SqliteClient, app::migrations};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("quarry.sqlite3");
    let mut record = insert("durable-import");
    let png = include_bytes!(
        "../../../src/domains/templates/slide_templates/catalog/previews/layered-arch.png"
    );
    record.assets.push(TemplateAsset {
        asset_id: "durable-image".into(),
        template_id: record.template.template_id.clone(),
        content_type: "image/png".into(),
        bytes: png.to_vec(),
    });
    let decoded = image::load_from_memory(png).unwrap();
    let (width, height) = (decoded.width(), decoded.height());
    let stored_template;
    record.preview = Some(StoredTemplatePreview {
        template_id: record.template.template_id.clone(),
        content_type: "image/png".into(),
        bytes: png.to_vec(),
        width,
        height,
    });
    {
        let client = SqliteClient::open(&path).unwrap();
        migrations::migrate(&client).unwrap();
        client.with_connection(|connection| {
            connection.execute("INSERT INTO users (first_name, last_name, email, username, password_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                ["Test", "Owner", "coexist@example.test", "fixture-key", "Analyst"])?;
            Ok(())
        }).unwrap();
        let repository = SqliteTemplateRepository::new(client.clone());
        repository.ensure_app("Quarry_WestMonroe", None).unwrap();
        repository
            .insert_with_processing_classification(&record, &pending("class-v1"))
            .unwrap();
        repository
            .complete_slide_processing(&CompleteSlideProcessing {
                classification: classified("durable-import", "class-v1"),
                embedding_dimensions: 2,
                embedding_model: "embedder".into(),
                subject_vector: vec![1.0, 0.0],
                capability_vector: vec![0.0, 1.0],
            })
            .unwrap();
        stored_template = repository.find_by_id("durable-import").unwrap();
        client
            .with_connection(|connection| {
                assert_eq!(
                    connection
                        .query_row("SELECT count(*) FROM templates", [], |r| r.get::<_, i64>(0))?,
                    1
                );
                Ok(())
            })
            .unwrap();
    }
    for _ in 0..2 {
        let client = SqliteClient::open(&path).unwrap();
        migrations::migrate(&client).unwrap();
        let repository = SqliteTemplateRepository::new(client.clone());
        repository.ensure_app("Quarry_WestMonroe", None).unwrap();
        crate::domains::templates::slide_templates::catalog::seed(&repository).unwrap();
        assert_eq!(
            repository.find_by_id("durable-import").unwrap(),
            stored_template
        );
        assert_eq!(
            repository.find_metadata("durable-import").unwrap(),
            Some(record.metadata.clone())
        );
        assert_eq!(
            repository
                .find_asset("durable-import", "durable-image")
                .unwrap(),
            Some(record.assets[0].clone())
        );
        assert_eq!(
            repository.find_preview("durable-import").unwrap(),
            record.preview
        );
        let classification = repository
            .find_slide_classification("durable-import")
            .unwrap()
            .unwrap();
        assert_eq!(classification.status, "ready");
        assert_eq!(classification.embedding_status, "ready");
        assert_eq!(classification.subject_vector, Some(vec![1.0, 0.0]));
        assert_eq!(classification.capability_vector, Some(vec![0.0, 1.0]));
        assert_eq!(
            repository
                .search_slide_classifications("Quarry_WestMonroe", "cloud", 10)
                .unwrap(),
            ["durable-import"]
        );
        client
            .with_connection(|connection| {
                assert_eq!(
                    connection.query_row("SELECT first_name FROM users", [], |r| r
                        .get::<_, String>(0))?,
                    "Test"
                );
                Ok(())
            })
            .unwrap();
    }
}

#[test]
fn background_claims_never_process_another_apps_templates() {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap();
    let other = repository.for_app("Another_App").unwrap().unwrap();
    other.ensure_app("Another_App", None).unwrap();
    other
        .insert_with_pending_classification(&insert("foreign"), &pending("foreign-v1"))
        .unwrap();
    assert!(repository
        .claim_next_slide_classification("2026-11-04T00:00:00.000Z", "2026-11-04T00:03:00.000Z")
        .unwrap()
        .is_none());
    let job = other
        .claim_next_slide_classification("2026-11-04T00:00:00.000Z", "2026-11-04T00:03:00.000Z")
        .unwrap()
        .unwrap();
    assert_eq!(job.template.template_id, "foreign");
    other
        .complete_slide_classification(&classified("foreign", "foreign-v1"))
        .unwrap();
    assert!(repository
        .claim_next_slide_embedding("2026-11-04T00:00:00.000Z", "2026-11-04T00:03:00.000Z")
        .unwrap()
        .is_none());
    assert!(other
        .claim_next_slide_embedding("2026-11-04T00:00:00.000Z", "2026-11-04T00:03:00.000Z")
        .unwrap()
        .is_some());
}

#[test]
fn pending_classification_and_template_preview_commit_atomically() {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap();
    let mut record = insert("rolled-back");
    let preview = crate::slide_template_test_support::FixturePreview
        .generate(&[])
        .unwrap()
        .unwrap();
    record.preview = Some(StoredTemplatePreview {
        template_id: "rolled-back".into(),
        content_type: "image/png".into(),
        bytes: preview.bytes,
        width: preview.width,
        height: preview.height,
    });
    let mut invalid = pending("fingerprint");
    invalid.schema_version = 1;
    assert!(repository
        .insert_with_pending_classification(&record, &invalid)
        .is_err());
    assert!(repository.find_by_id("rolled-back").unwrap().is_none());
    assert!(repository.find_preview("rolled-back").unwrap().is_none());
    assert!(repository.list(None).unwrap().is_empty());
}

#[test]
fn classified_batch_publication_rolls_back_templates_and_jobs_on_later_insert_failure() {
    let repository =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap();
    let records = vec![
        (insert("duplicate"), pending("first")),
        (insert("duplicate"), pending("second")),
    ];
    assert!(repository
        .insert_many_with_pending_classification_cancellable(&records, None)
        .is_err());
    assert!(repository.list(None).unwrap().is_empty());
    assert!(repository
        .find_slide_classification("duplicate")
        .unwrap()
        .is_none());
    assert!(repository.find_by_id("duplicate").unwrap().is_none());
}

#[test]
fn classified_batch_jobs_and_previews_survive_reopen_and_cancelled_publication_is_empty() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("batch.sqlite");
    let ids = {
        let repository =
            std::sync::Arc::new(crate::slide_template_test_support::repository(&path).unwrap());
        let service = ImportService::with_preview_generator(
            repository.clone(),
            std::sync::Arc::new(crate::slide_template_test_support::FixturePreview),
        );
        let mut document = insert("source").template.template_json;
        document.presentation.slides = vec![document.presentation.slides[0].clone(); 12];
        let source = pptx::write(&document).unwrap();
        let result = service
            .queue_classified_batch_import(
                &source,
                TemplateKind::Diagram,
                &RequestCancellation::default(),
            )
            .unwrap();
        let cancellation = RequestCancellation::default();
        cancellation.cancel();
        assert!(service
            .queue_classified_batch_import(&source, TemplateKind::Diagram, &cancellation)
            .is_err());
        assert_eq!(repository.list(None).unwrap().len(), 12);
        result
            .templates
            .into_iter()
            .map(|template| template.template_id)
            .collect::<Vec<_>>()
    };
    let repository = crate::slide_template_test_support::repository(&path).unwrap();
    for id in ids {
        assert!(repository.find_by_id(&id).unwrap().is_some());
        assert!(repository.find_preview(&id).unwrap().is_some());
        let state = repository.find_slide_classification(&id).unwrap().unwrap();
        assert_eq!(state.status, "pending");
        assert_eq!(state.attempt_count, 0);
    }
}

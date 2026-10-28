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

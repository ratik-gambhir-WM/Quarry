use super::*;
use crate::{
    adapters::openai::decisions::{tests::fake_provider, DecisionsClient},
    domains::documents::{
        classification::{
            model::ResultStatus,
            policy::PRIMARY_QUESTION,
            service::ClassificationService,
            taxonomy::{Category, UNCLASSIFIED},
        },
        ingestion::persistence::{persist_document_and_chunks, CommitContext},
        store::repository::DocumentStore,
        test_support::{database, sample, save},
    },
    shared::{
        error::ServiceError,
        ids::{document_id_from_content, sha256_hex},
    },
};
use axum::http::StatusCode;

const DEAL: &str = "DEAL-SQL";

/// Admission records the content identity before any canonical write.
async fn admit(db: &PostgresClient, content: &str) -> String {
    let hash = sha256_hex(content.as_bytes());
    sqlx::query(
        "INSERT INTO document_input_identities(deal_id,input_hash,file_id) VALUES($1,$2,$3)",
    )
    .bind(DEAL)
    .bind(&hash)
    .bind(document_id_from_content(DEAL, &hash))
    .execute(db.pool())
    .await
    .unwrap();
    hash
}

fn service(endpoint: Option<&str>, db: &PostgresClient, enabled: bool) -> ClassificationService {
    ClassificationService::new(
        ClassificationRepository::new(db.clone()),
        endpoint.map(|endpoint| {
            DecisionsClient::new(reqwest::Client::new(), "test-key").with_endpoint(endpoint)
        }),
        crate::integration_support::prompt_catalog(),
        enabled,
        2,
    )
}

fn answers(primary: &str) -> String {
    let options: Vec<_> = Category::ALL
        .iter()
        .map(|c| c.id())
        .chain(std::iter::once(UNCLASSIFIED))
        .collect();
    let rest = 0.1 / (options.len() - 1) as f64;
    let probabilities: Vec<_> = options
        .iter()
        .map(|value| {
            serde_json::json!({
                "value": value,
                "probability": if *value == primary { 0.9 } else { rest }
            })
        })
        .collect();
    let mut answers = vec![serde_json::json!({
        "type": "choice",
        "name": PRIMARY_QUESTION,
        "choice": primary,
        "confidence": 0.85,
        "probabilities": probabilities,
    })];
    for category in Category::ALL {
        answers.push(serde_json::json!({
            "type": "predicate",
            "name": category.coverage_question(),
            "probability": if category == Category::DataEngineeringIntegration { 0.9 } else { 0.1 },
        }));
    }
    serde_json::json!({"model": "gpt-6-luna", "answers": answers}).to_string()
}

/// Simulate one document task: classify the parsed chunks, then commit both together.
async fn ingest(
    db: &PostgresClient,
    classification: &ClassificationService,
    content: &str,
) -> Result<ClassificationRecord, RepositoryError> {
    let (doc, chunks, bytes) = sample(content, DEAL, content, 3);
    let prepared = classification.prepare("evidence.docx", &doc, &chunks);
    let record = classification.classify(prepared).await;
    persist_document_and_chunks(
        &DocumentStore::new(db.clone()),
        DEAL,
        1,
        doc,
        chunks,
        bytes,
        CommitContext {
            attempt: None,
            classification: Some(record.clone()),
        },
    )
    .await
    .map(|_| record)
}

async fn row_count(db: &PostgresClient) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM document_classifications")
        .fetch_one(db.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn classification_commits_with_the_document_and_is_listed() {
    let db = database().await;
    let hash = admit(&db, "pipeline evidence").await;
    let (endpoint, captured) = fake_provider(
        StatusCode::OK,
        vec![],
        answers("data_engineering_integration"),
    )
    .await;
    let classification = service(Some(&endpoint), &db, true);
    let record = ingest(&db, &classification, "pipeline evidence")
        .await
        .unwrap();
    assert_eq!(record.status, RunStatus::Completed);
    assert_eq!(captured.requests.lock().unwrap().len(), 1);

    let listed = DocumentStore::new(db.clone())
        .list_for_deal(DEAL)
        .await
        .unwrap();
    let metadata = &listed[0].classification;
    assert_eq!(listed[0].content_sha256, hash);
    assert_eq!(metadata.version_id, listed[0].file_id);
    assert_eq!(metadata.run_status, "completed");
    assert_eq!(metadata.generation, Some(1));
    let result = metadata.result.as_ref().unwrap();
    assert_eq!(result.status, ResultStatus::Classified);
    assert_eq!(
        result.primary_category,
        Some(Category::DataEngineeringIntegration)
    );
    assert!(result.classified_at.is_some());
    // The raw evidence sample is never part of public metadata.
    assert!(!serde_json::to_string(&listed[0])
        .unwrap()
        .contains("pipeline evidence chunk"));

    // An exact replay of the same content keeps the committed classification.
    let failed = ClassificationRecord {
        status: RunStatus::Failed,
        result_json: None,
        error: Some(ErrorCode::Timeout),
        ..record
    };
    let (doc, chunks, bytes) = sample("pipeline evidence", DEAL, "pipeline evidence", 3);
    persist_document_and_chunks(
        &DocumentStore::new(db.clone()),
        DEAL,
        1,
        doc,
        chunks,
        bytes,
        CommitContext {
            attempt: None,
            classification: Some(failed),
        },
    )
    .await
    .unwrap();
    let listed = DocumentStore::new(db.clone())
        .list_for_deal(DEAL)
        .await
        .unwrap();
    assert_eq!(listed[0].classification.run_status, "completed");
}

#[tokio::test]
async fn a_failed_commit_saves_no_classification() {
    let db = database().await;
    admit(&db, "rolled back evidence").await;
    sqlx::raw_sql("CREATE FUNCTION reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected deferred failure'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON quarry_file_versions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_commit();")
        .execute(db.pool())
        .await
        .unwrap();
    let classification = service(None, &db, false);
    assert!(ingest(&db, &classification, "rolled back evidence")
        .await
        .is_err());
    assert_eq!(row_count(&db).await, 0);
}

#[tokio::test]
async fn provider_failures_are_saved_as_failed_without_failing_the_document() {
    let db = database().await;
    admit(&db, "unavailable evidence").await;
    let (endpoint, captured) = fake_provider(
        StatusCode::SERVICE_UNAVAILABLE,
        vec![("retry-after", "1")],
        "unavailable".into(),
    )
    .await;
    let classification = service(Some(&endpoint), &db, true);
    let record = ingest(&db, &classification, "unavailable evidence")
        .await
        .unwrap();
    assert_eq!(captured.requests.lock().unwrap().len(), 3);
    assert_eq!(
        (record.status, record.error),
        (RunStatus::Failed, Some(ErrorCode::UpstreamUnavailable))
    );
    let listed = DocumentStore::new(db.clone())
        .list_for_deal(DEAL)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1, "the document itself is saved");
    assert_eq!(listed[0].classification.run_status, "failed");
    assert_eq!(
        listed[0].classification.last_error_code.as_deref(),
        Some("upstream_unavailable")
    );
    assert_eq!(listed[0].classification.result, None);
}

#[tokio::test]
async fn invalid_output_fails_immediately_and_disabled_or_empty_input_needs_no_provider() {
    let db = database().await;
    let (endpoint, captured) = fake_provider(
        StatusCode::OK,
        vec![],
        serde_json::json!({"answers": [{"type": "refusal", "name": "covers_sdlc"}]}).to_string(),
    )
    .await;
    let classification = service(Some(&endpoint), &db, true);
    let (doc, chunks, _) = sample("invalid", DEAL, "invalid", 2);
    let record = classification
        .classify(classification.prepare("invalid.docx", &doc, &chunks))
        .await;
    assert_eq!(
        (record.status, record.error),
        (RunStatus::Failed, Some(ErrorCode::InvalidResponse))
    );
    assert_eq!(captured.requests.lock().unwrap().len(), 1);

    let disabled = service(Some(&endpoint), &db, false);
    let record = disabled
        .classify(disabled.prepare("disabled.docx", &doc, &chunks))
        .await;
    assert_eq!((record.status, record.error), (RunStatus::Disabled, None));
    assert!(!record.sample.text.is_empty(), "evidence is kept for later");

    let (doc, mut chunks, _) = sample("empty", DEAL, "empty", 1);
    chunks[0].text = "   ".into();
    let record = classification
        .classify(classification.prepare("empty.docx", &doc, &chunks))
        .await;
    assert_eq!(record.status, RunStatus::Completed);
    assert!(record
        .result_json
        .unwrap()
        .contains("insufficient_information"));
    assert_eq!(captured.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn reclassification_is_a_synchronous_generation_compare_and_set() {
    let db = database().await;
    let hash = admit(&db, "reclassify evidence").await;
    let (first, _) = fake_provider(StatusCode::OK, vec![], answers("sdlc")).await;
    ingest(
        &db,
        &service(Some(&first), &db, true),
        "reclassify evidence",
    )
    .await
    .unwrap();
    let chunks_before: i64 = sqlx::query_scalar("SELECT count(*) FROM document_chunks")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let file_id = document_id_from_content(DEAL, &hash);
    let (second, captured) =
        fake_provider(StatusCode::OK, vec![], answers("product_security")).await;
    let classification = service(Some(&second), &db, true);

    assert!(matches!(
        classification.request(DEAL, &file_id, 1, 0).await,
        Err(ServiceError::Conflict(_))
    ));
    assert!(
        captured.requests.lock().unwrap().is_empty(),
        "stale requests spend nothing"
    );
    let updated = classification.request(DEAL, &file_id, 1, 1).await.unwrap();
    assert_eq!(updated.generation, Some(2));
    assert_eq!(updated.version_id, file_id);
    assert_eq!(
        updated.result.as_ref().unwrap().primary_category,
        Some(Category::ProductSecurity)
    );
    assert!(updated.result.unwrap().classified_at.is_some());
    let listed = DocumentStore::new(db.clone())
        .list_for_deal(DEAL)
        .await
        .unwrap();
    assert_eq!(listed[0].classification.generation, Some(2));

    // A provider failure returns 503 and leaves the stored classification unchanged.
    let (broken, _) = fake_provider(StatusCode::BAD_REQUEST, vec![], "bad".into()).await;
    assert!(matches!(
        service(Some(&broken), &db, true)
            .request(DEAL, &file_id, 1, 2)
            .await,
        Err(ServiceError::Unavailable(_))
    ));
    let listed = DocumentStore::new(db.clone())
        .list_for_deal(DEAL)
        .await
        .unwrap();
    assert_eq!(listed[0].classification.generation, Some(2));

    // Never re-parses or re-embeds.
    let chunks_after: i64 = sqlx::query_scalar("SELECT count(*) FROM document_chunks")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(chunks_before, chunks_after);
    assert!(matches!(
        classification.request(DEAL, &"0".repeat(64), 1, 0).await,
        Err(ServiceError::NotFound(_))
    ));
    assert!(matches!(
        classification.request("DEAL-FOREIGN", &file_id, 1, 2).await,
        Err(ServiceError::NotFound(_) | ServiceError::Forbidden)
    ));
}

#[tokio::test]
async fn documents_without_a_classification_are_resampled_from_stored_chunks() {
    let db = database().await;
    save(&db, "legacy evidence", DEAL, "legacy evidence", 2).await;
    let hash = admit(&db, "legacy evidence").await;
    let listed = DocumentStore::new(db.clone())
        .list_for_deal(DEAL)
        .await
        .unwrap();
    assert_eq!(listed[0].classification.run_status, "not_requested");
    let (endpoint, _) = fake_provider(StatusCode::OK, vec![], answers("sdlc")).await;
    let file_id = document_id_from_content(DEAL, &hash);
    let classified = service(Some(&endpoint), &db, true)
        .request(DEAL, &file_id, 1, 0)
        .await
        .unwrap();
    assert_eq!(classified.generation, Some(1));
    assert!(matches!(
        service(None, &db, false)
            .request(DEAL, &file_id, 1, 1)
            .await,
        Err(ServiceError::Unavailable(_))
    ));
}

#[tokio::test]
async fn parent_deletion_cascades_the_classification() {
    let db = database().await;
    admit(&db, "cascade evidence").await;
    ingest(&db, &service(None, &db, false), "cascade evidence")
        .await
        .unwrap();
    assert_eq!(row_count(&db).await, 1);
    sqlx::query("DELETE FROM document_input_identities")
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(row_count(&db).await, 0);
}

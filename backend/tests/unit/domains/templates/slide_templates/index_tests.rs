use super::*;
use crate::{domains::templates::slide_templates::*, slide_template_test_support as support};
use std::path::Path;
struct Classifier;
impl SlideClassifier for Classifier {
    fn classify<'a>(
        &'a self,
        _: &'a SlideClassificationInput,
    ) -> ProviderFuture<'a, SlideRetrievalMetadata> {
        Box::pin(async {
            normalize_slide_retrieval_metadata(serde_json::json!({"subject":{"summary":"Cloud architecture.","domains":[{"id":"cloud","relevance":"primary","topics":["cloud"]}],"otherTopics":[],"technologies":[],"entities":[],"claims":[],"synonyms":[]},"communication":{"intents":["explanation"],"informationTypes":[],"audience":[]},"templateFit":{"archetype":"architecture","contentSlots":[]},"visual":{"layoutType":"diagram-led","visualElements":[],"contentDensity":"low","structuralFeatures":[]},"retrievalKeywords":["cloud"]})).map_err(|_|SlideProviderError::new("fixture_invalid",false))
        })
    }
}
struct Embedder;
impl SlideEmbedder for Embedder {
    fn embed<'a>(&'a self, _: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async { Ok(vec![1.0, 0.5]) })
    }
}
async fn record() -> TemplateProjectionRecord {
    let repository = Arc::new(support::repository(Path::new(":memory:")).unwrap());
    let importer = ImportService::new(repository.clone());
    let source = include_bytes!("../../../../fixtures/slide_templates/pptx/styled.pptx");
    let (_, job) = importer
        .prepare_import_v2_cancellable(
            source,
            TemplateKind::Diagram,
            &RequestCancellation::default(),
        )
        .unwrap();
    let classifier = SlideClassificationService::new(SlideClassificationServiceOptions {
        classifier: Arc::new(Classifier),
        classification_model: "fixture-classifier".into(),
        embedder: Arc::new(Embedder),
        embedding_dimensions: Some(2),
        embedding_max_text_bytes: 32_000,
        embedding_model: "fixture-embedder".into(),
        input_limits: Default::default(),
        repository,
    });
    let processed = classifier.process(&job).await.unwrap();
    importer.complete_processing(&processed).unwrap();
    importer
        .projection_record(&job.template.template_id)
        .unwrap()
        .unwrap()
}
#[tokio::test]
async fn both_vector_roles_and_complete_metadata_publish_after_bounded_parts() {
    let queries = prepare_record(record().await, NonZeroUsize::new(2).unwrap()).unwrap();
    assert!(queries.len() >= 3);
    for query in &queries {
        assert!(query.to_json_bytes().unwrap().len() <= MAX_QUERY);
    }
    let publish = String::from_utf8(queries[queries.len() - 2].to_json_bytes().unwrap()).unwrap();
    assert!(publish.contains(SUBJECT));
    assert!(publish.contains(CAPABILITY));
    assert!(publish.contains("embedding_fingerprint"));
    assert!(publish.contains("fixture-embedder"));
    let staged = String::from_utf8(queries[0].to_json_bytes().unwrap()).unwrap();
    assert!(staged.contains("metadata"));
    assert!(staged.contains("subject_vector") || staged.contains("subjectVector"));
    assert!(prepare_record(record().await, NonZeroUsize::new(3).unwrap()).is_err());
}
#[tokio::test]
#[ignore = "requires explicitly disposable Helix at 127.0.0.1:16969"]
async fn live_helix_templates_metadata_dual_vectors_and_deletion() {
    let helix = Arc::new(HelixClient::with_config("http://127.0.0.1:16969", None).unwrap());
    let index = TemplateIndex::new(helix.clone(), NonZeroUsize::new(2).unwrap());
    index.initialize().await.unwrap();
    let record = record().await;
    let id = record.template.template_id.clone();
    index.upsert(record.clone()).await.unwrap();
    index.upsert(record.clone()).await.unwrap();
    let root: Value = helix
        .execute_read_query(
            "test.templates.root",
            QueryRequest::read(
                read_batch()
                    .var_as(
                        "items",
                        scoped(ROOT, &id).project(vec![
                            PropertyProjection::new("generation"),
                            PropertyProjection::new("part_count"),
                            PropertyProjection::new("title"),
                        ]),
                    )
                    .returning(["items"]),
            ),
        )
        .await
        .unwrap();
    assert_eq!(root["items"].as_array().unwrap().len(), 1);
    assert_eq!(root["items"][0]["title"], record.metadata.title);
    for label in [SUBJECT, CAPABILITY] {
        let vector: Value = helix
            .execute_read_query(
                "test.templates.vector",
                QueryRequest::read(
                    read_batch()
                        .var_as(
                            "items",
                            scoped(label, &id).project(vec![
                                PropertyProjection::new("embedding"),
                                PropertyProjection::new("embedding_model"),
                                PropertyProjection::new("embedding_dimensions"),
                            ]),
                        )
                        .returning(["items"]),
                ),
            )
            .await
            .unwrap();
        assert_eq!(vector["items"].as_array().unwrap().len(), 1);
        assert_eq!(vector["items"][0]["embedding"].as_array().unwrap().len(), 2);
        assert_eq!(vector["items"][0]["embedding_model"], "fixture-embedder");
    }
    let parts: Value = helix
        .execute_read_query(
            "test.templates.parts",
            QueryRequest::read(
                read_batch()
                    .var_as(
                        "items",
                        scoped(PART, &id)
                            .where_(Predicate::eq(
                                "generation",
                                root["items"][0]["generation"].as_str().unwrap().to_owned(),
                            ))
                            .project(vec![
                                PropertyProjection::new("part_index"),
                                PropertyProjection::new("json_fragment"),
                            ]),
                    )
                    .returning(["items"]),
            ),
        )
        .await
        .unwrap();
    let mut parts = parts["items"].as_array().unwrap().clone();
    parts.sort_by_key(|p| p["part_index"].as_i64().unwrap());
    let document = parts
        .iter()
        .map(|p| p["json_fragment"].as_str().unwrap())
        .collect::<String>();
    let projected: Value = serde_json::from_str(&document).unwrap();
    assert_eq!(
        projected["template"]["template_json"]["presentation"]["title"],
        record.metadata.title
    );
    assert_eq!(projected["classification"]["subjectVector"][0], 1.0);
    index.delete(&id).await.unwrap();
    for label in [ROOT, PART, SUBJECT, CAPABILITY] {
        let found: Value = helix
            .execute_read_query(
                "test.templates.deleted",
                QueryRequest::read(
                    read_batch()
                        .var_as(
                            "items",
                            scoped(label, &id)
                                .project(vec![PropertyProjection::new("template_id")]),
                        )
                        .returning(["items"]),
                ),
            )
            .await
            .unwrap();
        assert!(found["items"].as_array().unwrap().is_empty());
    }
}

#[test]
fn builtins_project_preview_metadata_without_copying_binary_preview_data() {
    let repository = Arc::new(support::repository(Path::new(":memory:")).unwrap());
    crate::domains::templates::slide_templates::catalog::seed(repository.as_ref()).unwrap();
    let importer = ImportService::new(repository);
    let record = importer
        .projection_record("layered-architecture")
        .unwrap()
        .unwrap();
    let preview = record.preview.as_ref().unwrap();
    assert_eq!(preview.content_type, "image/png");
    assert!(preview.width > 0 && preview.height > 0);
    assert_eq!(preview.url.matches("appId=").count(), 1);
    let projected = serde_json::to_value(&record).unwrap();
    assert!(projected["preview"].get("bytes").is_none());
    assert!(projected["metadata"]["catalog_checksum"].is_string());
}

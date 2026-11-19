use super::super::{
    providers::*, StoredTemplateMetadata, TemplateInsert, TemplateRepository, TemplateSource,
};
use super::*;
use crate::domains::templates::slide_templates::{
    SlideClassificationService, SlideClassificationServiceOptions, TemplateRetrievalRepository,
};
use serde_json::{json, Value};
use std::sync::Arc;

fn metadata() -> SlideRetrievalMetadata {
    normalize_slide_retrieval_metadata(json!({
        "subject": {
            "summary":"A security testing architecture with data integrations.",
            "domains":[
                {"id":"cybersecurity","relevance":"primary","topics":["Security Testing","security-testing"]},
                {"id":"software-architecture","relevance":"secondary","topics":["Extensibility"]}
            ],
            "otherTopics":[],"technologies":["Salesforce","Snowflake"],"entities":["PolicyCenter"],
            "claims":["Testing coverage has a control gap"],"synonyms":["Application security assurance"]
        },
        "communication":{"intents":["finding","evidence","recommendation"],"informationTypes":["Observations","Gaps","Remediation Actions"],"audience":["Technology diligence"]},
        "templateFit":{"archetype":"Finding Evidence Recommendation","contentSlots":[{"elementId":"text-1","role":"Headline","capacity":"short-text"}]},
        "visual":{"layoutType":"diagram-led","visualElements":["System Boxes","Directional Connectors"],"contentDensity":"medium","structuralFeatures":["Three tiers","Legend"]},
        "retrievalKeywords":["Application stack","System integration"]
    }))
    .unwrap()
}

fn comparison_metadata() -> SlideRetrievalMetadata {
    let mut metadata = metadata();
    metadata.communication.intents = vec!["comparison".into()];
    metadata
}

async fn insert(repository: &dyn TemplateRepository, id: &str, title: &str) {
    let template = StoredTemplate { revision: 1,
        template_id: id.to_owned(),
        template_json: serde_json::from_value(json!({
            "presentation":{"title":title,"preserveElementOrder":true,"showBranding":false,"slides":[{
                "id":"slide-1","name":"Architecture","width":1280,"height":720,"backgroundColor":"FFFFFF",
                "elements":[{"id":"text-1","type":"text","x":20,"y":20,"w":300,"h":60,"text":"Security finding","fill":"transparent","stroke":"transparent","strokeWidth":0,"textColor":"000000","fontSize":18,"fontFace":"Arial","padding":0,"align":"left","valign":"top","runs":[]}]
            }]}
        }))
        .unwrap(),
    };
    let fingerprint =
        build_slide_classification_input_fingerprint(&template.template_json, None).unwrap();
    let pending = PendingSlideClassification {
        input_fingerprint: fingerprint.clone(),
        prompt_version: SLIDE_CLASSIFICATION_PROMPT_VERSION.into(),
        schema_version: SLIDE_CLASSIFICATION_SCHEMA_VERSION,
    };
    repository
        .insert_with_pending_classification(
            &TemplateInsert {
                template: template.clone(),
                assets: vec![],
                metadata: StoredTemplateMetadata {
                    template_id: id.to_owned(),
                    kind: TemplateKind::Diagram,
                    title: title.to_owned(),
                    description: String::new(),
                    source: TemplateSource::Import,
                    created_at: 10,
                    catalog_version: None,
                    catalog_checksum: None,
                },
                preview: None,
            },
            &pending,
        )
        .await
        .unwrap();
}

/// Run both durable stages through their claims, as the worker does.
async fn process(repository: &dyn TemplateRepository, service: &SlideClassificationService) {
    let job = repository
        .claim_next_slide_classification(60)
        .await
        .unwrap()
        .unwrap();
    service.classify(job).await.unwrap();
    let job = repository
        .claim_next_slide_embedding(60)
        .await
        .unwrap()
        .unwrap();
    service.embed(&job).await.unwrap();
}

struct Classifier;
impl SlideClassifier for Classifier {
    fn classify<'a>(
        &'a self,
        input: &'a SlideClassificationInput,
    ) -> ProviderFuture<'a, SlideRetrievalMetadata> {
        Box::pin(async move {
            Ok(if input.digest.contains("Other Architecture") {
                comparison_metadata()
            } else {
                metadata()
            })
        })
    }
}

struct Embedder;
impl SlideEmbedder for Embedder {
    fn embed<'a>(&'a self, text: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async move {
            Ok(
                if text.contains("Other") || text.contains("finding | evidence | recommendation") {
                    vec![0.0, 1.0]
                } else {
                    vec![1.0, 0.0]
                },
            )
        })
    }
}

fn service(repository: Arc<dyn TemplateRepository>) -> SlideClassificationService {
    SlideClassificationService::new(SlideClassificationServiceOptions {
        classifier: Arc::new(Classifier),
        classification_model: "classifier".into(),
        embedder: Arc::new(Embedder),
        embedding_dimensions: Some(2),
        embedding_max_text_bytes: 32_000,
        embedding_model: "embedding".into(),
        input_limits: SlideClassificationInputLimits::default(),
        repository,
    })
}

fn query(mode: &str) -> Value {
    json!({
        "mode":mode,"query":null,"semantic_target":"both","template_ids":[],"similar_to_template_id":null,"facet_name":null,
        "filters":{"kinds":[],"domains":[],"topics":[],"intents":[],"archetypes":[],"slot_roles":[],"content_density":[]},
        "select":["subject"],"limit":20
    })
}

#[test]
fn metadata_and_embedding_documents_are_normalized_and_bounded() {
    let metadata = metadata();
    assert_eq!(metadata.subject.domains[0].topics, ["security-testing"]);
    assert_eq!(metadata.template_fit.content_slots[0].role, "headline");
    assert_eq!(metadata.visual.layout_type, "diagram-led");
    let documents = build_slide_embedding_documents(
        "Product Architecture",
        TemplateKind::Diagram,
        &metadata,
        10_000,
    )
    .unwrap();
    assert!(documents
        .subject
        .contains("technologies: Salesforce | Snowflake"));
    assert!(documents
        .capability
        .contains("communication intents: finding | evidence | recommendation"));
    assert!(build_slide_embedding_documents(
        "Product Architecture",
        TemplateKind::Diagram,
        &metadata,
        10
    )
    .is_err());
    assert_ne!(
        build_slide_embedding_fingerprint("subject", &documents.subject, "embedding", Some(2)),
        build_slide_embedding_fingerprint(
            "capability",
            &documents.capability,
            "embedding",
            Some(2)
        ),
    );
}

#[tokio::test]
async fn dual_vectors_drive_query_similarity_and_finding_retrieval() {
    let repository: Arc<dyn TemplateRepository> = Arc::new(
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let scoped = repository.clone();
    let service = service(scoped.clone());
    insert(scoped.as_ref(), "security", "Security Architecture").await;
    process(scoped.as_ref(), &service).await;
    insert(scoped.as_ref(), "other", "Other Architecture").await;
    process(scoped.as_ref(), &service).await;
    let retrieval = SlideRetrievalService {
        repository: repository.clone(),
        embedder: Some(Arc::new(Embedder)),
        profile: Some(TemplateEmbeddingProfile::new("embedding", 2)),
    };

    let mut semantic = query("semantic");
    semantic["query"] = json!("Security testing");
    semantic["semantic_target"] = json!("subject");
    assert_eq!(
        retrieval.query(&semantic).await.unwrap()["results"][0]["templateId"],
        "security"
    );
    semantic["semantic_target"] = json!("capability");
    assert_eq!(
        retrieval.query(&semantic).await.unwrap()["results"][0]["templateId"],
        "other"
    );

    let mut similar = query("similar");
    similar["similar_to_template_id"] = json!("security");
    assert_eq!(
        retrieval.query(&similar).await.unwrap()["results"][0]["templateId"],
        "other"
    );

    let mut facets = query("facets");
    facets["facet_name"] = json!("domains");
    assert_eq!(
        retrieval.query(&facets).await.unwrap()["values"][0],
        json!({"count":2,"value":"cybersecurity"})
    );

    let finding = retrieval.find_for_finding(&json!({"markdown":"# Finding\nSecurity testing control gap\n## Evidence\nCoverage is incomplete\n## Recommendation\nExpand testing"})).await.unwrap();
    assert_eq!(finding["results"][0]["templateId"], "security");
    assert_eq!(
        finding["results"][0]["matchedDomains"],
        json!(["cybersecurity"])
    );
    assert_eq!(
        finding["results"][0]["matchedIntents"],
        json!(["finding", "evidence", "recommendation"])
    );
}

/// Reference exact cosine in f64, used only to check SQL ranking parity.
fn reference_cosine(left: &[f32], right: &[f32]) -> f64 {
    let (mut dot, mut l, mut r) = (0.0, 0.0, 0.0);
    for (&a, &b) in left.iter().zip(right) {
        let (a, b) = (f64::from(a), f64::from(b));
        dot += a * b;
        l += a * a;
        r += b * b;
    }
    dot / (l * r).sqrt()
}

struct VectorEmbedder(Vec<f64>);
impl SlideEmbedder for VectorEmbedder {
    fn embed<'a>(&'a self, _: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async move { Ok(self.0.clone()) })
    }
}

#[tokio::test]
async fn sql_cosine_ranking_matches_reference_scores_ties_and_profile_scope() {
    use crate::adapters::postgres::vector::CosineVector;
    let owner = crate::slide_template_test_support::repository(std::path::Path::new(":memory:"))
        .await
        .unwrap();
    let other = owner.other_user_fixture().await;
    let repository: Arc<dyn TemplateRepository> = Arc::new(owner);
    let scoped = repository.clone();
    // Distinct, tied and opposite vectors; "tie-a"/"tie-b" share a score and creation time.
    let fixtures: [(&str, [f64; 2]); 5] = [
        ("exact", [1.0, 0.0]),
        ("tie-b", [1.0, 1.0]),
        ("tie-a", [2.0, 2.0]),
        ("orthogonal", [0.0, 1.0]),
        ("opposite", [-1.0, 0.1]),
    ];
    for (id, vector) in fixtures {
        insert(scoped.as_ref(), id, id).await;
        let service = SlideClassificationService::new(SlideClassificationServiceOptions {
            classifier: Arc::new(Classifier),
            classification_model: "classifier".into(),
            embedder: Arc::new(VectorEmbedder(vector.to_vec())),
            embedding_dimensions: Some(2),
            embedding_max_text_bytes: 32_000,
            embedding_model: "embedding".into(),
            input_limits: SlideClassificationInputLimits::default(),
            repository: scoped.clone(),
        });
        process(scoped.as_ref(), &service).await;
    }
    let profile = TemplateEmbeddingProfile::new("embedding", 2);
    let query = CosineVector::from_f64(&[0.9, 0.1], 2).unwrap();
    let ranked = repository
        .rank_slide_embeddings(&profile.id(), EmbeddingRole::Subject, &query)
        .await
        .unwrap();
    assert_eq!(ranked.len(), fixtures.len());
    for ranked in &ranked {
        let stored = fixtures
            .iter()
            .find(|(id, _)| *id == ranked.template_id)
            .unwrap()
            .1;
        let stored = CosineVector::from_f64(&stored, 2).unwrap();
        let expected = reference_cosine(query.as_slice(), stored.as_slice());
        assert!(
            (ranked.similarity - expected).abs() < 1e-6,
            "{}: {} vs {expected}",
            ranked.template_id,
            ranked.similarity
        );
    }
    // Descending score polarity with stable identity ties.
    assert_eq!(
        ranked
            .iter()
            .map(|r| r.template_id.as_str())
            .collect::<Vec<_>>(),
        ["exact", "tie-a", "tie-b", "orthogonal", "opposite"]
    );
    // Another profile or another user ranks nothing.
    assert!(repository
        .rank_slide_embeddings(
            &TemplateEmbeddingProfile::new("embedding", 3).id(),
            EmbeddingRole::Subject,
            &query
        )
        .await
        .unwrap()
        .is_empty());
    assert!(other
        .rank_slide_embeddings(&profile.id(), EmbeddingRole::Subject, &query)
        .await
        .unwrap()
        .is_empty());

    // The service ranks the complete compatible list, so a semantic winner is found
    // even when the output limit is one, and "similar" needs no query provider.
    let retrieval = SlideRetrievalService {
        repository: repository.clone(),
        embedder: Some(Arc::new(VectorEmbedder(vec![-1.0, 0.0]))),
        profile: Some(profile.clone()),
    };
    let mut semantic = query_input("semantic");
    semantic["query"] = json!("anything");
    semantic["semantic_target"] = json!("subject");
    semantic["limit"] = json!(1);
    assert_eq!(
        retrieval.query(&semantic).await.unwrap()["results"],
        json!([{"kind":"diagram","previewUrl":null,"sections":{"subject":serde_json::to_value(&metadata().subject).unwrap()},"templateId":"opposite","title":"opposite"}])
    );
    let without_provider = SlideRetrievalService {
        repository: repository.clone(),
        embedder: None,
        profile: Some(profile.clone()),
    };
    let mut similar = query_input("similar");
    similar["similar_to_template_id"] = json!("exact");
    let results = without_provider.query(&similar).await.unwrap();
    let ids = results["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["templateId"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(!ids.contains(&"exact".to_owned()));
    assert_eq!(ids.len(), fixtures.len() - 1);
    // Missing sources remain unavailable in the owner's catalog.
    let mut missing = similar.clone();
    missing["similar_to_template_id"] = json!("missing");
    let error = without_provider.query(&missing).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<SlideRetrievalError>().unwrap().code,
        "slide_not_available"
    );
    // Semantic search without a provider fails explicitly; keyword-free hybrid degrades.
    let error = without_provider.query(&semantic).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<SlideRetrievalError>().unwrap().code,
        "semantic_unavailable"
    );
    let mut hybrid = query_input("hybrid");
    hybrid["query"] = json!("security");
    let hybrid = without_provider.query(&hybrid).await.unwrap();
    assert_eq!(hybrid["warnings"], json!(["semantic_unavailable"]));
}

fn query_input(mode: &str) -> Value {
    query(mode)
}

#[tokio::test]
async fn large_catalog_preserves_filtered_matches_and_complete_role_fusion() {
    use crate::adapters::postgres::client::PostgresClient;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("large-retrieval-catalog");
    let client = PostgresClient::test_database_at(&path).await.unwrap();
    let repository = Arc::new(
        crate::slide_template_test_support::repository(&path)
            .await
            .unwrap(),
    );
    let scoped = repository.clone();
    insert(scoped.as_ref(), "target", "Target").await;
    process(scoped.as_ref(), &service(scoped.clone())).await;

    // Seed a complete catalog in bulk rather than invoking 10,001 provider workflows.
    // Target is last for subject, first for capability, and newer than the runner-up.
    let mut tx = client.pool().begin().await.unwrap();
    sqlx::query("UPDATE template_embeddings SET embedding=CASE role WHEN 'subject' THEN '[0,1]'::quarry_extensions.vector ELSE '[1,0]'::quarry_extensions.vector END WHERE template_id='target'")
        .execute(&mut *tx).await.unwrap();
    sqlx::query(
        "INSERT INTO templates(template_id,template_json,owner_user_id)
        SELECT 'candidate-'||n,template_json,owner_user_id FROM templates
        CROSS JOIN generate_series(0,10000) n WHERE template_id='target'",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO template_metadata(template_id,kind,description,source,created_at)
        SELECT template_id,CASE WHEN template_id='candidate-0' THEN 'diagram' ELSE 'commentary' END,
            '', 'import', '1970-01-01T00:00:00.009Z' FROM templates WHERE template_id<>'target'",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO template_classification_status(template_id,status,metadata_json,
            embedding_status,subject_embedding_document,subject_embedding_fingerprint,
            capability_embedding_document,capability_embedding_fingerprint,embedding_profile_id,
            input_fingerprint,classified_fingerprint,schema_version,prompt_version)
        SELECT t.template_id,'ready',CASE WHEN t.template_id='candidate-0' THEN $1 ELSE s.metadata_json END,
            'ready',s.subject_embedding_document,s.subject_embedding_fingerprint,
            s.capability_embedding_document,s.capability_embedding_fingerprint,s.embedding_profile_id,
            s.input_fingerprint,s.classified_fingerprint,s.schema_version,s.prompt_version
        FROM templates t CROSS JOIN template_classification_status s
        WHERE t.template_id<>'target' AND s.template_id='target'")
        .bind(serde_json::to_string(&comparison_metadata()).unwrap())
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO template_embeddings(template_id,role,profile_id,dimensions,input_fingerprint,embedding)
        SELECT t.template_id,e.role,e.profile_id,e.dimensions,e.input_fingerprint,
            CASE WHEN e.role='subject' THEN '[1,0]'::quarry_extensions.vector
                 WHEN t.template_id='candidate-0' THEN '[1,1]'::quarry_extensions.vector
                 ELSE '[0,1]'::quarry_extensions.vector END
        FROM templates t CROSS JOIN template_embeddings e
        WHERE t.template_id<>'target' AND e.template_id='target'")
        .execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    let retrieval = SlideRetrievalService {
        repository,
        embedder: Some(Arc::new(VectorEmbedder(vec![1.0, 0.0]))),
        profile: Some(TemplateEmbeddingProfile::new("embedding", 2)),
    };
    let mut input = query("semantic");
    input["query"] = json!("anything");
    input["semantic_target"] = json!("subject");
    input["filters"]["kinds"] = json!(["diagram"]);
    input["filters"]["intents"] = json!(["finding"]);
    input["limit"] = json!(1);
    let filtered = retrieval.query(&input).await.unwrap();
    assert_eq!(filtered["results"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["results"][0]["templateId"], "target");

    // Both diagrams tie in complete reciprocal-rank fusion: (2,1) versus (1,2).
    // Recency selects target. Truncating its subject rank incorrectly selects candidate-0.
    input["semantic_target"] = json!("both");
    input["filters"]["intents"] = json!([]);
    let fused = retrieval.query(&input).await.unwrap();
    assert_eq!(fused["results"].as_array().unwrap().len(), 1);
    assert_eq!(fused["results"][0]["templateId"], "target");
    client.close().await;
}

#[tokio::test]
async fn retrieval_and_tools_cannot_read_another_users_catalog() {
    let owner = Arc::new(
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let other = Arc::new(owner.other_user_fixture().await);
    insert(owner.as_ref(), "private-slide", "Private architecture").await;
    process(owner.as_ref(), &service(owner.clone())).await;
    let retrieval = Arc::new(SlideRetrievalService {
        repository: other.clone(),
        embedder: Some(Arc::new(Embedder)),
        profile: Some(TemplateEmbeddingProfile::new("embedding", 2)),
    });
    let mut input = query("semantic");
    input["query"] = json!("architecture");
    assert_eq!(retrieval.query(&input).await.unwrap()["results"], json!([]));
    let mut similar = query("similar");
    similar["similar_to_template_id"] = json!("private-slide");
    let error = retrieval.query(&similar).await.unwrap_err();
    assert_eq!(
        error.downcast_ref::<SlideRetrievalError>().unwrap().code,
        "slide_not_available"
    );
    assert_eq!(
        retrieval
            .find_for_finding(&json!({"markdown":"Architecture finding"}))
            .await
            .unwrap()["results"],
        json!([])
    );
    let tools = SlideAgentTools {
        retrieval,
        templates: super::super::ImportService::new(other),
    };
    assert_eq!(
        tools
            .get_slide(&json!({"template_id":"private-slide"}))
            .await
            .unwrap()["error"]["code"],
        "slide_not_available"
    );
    assert!(tools
        .get_slide(&json!({"template_id":"private-slide", "user_id":1}))
        .await
        .is_err());
}

use super::super::{
    providers::*, StoredTemplateMetadata, TemplateInsert, TemplateRepository, TemplateSource,
};
use super::*;
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

fn insert(repository: &dyn TemplateRepository, id: &str, title: &str) -> SlideClassificationJob {
    let template = StoredTemplate {
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
        .insert_with_processing_classification(
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
        .unwrap();
    SlideClassificationJob {
        attempt_count: 1,
        input_fingerprint: fingerprint,
        kind: TemplateKind::Diagram,
        preview: None,
        template,
        title: title.to_owned(),
    }
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
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap(),
    );
    let app = repository.for_app("TestApp").unwrap().unwrap();
    let service = service(Arc::clone(&app));
    service
        .process_and_persist(&insert(app.as_ref(), "security", "Security Architecture"))
        .await
        .unwrap();
    service
        .process_and_persist(&insert(app.as_ref(), "other", "Other Architecture"))
        .await
        .unwrap();
    let retrieval = SlideRetrievalService {
        repository: Arc::clone(&repository),
        embedder: Some(Arc::new(Embedder)),
        embedding_model: Some("embedding".into()),
        embedding_dimensions: Some(2),
    };

    let mut semantic = query("semantic");
    semantic["query"] = json!("Security testing");
    semantic["semantic_target"] = json!("subject");
    assert_eq!(
        retrieval.query("TestApp", &semantic).await.unwrap()["results"][0]["templateId"],
        "security"
    );
    semantic["semantic_target"] = json!("capability");
    assert_eq!(
        retrieval.query("TestApp", &semantic).await.unwrap()["results"][0]["templateId"],
        "other"
    );

    let mut similar = query("similar");
    similar["similar_to_template_id"] = json!("security");
    assert_eq!(
        retrieval.query("TestApp", &similar).await.unwrap()["results"][0]["templateId"],
        "other"
    );

    let mut facets = query("facets");
    facets["facet_name"] = json!("domains");
    assert_eq!(
        retrieval.query("TestApp", &facets).await.unwrap()["values"][0],
        json!({"count":2,"value":"cybersecurity"})
    );

    let finding = retrieval.find_for_finding("TestApp", &json!({"markdown":"# Finding\nSecurity testing control gap\n## Evidence\nCoverage is incomplete\n## Recommendation\nExpand testing"})).await.unwrap();
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

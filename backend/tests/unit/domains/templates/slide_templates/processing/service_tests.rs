use super::*;
use crate::domains::templates::slide_templates::ProviderFuture;

struct PairEmbedder {
    capability: Vec<f64>,
}
impl SlideEmbedder for PairEmbedder {
    fn embed<'a>(&'a self, text: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async move {
            Ok(if text == "subject" {
                vec![1.0, 0.5]
            } else {
                self.capability.clone()
            })
        })
    }
}

#[tokio::test]
async fn embedding_pairs_infer_dimensions_and_reject_invalid_capability_vectors() {
    use crate::slide_template_test_support as support;
    for dimensions in [None, Some(2)] {
        for (capability, valid) in [
            (vec![0.5, 1.0], true),
            (vec![1.0], false),
            (vec![0.0, 0.0], false),
            (vec![f64::NAN, 1.0], false),
        ] {
            let service = SlideClassificationService::new(SlideClassificationServiceOptions {
                classifier: Arc::new(support::FixtureClassifier),
                classification_model: "fixture".into(),
                embedder: Arc::new(PairEmbedder { capability }),
                embedding_dimensions: dimensions,
                embedding_max_text_bytes: 32000,
                embedding_model: "fixture".into(),
                input_limits: Default::default(),
                repository: Arc::new(
                    support::repository(std::path::Path::new(":memory:"))
                        .await
                        .unwrap(),
                ),
            });
            let pair = service
                .prepare_embeddings(&SlideEmbeddingJob {
                    attempt_count: 1,
                    claim: StageClaim {
                        template_id: "t".into(),
                        token: "token".into(),
                    },
                    capability_document: "capability".into(),
                    capability_fingerprint: "capability-fingerprint".into(),
                    subject_document: "subject".into(),
                    subject_fingerprint: "subject-fingerprint".into(),
                    template_id: "t".into(),
                })
                .await;
            if valid {
                let pair = pair.unwrap();
                assert_eq!(pair.profile, TemplateEmbeddingProfile::new("fixture", 2));
                assert_eq!(pair.subject.as_slice(), [1.0, 0.5]);
                assert_eq!(pair.capability.as_slice(), [0.5, 1.0]);
            } else {
                assert!(pair.is_err());
            }
        }
    }
}

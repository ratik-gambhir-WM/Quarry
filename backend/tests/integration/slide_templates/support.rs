use crate::{adapters::sqlite::client::SqliteClient, domains::templates::slide_templates::*};
use std::{path::Path, sync::Arc};
pub fn repository(path: &Path) -> anyhow::Result<SqliteTemplateRepository> {
    let client = if path == std::path::Path::new(":memory:") {
        SqliteClient::open_in_memory()?
    } else {
        SqliteClient::open(path)?
    };
    crate::app::migrations::migrate(&client)?;
    let repository = SqliteTemplateRepository::new(client);
    repository.ensure_app("Quarry_WestMonroe", None)?;
    Ok(repository)
}
pub fn studio(seed: bool) -> Arc<SlideTemplates> {
    let repository = Arc::new(repository(std::path::Path::new(":memory:")).unwrap());
    if seed {
        crate::domains::templates::slide_templates::catalog::seed(repository.as_ref()).unwrap();
    }
    Arc::new(SlideTemplates::new(
        repository,
        Arc::new(DisabledTemplatePreviewGenerator),
        None,
        None,
        false,
    ))
}
pub fn chromium_candidates() -> Vec<std::path::PathBuf> {
    let values = std::env::var("HOME")
        .ok()
        .map(|home| vec![("HOME", home)])
        .unwrap_or_default();
    crate::app::config::AppConfig::from_values(values)
        .unwrap()
        .slide_templates
        .chromium_path
        .into_iter()
        .collect()
}
pub struct FixtureClassifier;
impl SlideClassifier for FixtureClassifier {
    fn classify<'a>(
        &'a self,
        _: &'a SlideClassificationInput,
    ) -> ProviderFuture<'a, SlideRetrievalMetadata> {
        Box::pin(async {
            normalize_slide_retrieval_metadata(serde_json::json!({"subject":{"summary":"Cloud architecture.","domains":[{"id":"cloud","relevance":"primary","topics":["cloud"]}],"otherTopics":[],"technologies":[],"entities":[],"claims":[],"synonyms":[]},"communication":{"intents":["explanation"],"informationTypes":[],"audience":[]},"templateFit":{"archetype":"architecture","contentSlots":[]},"visual":{"layoutType":"diagram-led","visualElements":[],"contentDensity":"low","structuralFeatures":[]},"retrievalKeywords":["cloud"]})).map_err(|_|SlideProviderError::new("fixture_invalid",false))
        })
    }
}
pub struct FixtureEmbedder(pub bool);
impl SlideEmbedder for FixtureEmbedder {
    fn embed<'a>(&'a self, _: &'a str) -> ProviderFuture<'a, Vec<f64>> {
        Box::pin(async move {
            if self.0 {
                Err(SlideProviderError::new("embedding_timeout", true))
            } else {
                Ok(vec![1.0, 0.5])
            }
        })
    }
}

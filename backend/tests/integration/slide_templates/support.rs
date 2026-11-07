use crate::{adapters::sqlite::client::SqliteClient, domains::templates::slide_templates::*};
use std::{path::Path, sync::Arc};
pub fn repository(path: &Path) -> anyhow::Result<SqliteTemplateRepository> {
    let client = if path == std::path::Path::new(":memory:") {
        SqliteClient::open_in_memory()?
    } else {
        SqliteClient::open(path)?
    };
    crate::app::migrations::migrate(&client)?;
    let has_user = client.with_connection(|connection| {
        connection.query_row("SELECT EXISTS(SELECT 1 FROM users WHERE id=1)", [], |row| {
            row.get::<_, bool>(0)
        })
    })?;
    if !has_user {
        crate::integration_support::seed_identity(&client, "template-fixture@example.com", 't');
    }
    let repository = SqliteTemplateRepository::new(client).user_scope(1)?;
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
    let values = ["HOME", "QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
        .collect::<Vec<_>>();
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

/// A real bounded PNG without browser or provider dependencies.
pub struct FixturePreview;
impl TemplatePreviewGenerator for FixturePreview {
    fn generate(&self, _: &[u8]) -> anyhow::Result<Option<GeneratedTemplatePreview>> {
        let image = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 255]));
        let mut output = std::io::Cursor::new(Vec::new());
        image.write_to(&mut output, image::ImageOutputFormat::Png)?;
        Ok(Some(GeneratedTemplatePreview {
            bytes: output.into_inner(),
            width: 1,
            height: 1,
        }))
    }
}

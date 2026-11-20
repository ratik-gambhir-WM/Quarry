use super::*;

pub(crate) fn assemble_api(
    config: &AppConfig,
    postgres: PostgresClient,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Router {
    assemble_api_routes(config, postgres, http, prompts, None)
}

/// Router-only assembly for tests: background workers are started and detached.
pub(crate) fn assemble_api_routes(
    config: &AppConfig,
    postgres: PostgresClient,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
    embedded: Option<Arc<templates::slide_templates::SlideTemplates>>,
) -> Router {
    assemble_api_components(config, postgres, http, prompts, embedded).0
}

use super::*;

pub(crate) fn assemble_api(
    config: &AppConfig,
    postgres: PostgresClient,
    helix: Arc<HelixClient>,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Router {
    assemble_api_routes(config, postgres, helix, http, prompts, None)
}

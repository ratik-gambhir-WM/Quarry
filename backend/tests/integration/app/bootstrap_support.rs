use super::*;

pub(crate) fn assemble_api(
    config: &AppConfig,
    postgres: PostgresClient,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Router {
    assemble_api_routes(config, postgres, http, prompts, None)
}

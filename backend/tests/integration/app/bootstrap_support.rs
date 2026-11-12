use super::*;

pub(crate) fn assemble_api(
    config: &AppConfig,
    postgres: PostgresClient,
    document_graph: Arc<HelixClient>,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Router {
    assemble_api_routes(config, postgres, document_graph, http, prompts, None)
}

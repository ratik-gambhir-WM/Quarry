use super::*;

pub(crate) fn assemble_api(
    config: &AppConfig,
    postgres: PostgresClient,
    document_graph: Arc<HelixClient>,
    http: reqwest::Client,
    prompts: Arc<PromptCatalog>,
) -> Router {
    let legacy = config
        .diligence_studio
        .as_ref()
        .map(|c| Arc::new(DiligenceStudioClient::new(http.clone(), c.base_url.clone())));
    assemble_api_with_templates(
        config,
        postgres,
        document_graph,
        http,
        prompts,
        Arc::new(TemplateService::new(legacy)),
        None,
    )
}

use super::*;

#[tokio::test]
async fn rejects_a_missing_catalog_before_opening_local_infrastructure() {
    let mut config = AppConfig::default();
    config.prompts.directory = std::path::PathBuf::from("missing-prompt-catalog");
    config.sqlite.path = std::path::PathBuf::from("/dev/null/quarry.sqlite3");

    let error = match bootstrap(config).await {
        Ok(_) => panic!("a missing catalog must prevent bootstrap"),
        Err(error) => error,
    };

    assert!(matches!(error, BootstrapError::PromptCatalog(_)));
}

use super::*;

#[tokio::test]
async fn rejects_a_missing_catalog_before_opening_local_infrastructure() {
    let mut config = AppConfig::default();
    config.prompts.directory = std::path::PathBuf::from("missing-prompt-catalog");
    config.postgres.url = crate::app::config::SecretString::new("postgresql://invalid/Quarry");

    let error = match bootstrap(config).await {
        Ok(_) => panic!("a missing catalog must prevent bootstrap"),
        Err(error) => error,
    };

    assert!(matches!(error, BootstrapError::PromptCatalog(_)));
}

/// Explicit full-stack validation harness: no .env reads and no existing stores.
#[tokio::test]
#[ignore = "serves disposable production-composed API on 127.0.0.1:3001; requires isolated Helix on 16969"]
async fn serve_disposable_slide_template_validation_api() {
    let mut values: Vec<(String, String)> = vec![
        ("HELIX_VECTOR_DIMENSION".into(), "2".into()),
        ("OPENAI_SLIDE_EMBEDDING_DIMENSIONS".into(), "2".into()),
        ("HELIX_URL".into(), "http://127.0.0.1:16969".into()),
        (
            "QUARRY_DATABASE_URL".into(),
            std::env::var("QUARRY_TEST_DATABASE_URL").expect("disposable PostgreSQL database URL"),
        ),
    ];
    if let Ok(home) = std::env::var("HOME") {
        values.push(("HOME".into(), home));
    }
    // Test-only overrides let browser validation avoid occupied ports and reuse its disposable DB.
    for (test_key, config_key) in [
        ("QUARRY_VALIDATION_API_PORT", "QUARRY_API_PORT"),
        ("QUARRY_VALIDATION_DATABASE_URL", "QUARRY_DATABASE_URL"),
        ("QUARRY_VALIDATION_CORS_ORIGINS", "QUARRY_CORS_ORIGINS"),
        (
            "QUARRY_VALIDATION_RENDER_URL",
            "QUARRY_TEMPLATE_PREVIEW_RENDER_URL",
        ),
    ] {
        if let Ok(value) = std::env::var(test_key) {
            values.push((config_key.into(), value));
        }
    }
    let config = crate::integration_support::config(values).unwrap();
    assert!(
        url::Url::parse(config.postgres.url.expose())
            .unwrap()
            .path()
            .ends_with("_test"),
        "validation must use a separate _test database"
    );
    let app = bootstrap(config).await.unwrap();
    let listener = tokio::net::TcpListener::bind(app.bind_address)
        .await
        .unwrap();
    axum::serve(listener, app.router)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            app.templates.shutdown();
            app.templates.stop_worker().await;
        })
        .await
        .unwrap();
}

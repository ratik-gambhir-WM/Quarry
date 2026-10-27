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

#[test]
fn template_bootstrap_shares_product_client_and_preserves_builtin_deletions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("quarry.sqlite3");
    for startup in 0..3 {
        let sqlite = open_sqlite(&path).unwrap();
        migrations::migrate(&sqlite).unwrap();
        let repository = seed_templates(sqlite.clone()).unwrap();
        if startup == 0 {
            assert!(repository.delete("layered-architecture").unwrap());
        }
        assert!(repository
            .find_by_id("layered-architecture")
            .unwrap()
            .is_none());
        sqlite
            .with_connection(|connection| {
                let count: i64 =
                    connection.query_row("SELECT count(*) FROM templates", [], |r| r.get(0))?;
                assert_eq!(count, 9);
                let membership: i64 =
                    connection.query_row("SELECT count(*) FROM app_templates", [], |r| r.get(0))?;
                assert_eq!(membership, 8);
                Ok(())
            })
            .unwrap();
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

/// Explicit full-stack validation harness: no .env reads and no existing stores.
#[tokio::test]
#[ignore = "serves disposable production-composed API on 127.0.0.1:3001; requires isolated Helix on 16969"]
async fn serve_disposable_slide_template_validation_api() {
    let directory = tempfile::tempdir().unwrap();
    let mut values = vec![
        (
            "QUARRY_TEMPLATE_BACKEND".to_owned(),
            std::env::var("QUARRY_VALIDATION_TEMPLATE_BACKEND")
                .unwrap_or_else(|_| "embedded".to_owned()),
        ),
        ("HELIX_VECTOR_DIMENSION".into(), "2".into()),
        ("OPENAI_SLIDE_EMBEDDING_DIMENSIONS".into(), "2".into()),
        (
            "DILIGENCE_STUDIO_API_BASE_URL".into(),
            "http://127.0.0.1:43127/api/v1".into(),
        ),
        ("HELIX_URL".into(), "http://127.0.0.1:16969".into()),
        (
            "QUARRY_DATABASE_PATH".into(),
            directory
                .path()
                .join("product.sqlite")
                .to_string_lossy()
                .into_owned(),
        ),
    ];
    if let Ok(home) = std::env::var("HOME") {
        values.push(("HOME".into(), home));
    }
    // Test-only overrides let browser validation avoid occupied ports and reuse its disposable DB.
    for (test_key, config_key) in [
        ("QUARRY_VALIDATION_API_PORT", "QUARRY_API_PORT"),
        ("QUARRY_VALIDATION_DATABASE_PATH", "QUARRY_DATABASE_PATH"),
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
    let config = AppConfig::from_values(values).unwrap();
    let app = bootstrap(config).await.unwrap();
    let listener = tokio::net::TcpListener::bind(app.bind_address)
        .await
        .unwrap();
    axum::serve(listener, app.router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            if let Some(templates) = app.templates {
                templates.shutdown();
            }
        })
        .await
        .unwrap();
}

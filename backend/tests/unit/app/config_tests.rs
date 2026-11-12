use super::*;

#[test]
fn default_config_is_local_and_has_a_timeout() {
    let config = AppConfig::default();

    assert_eq!(config.http.bind_address, "127.0.0.1:3001".parse().unwrap());
    assert_eq!(config.http.request_timeout, Duration::from_secs(120));
    assert_eq!(config.http.cors_origins.len(), 2);
    assert_eq!(
        config.postgres.url.expose(),
        "postgresql://localhost/Quarry"
    );
    assert_eq!(config.helix.vector_dimension.get(), 1536);
    assert!(config.openai.is_none());
    assert!(config.wm_ai.is_none());
    assert_eq!(config.prompts.directory, PathBuf::from("prompts"));
}

#[test]
fn prompt_catalog_directory_defaults_and_rejects_a_blank_override() {
    let configured =
        crate::integration_support::config([("QUARRY_PROMPTS_DIR", "  bundled/prompts  ")])
            .unwrap();
    assert_eq!(
        configured.prompts.directory,
        PathBuf::from("bundled/prompts")
    );
    assert!(
        crate::integration_support::config([("QUARRY_PROMPTS_DIR", " ")])
            .unwrap_err()
            .contains("QUARRY_PROMPTS_DIR")
    );
}

#[test]
fn parses_all_sections_from_an_injected_source() {
    let config = crate::integration_support::config([
        ("QUARRY_API_PORT", "4010"),
        ("QUARRY_DATABASE_PATH", "/tmp/quarry.sqlite3"),
        ("HELIX_URL", "http://helix.internal:6969"),
        ("OPENAI_API_KEY", "secret"),
        ("OPENAI_EMBEDDING_MODEL", "embedding-model"),
        ("QUARRY_DATA_ROOM_DEAL_42", "/tmp/data-room"),
        ("QUARRY_DOCUMENT_CONCURRENCY", "3"),
    ])
    .unwrap();

    assert_eq!(config.http.bind_address.port(), 4010);
    assert_eq!(
        config.postgres.url.expose(),
        "postgresql://localhost/Quarry"
    );
    assert_eq!(config.helix.url, "http://helix.internal:6969");
    assert_eq!(config.helix.vector_dimension.get(), 1536);
    let openai = config.openai.unwrap();
    assert_eq!(openai.embedding_model, "embedding-model");
    assert_eq!(openai.embedding_dimensions.get(), 1536);
    assert_eq!(
        config.data_room.root_for_deal("DEAL-42"),
        Some(&PathBuf::from("/tmp/data-room"))
    );
    assert_eq!(config.documents.max_concurrent_documents, 3);
}

#[test]
fn rejects_invalid_values_and_partial_optional_capabilities() {
    assert!(crate::integration_support::config([("QUARRY_REQUEST_TIMEOUT_SECONDS", "0")]).is_err());
    assert!(
        crate::integration_support::config([("OPENAI_EMBEDDING_MODEL", "model-only")])
            .unwrap_err()
            .contains("OPENAI_API_KEY")
    );
    assert!(crate::integration_support::config([(
        "WM_INDEX_SERVICE_URL",
        "https://index.example"
    )])
    .unwrap_err()
    .contains("partially configured"));
}

#[test]
fn helix_requires_an_instance_base_url_and_a_nonzero_vector_dimension() {
    let configured = crate::integration_support::config([
        ("HELIX_VECTOR_DIMENSION", "3072"),
        ("OPENAI_EMBEDDING_MODEL", "text-embedding-3-large"),
        ("OPENAI_API_KEY", "secret"),
    ])
    .unwrap();
    assert_eq!(configured.helix.vector_dimension.get(), 3072);
    assert_eq!(
        configured.openai.unwrap().embedding_dimensions.get(),
        configured.helix.vector_dimension.get()
    );

    for value in [
        "http://127.0.0.1:6969/v2/query",
        "http://user:secret@127.0.0.1:6969",
        "http://127.0.0.1:6969?debug=true",
        "ftp://127.0.0.1:6969",
    ] {
        let error = crate::integration_support::config([("HELIX_URL", value)]).unwrap_err();
        assert!(error.contains("HELIX_URL"), "{error}");
    }
    assert!(crate::integration_support::config([("HELIX_VECTOR_DIMENSION", "0")]).is_err());
}

#[test]
fn embedding_models_reject_dimensions_above_their_supported_maximum() {
    for (model, dimensions) in [
        ("text-embedding-3-small", "3072"),
        ("text-embedding-3-large", "4096"),
    ] {
        assert!(crate::integration_support::config([
            ("OPENAI_API_KEY", "secret"),
            ("OPENAI_EMBEDDING_MODEL", model),
            ("HELIX_VECTOR_DIMENSION", dimensions),
        ])
        .unwrap_err()
        .contains("supported dimensions"));
    }
    let config = crate::integration_support::config([
        ("OPENAI_API_KEY", "secret"),
        ("OPENAI_EMBEDDING_MODEL", "text-embedding-3-large"),
        ("HELIX_VECTOR_DIMENSION", "1536"),
    ])
    .unwrap();
    assert_eq!(config.openai.unwrap().embedding_dimensions.get(), 1536);
}

#[test]
fn chat_model_defaults_and_activates_the_openai_capability() {
    let defaulted = crate::integration_support::config([("OPENAI_API_KEY", "secret")]).unwrap();
    assert_eq!(defaulted.openai.unwrap().chat_model, "gpt-5.5");

    let configured = crate::integration_support::config([
        ("OPENAI_API_KEY", "secret"),
        ("OPENAI_CHAT_MODEL", "chat-model"),
    ])
    .unwrap();
    assert_eq!(configured.openai.unwrap().chat_model, "chat-model");
    assert!(crate::integration_support::config([("OPENAI_CHAT_MODEL", "chat-model")]).is_err());
}

#[test]
fn secrets_are_redacted_from_debug_output() {
    let config = crate::integration_support::config([("OPENAI_API_KEY", "super-secret")]).unwrap();
    let debug = format!("{config:?}");

    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("super-secret"));
}

#[test]
fn slide_template_provider_configuration_is_atomic() {
    let defaults = crate::integration_support::config(Vec::<(String, String)>::new()).unwrap();
    assert!(defaults.slide_templates.classification.is_none());
    let embedded =
        crate::integration_support::config([("QUARRY_SLIDE_TEMPLATES_PREVIEW", "disabled")])
            .unwrap();
    assert!(embedded.slide_templates.chromium_path.is_none());
    for values in [
        vec![("QUARRY_SLIDE_CLASSIFICATION_PROVIDER", "openai")],
        vec![("QUARRY_SLIDE_TEMPLATES_PREVIEW", "other")],
        vec![("TEMPLATE_PREVIEW_RENDER_SIZE", "1")],
        vec![("QUARRY_SLIDE_TEMPLATES_ALLOW_EMBEDDING_INSPECTION", "yes")],
    ] {
        assert!(crate::integration_support::config(values).is_err());
    }
    let configured = crate::integration_support::config([
        ("QUARRY_SLIDE_CLASSIFICATION_PROVIDER", "openai"),
        ("OPENAI_API_KEY", "synthetic-test-key"),
        ("OPENAI_SLIDE_CLASSIFICATION_MODEL", "fixture-classifier"),
        ("OPENAI_SLIDE_EMBEDDING_MODEL", "fixture-embedder"),
        ("OPENAI_SLIDE_EMBEDDING_DIMENSIONS", "2"),
    ])
    .unwrap();
    assert_eq!(
        configured
            .slide_templates
            .classification
            .unwrap()
            .dimensions
            .get(),
        2
    );
}

#[test]
fn slide_vector_dimensions_remain_independent_when_classification_is_disabled() {
    let configured = crate::integration_support::config([
        ("HELIX_VECTOR_DIMENSION", "3"),
        ("OPENAI_SLIDE_EMBEDDING_DIMENSIONS", "2"),
        ("QUARRY_SLIDE_CLASSIFICATION_PROVIDER", "disabled"),
    ])
    .unwrap();
    assert_eq!(configured.helix.vector_dimension.get(), 3);
    assert_eq!(configured.slide_templates.embedding_dimensions.get(), 2);
    assert!(configured.slide_templates.classification.is_none());
    assert!(
        crate::integration_support::config([("OPENAI_SLIDE_EMBEDDING_DIMENSIONS", "0")]).is_err()
    );
}

#[cfg(unix)]
#[test]
fn configured_chromium_path_must_be_executable() {
    let dir = tempfile::tempdir().unwrap();
    let executable = dir.path().join("chromium-fixture");
    std::fs::write(&executable, b"fixture").unwrap();
    assert!(crate::integration_support::config([(
        "QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH",
        executable.to_str().unwrap()
    )])
    .is_err());
}

#[test]
fn hybrid_search_rollout_is_opt_in_with_bounded_limits() {
    assert!(!AppConfig::default().search.enabled);
    let config = crate::integration_support::config([
        ("QUARRY_HYBRID_SEARCH_ENABLED", "true"),
        ("QUARRY_SEARCH_HELIX_READS", "4"),
    ])
    .unwrap();
    assert!(config.search.enabled);
    assert_eq!(config.search.helix_reads, 4);
    for (key, value) in [
        ("QUARRY_HYBRID_SEARCH_ENABLED", "yes"),
        ("QUARRY_SEARCH_ACTIVE_REQUESTS", "0"),
        ("QUARRY_SEARCH_QUEUED_REQUESTS", "257"),
        ("QUARRY_SEARCH_TIMEOUT_SECONDS", "111"),
        ("QUARRY_SEARCH_EMBEDDING_CONCURRENCY", "17"),
    ] {
        assert!(crate::integration_support::config([(key, value)]).is_err());
    }
}

#[test]
fn hybrid_semantic_relevance_threshold_is_finite_and_bounded() {
    assert_eq!(AppConfig::default().search.min_semantic_similarity, 0.4);
    let config =
        crate::integration_support::config([("QUARRY_SEARCH_MIN_SEMANTIC_SIMILARITY", "0.55")])
            .unwrap();
    assert_eq!(config.search.min_semantic_similarity, 0.55);
    for value in ["NaN", "inf", "-0.1", "1.01", "invalid"] {
        assert!(crate::integration_support::config([(
            "QUARRY_SEARCH_MIN_SEMANTIC_SIMILARITY",
            value
        )])
        .is_err());
    }
}
#[test]
fn postgres_configuration_is_required_bounded_and_redacted() {
    assert!(AppConfig::from_values(Vec::<(String, String)>::new())
        .unwrap_err()
        .contains("QUARRY_DATABASE_URL"));
    for value in ["", "file:///Quarry", "postgresql://localhost/", "not a url"] {
        assert!(AppConfig::from_values([("QUARRY_DATABASE_URL", value)]).is_err());
    }
    for (key, value) in [
        ("QUARRY_DATABASE_MAX_CONNECTIONS", "1"),
        ("QUARRY_DATABASE_MAX_CONNECTIONS", "129"),
        ("QUARRY_DATABASE_ACQUIRE_TIMEOUT_SECONDS", "0"),
        ("QUARRY_DATABASE_STATEMENT_TIMEOUT_SECONDS", "301"),
    ] {
        assert!(crate::integration_support::config([(key, value)])
            .unwrap_err()
            .contains(key));
    }
    let config = AppConfig::from_values([(
        "QUARRY_DATABASE_URL",
        "postgresql://fixture:private-password@localhost/Quarry",
    )])
    .unwrap();
    assert!(!format!("{config:?}").contains("private-password"));
    assert_eq!(config.postgres.max_connections, 16);
}

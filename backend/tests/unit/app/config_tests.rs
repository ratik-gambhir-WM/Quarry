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
    assert_eq!(config.embeddings.vector_dimension.get(), 1536);
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
    assert_eq!(config.embeddings.vector_dimension.get(), 1536);
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
fn shared_embedding_dimension_is_postgresql_named_and_bounded() {
    let configured = crate::integration_support::config([
        ("QUARRY_EMBEDDING_DIMENSION", "3072"),
        ("OPENAI_EMBEDDING_MODEL", "text-embedding-3-large"),
        ("OPENAI_API_KEY", "secret"),
    ])
    .unwrap();
    assert_eq!(configured.embeddings.vector_dimension.get(), 3072);
    assert_eq!(
        configured.openai.unwrap().embedding_dimensions.get(),
        configured.embeddings.vector_dimension.get()
    );
    for value in ["0", "16001"] {
        let error = crate::integration_support::config([("QUARRY_EMBEDDING_DIMENSION", value)])
            .unwrap_err();
        assert!(error.contains("QUARRY_EMBEDDING_DIMENSION"), "{error}");
    }
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
            ("QUARRY_EMBEDDING_DIMENSION", dimensions),
        ])
        .unwrap_err()
        .contains("supported dimensions"));
    }
    let config = crate::integration_support::config([
        ("OPENAI_API_KEY", "secret"),
        ("OPENAI_EMBEDDING_MODEL", "text-embedding-3-large"),
        ("QUARRY_EMBEDDING_DIMENSION", "1536"),
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
        ("QUARRY_EMBEDDING_DIMENSION", "3"),
        ("OPENAI_SLIDE_EMBEDDING_DIMENSIONS", "2"),
        ("QUARRY_SLIDE_CLASSIFICATION_PROVIDER", "disabled"),
    ])
    .unwrap();
    assert_eq!(configured.embeddings.vector_dimension.get(), 3);
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
        ("QUARRY_SEARCH_SQL_READS", "4"),
    ])
    .unwrap();
    assert!(config.search.enabled);
    assert_eq!(config.search.sql_reads, 4);
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

#[test]
fn background_document_workers_default_to_five_and_reject_out_of_range_values() {
    use crate::integration_support::config;
    let defaults = config(Vec::<(&str, &str)>::new()).unwrap();
    assert_eq!(defaults.documents.max_concurrent_background_documents, 5);
    assert_eq!(defaults.documents.max_concurrent_documents, 8);
    for valid in ["1", "5", "8"] {
        let parsed = config([("QUARRY_DOCUMENT_WORKER_CONCURRENCY", valid)]).unwrap();
        assert_eq!(
            parsed.documents.max_concurrent_background_documents,
            valid.parse::<usize>().unwrap()
        );
    }
    for invalid in ["0", "9", "many"] {
        assert!(config([("QUARRY_DOCUMENT_WORKER_CONCURRENCY", invalid)]).is_err());
    }
}

#[test]
fn document_classification_is_on_by_default_and_an_explicit_enable_requires_a_key() {
    use crate::integration_support::config;
    let defaults = config(Vec::<(&str, &str)>::new()).unwrap();
    assert!(defaults.documents.classification.enabled);
    assert!(!defaults.documents.classification.explicitly_enabled);
    assert_eq!(defaults.documents.classification.concurrency, 5);
    // The default never blocks startup without OpenAI; an explicit `true` does.
    let error = config([("QUARRY_DOCUMENT_CLASSIFICATION_ENABLED", "true")]).unwrap_err();
    assert!(error.contains("OPENAI_API_KEY"));
    let disabled = config([("QUARRY_DOCUMENT_CLASSIFICATION_ENABLED", "false")]).unwrap();
    assert!(!disabled.documents.classification.enabled);
    let enabled = config([
        ("QUARRY_DOCUMENT_CLASSIFICATION_ENABLED", "true"),
        ("QUARRY_DOCUMENT_CLASSIFICATION_CONCURRENCY", "8"),
        ("OPENAI_API_KEY", "sk-test"),
    ])
    .unwrap();
    assert!(enabled.documents.classification.enabled);
    assert_eq!(enabled.documents.classification.concurrency, 8);
    for (name, value) in [
        ("QUARRY_DOCUMENT_CLASSIFICATION_ENABLED", "yes"),
        ("QUARRY_DOCUMENT_CLASSIFICATION_CONCURRENCY", "0"),
        ("QUARRY_DOCUMENT_CLASSIFICATION_CONCURRENCY", "9"),
    ] {
        assert!(config([(name, value)]).is_err(), "{name}={value}");
    }
}

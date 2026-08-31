use super::*;

#[test]
fn default_config_is_local_and_has_a_timeout() {
    let config = AppConfig::default();

    assert_eq!(config.http.bind_address, "127.0.0.1:3001".parse().unwrap());
    assert_eq!(config.http.request_timeout, Duration::from_secs(120));
    assert_eq!(config.http.cors_origins.len(), 2);
    assert_eq!(config.sqlite.path, PathBuf::from("data/quarry.sqlite3"));
    assert!(config.openai.is_none());
    assert!(config.wm_ai.is_none());
}

#[test]
fn parses_all_sections_from_an_injected_source() {
    let config = AppConfig::from_values([
        ("PATHFINDER_API_PORT", "4010"),
        ("PATHFINDER_DATABASE_PATH", "/tmp/quarry.sqlite3"),
        ("HELIX_URL", "http://helix.internal:6969"),
        ("OPENAI_API_KEY", "secret"),
        ("OPENAI_EMBEDDING_MODEL", "embedding-model"),
        ("QUARRY_DATA_ROOM_DEAL_42", "/tmp/data-room"),
        ("QUARRY_DOCUMENT_CONCURRENCY", "3"),
    ])
    .unwrap();

    assert_eq!(config.http.bind_address.port(), 4010);
    assert_eq!(config.sqlite.path, PathBuf::from("/tmp/quarry.sqlite3"));
    assert_eq!(config.helix.url, "http://helix.internal:6969");
    assert_eq!(config.openai.unwrap().embedding_model, "embedding-model");
    assert_eq!(
        config.data_room.root_for_deal("DEAL-42"),
        Some(&PathBuf::from("/tmp/data-room"))
    );
    assert_eq!(config.documents.max_concurrent_documents, 3);
}

#[test]
fn rejects_invalid_values_and_partial_optional_capabilities() {
    assert!(AppConfig::from_values([("PATHFINDER_REQUEST_TIMEOUT_SECONDS", "0")]).is_err());
    assert!(
        AppConfig::from_values([("OPENAI_EMBEDDING_MODEL", "model-only")])
            .unwrap_err()
            .contains("OPENAI_API_KEY")
    );
    assert!(
        AppConfig::from_values([("WM_INDEX_SERVICE_URL", "https://index.example")])
            .unwrap_err()
            .contains("partially configured")
    );
}

#[test]
fn secrets_are_redacted_from_debug_output() {
    let config = AppConfig::from_values([("OPENAI_API_KEY", "super-secret")]).unwrap();
    let debug = format!("{config:?}");

    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("super-secret"));
}

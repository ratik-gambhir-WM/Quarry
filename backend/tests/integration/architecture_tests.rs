use std::{fs, path::Path};

use walkdir::WalkDir;

fn rust_sources(relative_directory: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_directory);
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        })
        .map(|entry| {
            let path = entry.path().display().to_string();
            let source = fs::read_to_string(entry.path()).unwrap();
            (path, source)
        })
        .collect()
}

#[test]
fn tests_are_kept_out_of_the_source_tree() {
    let test_cfg = regex::Regex::new(r"#\[cfg\([^\n]*\btest\b[^\n]*\)\]\s*").unwrap();
    let path_module =
        regex::Regex::new(r#"^#\[path = "([^"]+)"\]\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+;"#)
            .unwrap();
    let tests_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .canonicalize()
        .unwrap();
    for (path, source) in rust_sources("src") {
        assert!(
            !path.ends_with("_tests.rs") && !path.ends_with("/tests.rs"),
            "test module is stored under the production source tree: {path}"
        );
        assert!(
            !source.contains("#[test]") && !source.contains("#[tokio::test]"),
            "test body is stored under the production source tree: {path}"
        );
        for attribute in test_cfg.find_iter(&source) {
            let captures = path_module
                .captures(&source[attribute.end()..])
                .unwrap_or_else(|| panic!("{path}: test-only code must live under backend/tests; only path module hooks belong in source"));
            let target = Path::new(&path)
                .parent()
                .unwrap()
                .join(&captures[1])
                .canonicalize()
                .expect("test module path must exist");
            assert!(
                target.starts_with(&tests_root),
                "{path}: test-only module must point into backend/tests"
            );
        }
    }
}

#[test]
fn backend_tests_are_split_into_unit_and_integration_trees() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests_root = manifest.join("tests");

    for entry in WalkDir::new(&tests_root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
        })
    {
        let relative = entry.path().strip_prefix(&tests_root).unwrap();
        let category = relative.components().next().unwrap().as_os_str();
        assert!(
            category == "unit" || category == "integration",
            "backend test is outside tests/unit or tests/integration: {}",
            entry.path().display()
        );

        if category != "unit" {
            continue;
        }

        let unit_relative = relative.strip_prefix("unit").unwrap();
        let test_file_name = unit_relative.file_name().unwrap().to_string_lossy();
        let source_file_name = if test_file_name == "tests.rs" {
            "mod.rs".to_string()
        } else {
            format!(
                "{}.rs",
                test_file_name
                    .strip_suffix("_tests.rs")
                    .expect("unit test filenames must end in _tests.rs")
            )
        };
        let source = manifest
            .join("src")
            .join(unit_relative.parent().unwrap())
            .join(source_file_name);
        assert!(
            source.is_file(),
            "unit test does not mirror a source module: {}",
            entry.path().display()
        );
    }
}

#[test]
fn global_application_state_and_legacy_horizontal_roots_are_gone() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    for directory in [
        "handlers",
        "services",
        "repository",
        "routes",
        "core",
        "events",
    ] {
        assert!(
            !manifest.join("src").join(directory).exists(),
            "legacy src/{directory} root still exists"
        );
    }
    for file in [
        "bootstrap.rs",
        "config.rs",
        "document_jobs.rs",
        "errors.rs",
        "state.rs",
        "utils.rs",
    ] {
        assert!(
            !manifest.join("src").join(file).exists(),
            "legacy src/{file} still exists"
        );
    }
    for (path, source) in rust_sources("src") {
        assert!(!source.contains("AppState"), "{path} references AppState");
    }
}

#[test]
fn domains_and_adapters_preserve_dependency_direction() {
    for (path, source) in rust_sources("src/adapters") {
        assert!(
            !source.contains("crate::domains"),
            "{path} imports a product domain"
        );
    }

    for (path, source) in rust_sources("src/domains") {
        let is_request_layer = is_handler_source(&path) || path.ends_with("route.rs");
        if is_request_layer {
            assert!(
                !source.contains("::repository") && !source.contains("repository::"),
                "{path} imports a repository"
            );
            assert!(
                !source.contains("crate::adapters"),
                "{path} imports a concrete adapter"
            );
        }

        assert!(
            !source.contains("env::var")
                && !source.contains("std::env")
                && !source.contains("from_env("),
            "{path} reads ambient configuration"
        );
    }
}

#[test]
fn product_infrastructure_is_constructed_only_in_bootstrap() {
    let constructors = [
        "PostgresClient::connect(",
        "OpenAiClient::from_config(",
        "DecisionsClient::new(",
        "ClassificationRepository::new(",
        "OfficeConverter::new(",
        "FileUploadServiceClient::new(",
        "IndexServiceClient::new(",
        "GraphRagClient::new(",
        "PostgresTemplateRepository::new(",
        "HeadlessTemplatePreviewGenerator::new(",
        "OpenAISlideClassifier::new(",
        "OpenAISlideEmbedder::new(",
        "UserRepository::new(",
        "DealRepository::new(",
        "CompanyRepository::new(",
        "NotebookRepository::new(",
        "NotebookPipeline::new(",
        "DocumentStore::new(",
        "IdentityRepository::new(",
        "ResourceAccess::new(",
        "MembershipRepository::new(",
        "DealAccessService::new(",
    ];

    for (path, source) in rust_sources("src") {
        if path.ends_with("/app/bootstrap.rs")
            || path.contains("/src/bin/")
            || path.ends_with("_tests.rs")
            || path.ends_with("/tests.rs")
        {
            continue;
        }
        for constructor in constructors {
            assert!(
                !source.contains(constructor),
                "{path} constructs product infrastructure with {constructor}"
            );
        }
    }
}

#[test]
fn domain_routers_bind_feature_state_before_composition() {
    for (path, source) in rust_sources("src/domains")
        .into_iter()
        .filter(|(path, _)| path.ends_with("route.rs"))
    {
        if path.contains("/dev_support/") {
            continue;
        }
        assert!(
            source.contains(".with_state("),
            "{path} does not bind its feature state"
        );
    }
}

#[test]
fn cross_domain_imports_do_not_reach_private_layers() {
    let domain_names = [
        "assistant",
        "data_rooms",
        "deals",
        "companies",
        "dev_support",
        "documents",
        "research",
        "summaries",
        "system",
        "users",
        "templates",
        "identity",
        "notebooks",
        "memberships",
    ];
    for (path, source) in rust_sources("src/domains") {
        let owner = domain_names
            .iter()
            .find(|name| path.contains(&format!("/domains/{name}/")))
            .copied();
        for other in domain_names
            .iter()
            .copied()
            .filter(|name| Some(*name) != owner)
        {
            for private_layer in ["handler", "route", "repository"] {
                for forbidden in [
                    format!("domains::{other}::{private_layer}"),
                    format!("{other}::{private_layer}"),
                ] {
                    assert!(
                        !source.contains(&forbidden),
                        "{path} imports private cross-domain layer {forbidden}"
                    );
                }
            }
        }
    }
}

#[test]
fn document_attachment_check_is_one_explicit_cross_context_query() {
    let matches = rust_sources("src/domains/documents")
        .into_iter()
        .filter(|(path, _)| !path.ends_with("_tests.rs") && !path.ends_with("/tests.rs"))
        .flat_map(|(path, source)| {
            let count = source.matches("SqlBuilder::select(\"deals\")").count();
            std::iter::repeat_n(path, count)
        })
        .collect::<Vec<_>>();

    assert!(
        matches.is_empty(),
        "document authorization must use the canonical membership transaction policy"
    );
    let store = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domains/documents/store/repository.rs"),
    )
    .unwrap();
    assert!(store.contains("transaction_result_async") && store.contains("require_deal"));

    let behavior_tests = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/integration/domains/documents/store/repository_tests.rs"),
    )
    .unwrap();
    assert!(
        behavior_tests
            .contains("deal_validation_rejects_missing_archived_and_differently_owned_deals"),
        "cross-context attachment check lacks its focused behavior test"
    );
}

#[test]
fn template_domain_keeps_sql_and_http_ownership_explicit() {
    for (path, source) in rust_sources("src/domains/templates/slide_templates") {
        if !is_handler_source(&path) && !path.ends_with("/route.rs") {
            assert!(
                !source.contains("axum::") && !source.contains("StatusCode"),
                "reusable template mechanism depends on HTTP: {path}"
            );
        }
        assert!(
            !source.contains("Connection::open(")
                && !source.contains(".execute_batch(")
                && !source.contains(".prepare("),
            "template domain bypasses the PostgreSQL builder/client: {path}"
        );
    }
}

fn is_handler_source(path: &str) -> bool {
    if path.ends_with("/handler.rs") || path.ends_with("_handler.rs") || path.contains("/handler/")
    {
        return true;
    }
    // A flat validator remains handler-owned only when declared as its private child.
    path.ends_with("/validate.rs")
        && fs::read_to_string(Path::new(path).with_file_name("handler.rs"))
            .is_ok_and(|source| source.contains("#[path = \"validate.rs\"]\nmod validate;"))
}

#[test]
fn http_types_and_errors_stay_in_the_handler_layer() {
    for (path, source) in rust_sources("src/domains") {
        if is_handler_source(&path) {
            continue;
        }
        assert!(
            !source.contains("app::http")
                && (path.ends_with("/route.rs") || !source.contains("axum::"))
                && !source.contains("AppError")
                && !source.contains("AppResult"),
            "domain model/service/repository or route owns HTTP extraction or validation: {path}"
        );
    }
}

#[test]
fn product_services_do_not_reach_handler_owned_validation() {
    for (path, source) in rust_sources("src/domains")
        .into_iter()
        .filter(|(path, _)| path.ends_with("service.rs"))
    {
        assert!(
            !source.contains("super::handler")
                && !source.contains("handler::")
                && !source.contains("::upload::"),
            "product service depends on the HTTP handler layer: {path}"
        );
    }
}

#[test]
fn openai_provider_calls_use_rig_without_a_product_http_client() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for file in [
        "src/adapters/openai/client.rs",
        "src/adapters/slide_templates/providers.rs",
        "src/domains/templates/slide_templates/providers.rs",
        "src/domains/templates/slide_templates/processing/service.rs",
        "src/domains/templates/slide_templates/processing/worker.rs",
    ] {
        let source = fs::read_to_string(root.join(file)).unwrap();
        if file.starts_with("src/domains/") {
            assert!(
                !source.contains("use rig::"),
                "{file} must use the provider adapter rather than Rig wire types"
            );
        }
        for forbidden in [
            "reqwest::Client",
            ".bearer_auth(",
            "post_json(",
            "api.openai.com",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} bypasses Rig with {forbidden}"
            );
        }
    }
    // Documented exception: Rig has no Decisions endpoint. Exactly one adapter owns that
    // transport, on one fixed endpoint, and no other source reaches OpenAI directly.
    let decisions = fs::read_to_string(root.join("src/adapters/openai/decisions.rs")).unwrap();
    assert_eq!(decisions.matches("api.openai.com").count(), 1);
    assert!(decisions.contains("\"https://api.openai.com/v1/decisions\""));
    for (path, source) in rust_sources("src") {
        if path.ends_with("/adapters/openai/decisions.rs")
            || path.contains("/slide_templates/")
            || path.ends_with("_tests.rs")
        {
            continue;
        }
        assert!(
            !source.contains("api.openai.com/v1/decisions"),
            "{path} calls Decisions outside its adapter"
        );
    }
}

#[test]
fn document_data_access_is_repository_owned_and_has_no_graph_dependency() {
    for (path, source) in rust_sources("src/domains/documents") {
        for forbidden in ["DocumentGraphReader", "DocumentGraphWriter"] {
            assert!(
                !source.contains(forbidden),
                "{path}: document graph dependency {forbidden}"
            );
        }
    }
    for (path, source) in rust_sources("src/adapters/postgres") {
        for table in [
            "document_chunks",
            "document_jobs",
            "document_identity_manifests",
            "quarry_file_versions",
        ] {
            assert!(
                !source.contains(table),
                "{path}: feature SQL belongs in a document repository"
            );
        }
    }
}

#[test]
fn notebook_storage_and_workers_have_no_remote_cleanup_path() {
    for (path, source) in rust_sources("src/domains/notebooks") {
        assert!(
            !source.contains("notebook_cleanup"),
            "{path}: retired notebook cleanup dependency"
        );
    }
}

#[test]
fn template_storage_has_no_graph_projection_or_legacy_vector_encoding() {
    for (path, source) in rust_sources("src") {
        let lowered = source.to_lowercase();
        for forbidden in ["templategraph", "template_projection"] {
            assert!(
                !lowered.contains(forbidden),
                "{path}: retired graph dependency {forbidden}"
            );
        }
    }
    for (path, source) in rust_sources("src/domains/templates") {
        for forbidden in [
            "subject_embedding BYTEA",
            "serialize_embedding",
            "deserialize_embedding",
            "cosine_similarity",
            "Mutex<()>",
        ] {
            assert!(
                !source.contains(forbidden),
                "{path}: template vectors/serialization must stay in PostgreSQL ({forbidden})"
            );
        }
    }
}

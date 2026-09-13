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
    for (path, source) in rust_sources("src") {
        assert!(
            !path.ends_with("_tests.rs") && !path.ends_with("/tests.rs"),
            "test module is stored under the production source tree: {path}"
        );
        assert!(
            !source.contains("#[test]") && !source.contains("#[tokio::test]"),
            "test body is stored under the production source tree: {path}"
        );
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
        let is_request_layer = path.ends_with("handler.rs") || path.ends_with("route.rs");
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
        "SqliteClient::open(",
        "HelixClient::from_config(",
        "OpenAiClient::from_config(",
        "OfficeConverter::new(",
        "FileUploadServiceClient::new(",
        "IndexServiceClient::new(",
        "GraphRagClient::new(",
        "DiligenceStudioClient::new(",
        "UserRepository::new(",
        "DealRepository::new(",
        "DocumentStore::new(",
        "DocumentIndexReader::new(",
        "DocumentIndexWriter::new(",
        "DocumentSearchIndex::new(",
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
        "data_rooms",
        "deals",
        "dev_support",
        "documents",
        "research",
        "summaries",
        "system",
        "users",
        "templates",
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

    assert_eq!(matches.len(), 1, "unexpected document-to-deal query count");
    assert!(
        matches[0].ends_with("/domains/documents/store/sqlite.rs"),
        "cross-context attachment check moved to {}",
        matches[0]
    );

    let behavior_tests = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/unit/domains/documents/store/sqlite_tests.rs"),
    )
    .unwrap();
    assert!(
        behavior_tests
            .contains("deal_validation_rejects_missing_archived_and_differently_owned_deals"),
        "cross-context attachment check lacks its focused behavior test"
    );
}

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
fn lower_layers_do_not_depend_on_axum_state() {
    for (path, source) in rust_sources("src/services")
        .into_iter()
        .chain(rust_sources("src/repository"))
    {
        assert!(!source.contains("AppState"), "{path} imports AppState");
    }
}

#[test]
fn request_layers_do_not_read_configuration_or_construct_infrastructure() {
    for directory in ["src/handlers", "src/services", "src/repository", "src/core"] {
        for (path, source) in rust_sources(directory) {
            assert!(
                !source.contains("env::var")
                    && !source.contains("std::env")
                    && !source.contains("from_env("),
                "{path} reads ambient configuration"
            );
        }
    }

    for (path, source) in rust_sources("src/handlers") {
        assert!(
            !source.contains("repository::"),
            "{path} imports a repository"
        );
        assert!(
            !source.contains("Client::new")
                && !source.contains("Client::from_config")
                && !source.contains("Client::with_config"),
            "{path} constructs an infrastructure client"
        );
    }
}

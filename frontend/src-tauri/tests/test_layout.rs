//! Guards the desktop crate's source/test separation.

use std::{fs, path::Path};

#[test]
fn tests_are_kept_out_of_the_source_tree() {
    visit_rust_sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
}

fn visit_rust_sources(directory: &Path) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            visit_rust_sources(&path);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("#[test]") && !source.contains("#[tokio::test]"),
            "test body is stored under the production source tree: {}",
            path.display()
        );
    }
}

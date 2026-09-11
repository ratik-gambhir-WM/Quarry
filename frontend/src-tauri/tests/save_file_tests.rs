//! Tests for native file export.

use super::*;

fn valid_input() -> SaveFileInput {
    SaveFileInput {
        contents: "# Summary".to_string(),
        extensions: vec!["md".to_string(), "markdown".to_string()],
        mime_type: "text/markdown;charset=utf-8".to_string(),
        suggested_name: "summary.md".to_string(),
        title: "Save summary".to_string(),
    }
}

#[test]
fn rejects_path_components_in_suggested_filename() {
    let mut input = valid_input();
    input.suggested_name = "../summary.md".to_string();
    assert_eq!(
        validate_input(&input).unwrap_err().code,
        crate::errors::ErrorCode::Validation
    );
}

#[test]
fn rejects_mismatched_mime_type_and_extensions() {
    let mut input = valid_input();
    input.extensions = vec!["json".to_string()];
    assert_eq!(
        validate_input(&input).unwrap_err().code,
        crate::errors::ErrorCode::Validation
    );
}

#[test]
fn writes_contents_via_a_sibling_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("summary.md");
    atomic_write(&path, b"# Saved").unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), "# Saved");
}

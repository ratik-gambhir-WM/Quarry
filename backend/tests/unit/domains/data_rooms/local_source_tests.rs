//! Tests for local data-room source validation.

use super::*;

#[test]
fn lists_actual_file_sizes_including_empty_files_without_following_symlinks() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("notes.docx"), b"notes").unwrap();
    fs::write(root.path().join("empty.pdf"), b"").unwrap();
    fs::create_dir(root.path().join("folder")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        root.path().join("notes.docx"),
        root.path().join("link.docx"),
    )
    .unwrap();

    let tree = build_directory_node(root.path(), root.path(), Path::new(""), true);
    let children = tree.children.unwrap();
    let find = |name: &str| children.iter().find(|node| node.name == name).unwrap();
    assert_eq!(find("notes.docx").size_bytes, Some(5));
    assert_eq!(find("empty.pdf").size_bytes, Some(0));
    assert_eq!(find("folder").size_bytes, None);
    assert!(!children.iter().any(|node| node.name == "link.docx"));
    assert_eq!(
        serde_json::to_value(find("notes.docx")).unwrap()["sizeBytes"],
        5
    );
}

#[test]
fn validates_pdf_bytes_in_one_shared_boundary() {
    assert!(validate_pdf_bytes(b"%PDF-1.4\n", "the PDF").is_ok());
    assert_eq!(
        validate_pdf_bytes(b"not a PDF", "the PDF").unwrap_err(),
        "the PDF does not contain a valid PDF header"
    );
    assert_eq!(
        validate_pdf_bytes(&[], "the PDF").unwrap_err(),
        "the PDF is empty"
    );
}

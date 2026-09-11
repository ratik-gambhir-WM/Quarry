//! Tests for native data-room file access.

use super::*;

#[test]
fn scans_supported_source_files_in_the_data_room() {
    let directory = tempfile::tempdir().unwrap();
    let admin = directory.path().join("01 Admin");
    fs::create_dir(&admin).unwrap();
    fs::write(admin.join("Project SOW.docx"), b"sow").unwrap();
    fs::write(admin.join("Project Timeline.pdf"), b"timeline").unwrap();
    fs::write(directory.path().join("Financials.xlsx"), b"numbers").unwrap();

    let result = scan_data_room(directory.path()).unwrap();

    assert_eq!(result.files.len(), 2);
    assert_eq!(result.files[0].relative_path, "01 Admin/Project SOW.docx");
}

#[test]
fn reads_only_files_inside_an_authorized_root() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("SOW.docx");
    fs::write(&source, b"questions").unwrap();
    let root = directory.path().canonicalize().unwrap();
    let registry = LocalDealRoots::default();
    registry.authorize(root.clone()).unwrap();
    registry.ensure_authorized(&root).unwrap();

    let files = read_selected_files(&root, &[source.display().to_string()]).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].data_base64, "cXVlc3Rpb25z");

    let outside = tempfile::NamedTempFile::new().unwrap();
    assert!(read_selected_files(&root, &[outside.path().display().to_string()]).is_err());
}

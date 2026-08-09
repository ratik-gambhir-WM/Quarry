use std::path::Path;

use super::*;

struct TestFile {
    path: PathBuf,
}

impl TestFile {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("quarry-parser-test-{}-{name}", std::process::id()));
        fs::write(&path, b"test fixture").unwrap();
        Self { path }
    }
}

impl Drop for TestFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn quarry_file_parts(quarry_file: &QuarryFile) -> (&'static str, &[u8], &Path) {
    match quarry_file {
        QuarryFile::Pdf { bytes, path } => ("pdf", bytes, path),
        QuarryFile::Docx { bytes, path } => ("docx", bytes, path),
    }
}

#[test]
fn generates_metadata_from_open_file() {
    let test_file = TestFile::new("metadata.pdf");
    let file = File::open(&test_file.path).unwrap();

    let metadata = generate_file_metadata(&file).unwrap();

    assert!(metadata.is_file());
    assert_eq!(metadata.len(), 12);
    assert!(file.metadata().is_ok());
}

#[test]
fn quarry_file_from_local_path_loads_bytes_and_maps_supported_extensions() {
    let cases = [("report.DOCX", "docx"), ("report.pdf", "pdf")];

    for (name, expected_kind) in cases {
        let source = TestFile::new(name);
        let quarry_file = QuarryFile::from_local_path(&source.path).unwrap();
        let (kind, bytes, stored_path) = quarry_file_parts(&quarry_file);

        assert_eq!(kind, expected_kind);
        assert_eq!(stored_path, source.path);
        assert_eq!(bytes, b"test fixture");
    }
}

#[test]
fn quarry_file_from_local_path_rejects_unsupported_or_missing_extensions() {
    for path in [
        "notes.txt",
        "slides.pptx",
        "scan.png",
        "model.xlsx",
        "README",
    ] {
        assert_eq!(
            QuarryFile::from_local_path(path).unwrap_err(),
            "invalid file format"
        );
    }
}

#[tokio::test]
async fn parse_passes_loaded_bytes_to_the_matching_parser() {
    let pdf = QuarryFile::Pdf {
        bytes: b"not a PDF".to_vec(),
        path: PathBuf::from("ignored.pdf"),
    };
    assert!(pdf
        .parse_for_user("user-1")
        .await
        .unwrap_err()
        .starts_with("failed to extract text from PDF bytes:"));

    let docx = QuarryFile::Docx {
        bytes: b"not a DOCX".to_vec(),
        path: PathBuf::from("ignored.docx"),
    };
    assert!(docx
        .parse_for_user("user-1")
        .await
        .unwrap_err()
        .contains("invalid Zip archive"));
}

#[tokio::test]
async fn parsing_requires_an_explicit_user_scope() {
    let pdf = QuarryFile::Pdf {
        bytes: b"not a PDF".to_vec(),
        path: PathBuf::from("ignored.pdf"),
    };

    assert_eq!(
        pdf.parse_for_user("  ").await.unwrap_err(),
        "user_id cannot be empty"
    );
}

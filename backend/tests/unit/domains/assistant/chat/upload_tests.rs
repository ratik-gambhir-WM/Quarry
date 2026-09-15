use super::*;
use std::io::{Cursor, Write};

fn raw(filename: &str, mime: &str, bytes: &[u8]) -> RawUpload {
    RawUpload {
        browser_mime: mime.to_string(),
        bytes: bytes.to_vec(),
        filename: filename.to_string(),
    }
}

#[test]
fn validates_signatures_and_mime_consistency() {
    let png = validate_upload(raw("image.png", "", b"\x89PNG\r\n\x1a\nrest")).unwrap();
    assert_eq!(png.kind, ChatUploadKind::Image);
    assert_eq!(png.mime_type, "image/png");
    assert!(validate_upload(raw("image.png", "image/jpeg", b"\x89PNG\r\n\x1a\nrest")).is_err());
    assert!(validate_upload(raw("fake.pdf", "application/pdf", b"not-pdf")).is_err());
}

#[test]
fn rejects_animated_gif_and_accepts_one_frame() {
    let single = b"GIF89a\x01\0\x01\0\0\0\0\x2c\0\0\0\0\x01\0\x01\0\0\x02\x01\0\0\x3b";
    assert!(validate_upload(raw("one.gif", "image/gif", single)).is_ok());
    let mut animated = single[..single.len() - 1].to_vec();
    animated.extend_from_slice(b"\x2c\0\0\0\0\x01\0\x01\0\0\x02\x01\0\0\x3b");
    assert!(validate_upload(raw("two.gif", "image/gif", &animated)).is_err());
}

#[test]
fn accepts_each_non_zip_contract_type() {
    for (filename, mime, bytes, kind) in [
        (
            "photo.jpg",
            "image/jpeg",
            b"\xff\xd8\xffdata".as_slice(),
            ChatUploadKind::Image,
        ),
        (
            "photo.webp",
            "image/webp",
            b"RIFF\0\0\0\0WEBPdata".as_slice(),
            ChatUploadKind::Image,
        ),
        (
            "file.pdf",
            "application/pdf",
            b"%PDF-1.7".as_slice(),
            ChatUploadKind::File,
        ),
        (
            "file.txt",
            "text/plain",
            b"text".as_slice(),
            ChatUploadKind::File,
        ),
        (
            "file.md",
            "text/plain",
            b"# text".as_slice(),
            ChatUploadKind::File,
        ),
        (
            "file.json",
            "application/json",
            b"not parsed by transport".as_slice(),
            ChatUploadKind::File,
        ),
        (
            "file.html",
            "text/html",
            b"<p>text</p>".as_slice(),
            ChatUploadKind::File,
        ),
        (
            "file.csv",
            "text/csv",
            b"a,b".as_slice(),
            ChatUploadKind::File,
        ),
        (
            "file.doc",
            "application/msword",
            b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1data".as_slice(),
            ChatUploadKind::File,
        ),
    ] {
        assert_eq!(
            validate_upload(raw(filename, mime, bytes)).unwrap().kind,
            kind
        );
    }
}

#[test]
fn validates_office_zip_family_markers() {
    for (filename, entry, marker) in [
        (
            "file.docx",
            "word/document.xml",
            "wordprocessingml.document",
        ),
        (
            "file.pptx",
            "ppt/presentation.xml",
            "presentationml.presentation",
        ),
        ("file.xlsx", "xl/workbook.xml", "spreadsheetml.sheet"),
    ] {
        let bytes = office_zip(entry, marker);
        assert!(validate_upload(raw(filename, "application/octet-stream", &bytes)).is_ok());
    }
    let wrong = office_zip("ppt/presentation.xml", "presentationml.presentation");
    assert!(validate_upload(raw("file.docx", "application/octet-stream", &wrong)).is_err());
}

fn office_zip(entry: &str, marker: &str) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("[Content_Types].xml", options).unwrap();
        write!(writer, "<Types>{marker}</Types>").unwrap();
        writer.start_file(entry, options).unwrap();
        writer.write_all(b"<xml/>").unwrap();
        writer.finish().unwrap();
    }
    bytes.into_inner()
}

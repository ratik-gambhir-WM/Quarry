use super::*;
use pptx_to_md::{
    ElementPosition, ListElement, ListItem, Run, TableCell, TableElement, TableRow, TextElement,
};
use std::io::{Cursor, Write};

fn run(text: &str) -> Run {
    Run {
        text: text.to_string(),
        formatting: Default::default(),
    }
}

#[test]
fn extracts_text_from_slide_elements_in_visual_order() {
    let elements = vec![
        SlideElement::Text(
            TextElement {
                runs: vec![run("Subtitle")],
            },
            ElementPosition { x: 200, y: 200 },
        ),
        SlideElement::List(
            ListElement {
                items: vec![
                    ListItem {
                        level: 0,
                        is_ordered: false,
                        runs: vec![run("First bullet")],
                    },
                    ListItem {
                        level: 0,
                        is_ordered: false,
                        runs: vec![run("Second bullet")],
                    },
                ],
            },
            ElementPosition { x: 100, y: 300 },
        ),
        SlideElement::Table(
            TableElement {
                rows: vec![TableRow {
                    cells: vec![
                        TableCell {
                            runs: vec![run("Name")],
                        },
                        TableCell {
                            runs: vec![run("Amount")],
                        },
                    ],
                }],
            },
            ElementPosition { x: 100, y: 400 },
        ),
        SlideElement::Text(
            TextElement {
                runs: vec![run("Title")],
            },
            ElementPosition { x: 100, y: 100 },
        ),
    ];

    assert_eq!(
        slide_text(&elements),
        "Title\nSubtitle\nFirst bullet\nSecond bullet\nName\tAmount"
    );
}

#[test]
fn parses_pptx_bytes_into_a_pathless_graph_ready_assembly() {
    let bytes = pptx_bytes();
    let text = parse_powerpoint_from_bytes(&bytes).unwrap();
    let assembly = parse_powerpoint_chunks_from_bytes(&bytes, None, "user-1").unwrap();

    assert_eq!(text, "Slide 1\nFirst slide\n\n\nSlide 2\nSecond slide\n");
    assert_eq!(assembly.document.file_name, "Presentation.pptx");
    assert_eq!(assembly.document.source_type, "pptx");
    assert!(assembly.document.local_path.is_none());
    assert_eq!(assembly.document.file_size_bytes, bytes.len() as u64);
    assert_eq!(assembly.document.content_hash, sha256_bytes(&bytes));
    assert_eq!(assembly.chunks.len(), 1);
    assert_eq!(assembly.chunks[0].page_numbers, Some(vec![1, 2]));
    assert_eq!(assembly.chunks[0].start_offset, 0);
    assert_eq!(assembly.chunks[0].end_offset, text.len());
}

#[test]
fn rejects_invalid_pptx_bytes_with_a_sanitized_error() {
    let error = parse_powerpoint_from_bytes(b"not a PowerPoint").unwrap_err();
    assert_eq!(error, "invalid PowerPoint presentation");
}

fn pptx_bytes() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut bytes);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("[Content_Types].xml", options).unwrap();
        zip.write_all(b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/><Override PartName=\"/ppt/slides/slide1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/><Override PartName=\"/ppt/slides/slide2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/></Types>").unwrap();
        zip.start_file("ppt/presentation.xml", options).unwrap();
        zip.write_all(b"<p:presentation xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"/>").unwrap();
        zip.start_file("ppt/slides/slide1.xml", options).unwrap();
        zip.write_all(slide_xml("First slide", 10).as_bytes())
            .unwrap();
        zip.start_file("ppt/slides/slide2.xml", options).unwrap();
        zip.write_all(slide_xml("Second slide", 20).as_bytes())
            .unwrap();
        zip.finish().unwrap();
    }
    bytes.into_inner()
}

fn slide_xml(text: &str, y: i64) -> String {
    format!(
        "<p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><p:cSld><p:spTree><p:sp><p:spPr><a:xfrm><a:off x=\"10\" y=\"{y}\"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"
    )
}

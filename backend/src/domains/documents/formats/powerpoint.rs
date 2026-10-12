#![allow(dead_code)]

use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use pptx_to_md::{parse_xml::parse_slide_xml, SlideElement};
use roxmltree::Document as XmlDocument;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zip::ZipArchive;

use crate::{
    domains::documents::{
        formats::text_chunking::token_bounded_ranges,
        model::{Document, DocumentChunk},
    },
    shared::{
        ids::document_id_from_content,
        office_zip::{validate_office_package, OfficeDocumentKind},
    },
};

#[derive(Debug, Clone)]
pub struct PowerpointAssembly {
    pub document: Document,
    pub chunks: Vec<DocumentChunk>,
}

#[derive(Debug, Clone)]
struct SlideTextRange {
    slide_number: u32,
    start_offset: usize,
    end_offset: usize,
}

const PRESENTATION_ENTRY_NAME: &str = "ppt/presentation.xml";
const PRESENTATION_RELATIONSHIPS_ENTRY_NAME: &str = "ppt/_rels/presentation.xml.rels";
const PRESENTATIONML_NAMESPACE: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const OFFICE_DOCUMENT_RELATIONSHIPS_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PACKAGE_RELATIONSHIPS_NAMESPACE: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships";
const SLIDE_RELATIONSHIP_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

/// Compatibility wrapper for callers that intentionally parse a local PPTX.
pub fn parse_powerpoint_file(path: &Path) -> Result<String, String> {
    ensure_supported_powerpoint_file(path)?;
    let bytes = fs::read(path).map_err(|_| "failed to read PowerPoint presentation".to_string())?;
    parse_powerpoint_from_bytes(&bytes)
}

/// Parses a PPTX package from upload bytes without retaining a caller path.
pub fn parse_powerpoint_from_bytes(bytes: &[u8]) -> Result<String, String> {
    let (text, _) = parse_powerpoint_text_from_bytes(bytes)?;
    Ok(text)
}

/// Parses and chunks raw PPTX bytes into a graph-ready assembly. Browser
/// uploads pass `None` for `path`, so no server-local path is invented.
pub fn parse_powerpoint_chunks_from_bytes(
    bytes: &[u8],
    path: Option<&Path>,
    user_id: impl Into<String>,
) -> Result<PowerpointAssembly, String> {
    let file_size_bytes = u64::try_from(bytes.len())
        .map_err(|_| "PPTX byte length does not fit in u64".to_string())?;
    let content_hash = sha256_bytes(bytes);
    let (text, slide_ranges) = parse_powerpoint_text_from_bytes(bytes)?;
    let source_path = path.map(|path| path.canonicalize().unwrap_or_else(|_| PathBuf::from(path)));
    let user_id = user_id.into();
    let document_id = document_id_from_content(&user_id, &content_hash);
    let chunks = chunk_nodes_from_text(&text, &slide_ranges, &document_id, &user_id);
    let token_count = chunks
        .iter()
        .map(|chunk| u64::from(chunk.token_count))
        .sum();

    Ok(PowerpointAssembly {
        document: Document {
            file_id: Uuid::new_v4().to_string(),
            document_id,
            user_id,
            file_name: source_path
                .as_deref()
                .and_then(Path::file_name)
                .and_then(|name| name.to_str())
                .unwrap_or("Presentation.pptx")
                .to_string(),
            source_type: "pptx".to_string(),
            local_path: source_path.map(|path| path.to_string_lossy().into_owned()),
            file_size_bytes,
            token_count,
            content_hash,
            rendered_pdf_path: None,
        },
        chunks,
    })
}

fn parse_powerpoint_text_from_bytes(bytes: &[u8]) -> Result<(String, Vec<SlideTextRange>), String> {
    validate_office_package(bytes, OfficeDocumentKind::PowerPoint)
        .map_err(|_| "invalid PowerPoint presentation".to_string())?;
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "invalid PowerPoint presentation".to_string())?;
    let slide_entries = presentation_slide_entries(&mut archive)?;

    let mut text = String::new();
    let mut slide_ranges = Vec::new();
    for (index, entry_name) in slide_entries.into_iter().enumerate() {
        let slide_number =
            u32::try_from(index + 1).map_err(|_| "invalid PowerPoint presentation".to_string())?;
        let mut slide_bytes = Vec::new();
        archive
            .by_name(&entry_name)
            .map_err(|_| "invalid PowerPoint presentation".to_string())?
            .read_to_end(&mut slide_bytes)
            .map_err(|_| "invalid PowerPoint presentation".to_string())?;
        let elements = parse_slide_xml(&slide_bytes)
            .map_err(|_| "invalid PowerPoint presentation".to_string())?;
        let body = slide_text(&elements);
        if body.trim().is_empty() {
            continue;
        }
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        let start_offset = text.len();
        text.push_str(&format!("Slide {slide_number}\n"));
        text.push_str(&body);
        slide_ranges.push(SlideTextRange {
            slide_number,
            start_offset,
            end_offset: text.len(),
        });
    }
    if text.trim().is_empty() {
        Err("PPTX did not contain readable text".to_string())
    } else {
        Ok((text, slide_ranges))
    }
}

fn presentation_slide_entries(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
) -> Result<Vec<String>, String> {
    let presentation_xml = read_zip_entry(archive, PRESENTATION_ENTRY_NAME)?;
    let presentation = parse_presentation_xml(&presentation_xml)?;
    let relationships = parse_presentation_relationships(&read_zip_entry(
        archive,
        PRESENTATION_RELATIONSHIPS_ENTRY_NAME,
    )?)?;
    let mut relationship_ids = HashSet::new();
    let mut entries = Vec::new();

    for slide_id in presentation.descendants().filter(|node| {
        node.is_element()
            && node.tag_name().namespace() == Some(PRESENTATIONML_NAMESPACE)
            && node.tag_name().name() == "sldId"
    }) {
        let relationship_id = slide_id
            .attribute((OFFICE_DOCUMENT_RELATIONSHIPS_NAMESPACE, "id"))
            .ok_or_else(|| "invalid PowerPoint presentation".to_string())?;
        if !relationship_ids.insert(relationship_id) {
            return Err("invalid PowerPoint presentation".to_string());
        }
        let target = relationships
            .get(relationship_id)
            .ok_or_else(|| "invalid PowerPoint presentation".to_string())?;
        entries.push(presentation_slide_entry_name(target)?);
    }

    if entries.is_empty() {
        return Err("PPTX did not contain readable text".to_string());
    }
    Ok(entries)
}

fn parse_presentation_xml(xml: &[u8]) -> Result<XmlDocument<'_>, String> {
    let xml =
        std::str::from_utf8(xml).map_err(|_| "invalid PowerPoint presentation".to_string())?;
    XmlDocument::parse(xml).map_err(|_| "invalid PowerPoint presentation".to_string())
}

fn parse_presentation_relationships(xml: &[u8]) -> Result<HashMap<String, String>, String> {
    let relationships = parse_presentation_xml(xml)?;
    let mut targets = HashMap::new();

    for relationship in relationships.descendants().filter(|node| {
        node.is_element()
            && node.tag_name().namespace() == Some(PACKAGE_RELATIONSHIPS_NAMESPACE)
            && node.tag_name().name() == "Relationship"
            && node.attribute("Type") == Some(SLIDE_RELATIONSHIP_TYPE)
    }) {
        if relationship.attribute("TargetMode") == Some("External") {
            return Err("invalid PowerPoint presentation".to_string());
        }
        let relationship_id = relationship
            .attribute("Id")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "invalid PowerPoint presentation".to_string())?;
        let target = relationship
            .attribute("Target")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "invalid PowerPoint presentation".to_string())?;
        if targets
            .insert(relationship_id.to_string(), target.to_string())
            .is_some()
        {
            return Err("invalid PowerPoint presentation".to_string());
        }
    }

    Ok(targets)
}

fn presentation_slide_entry_name(target: &str) -> Result<String, String> {
    let entry_name = format!("ppt/{target}");
    if !entry_name.starts_with("ppt/slides/") || !entry_name.ends_with(".xml") {
        return Err("invalid PowerPoint presentation".to_string());
    }
    Ok(entry_name)
}

fn read_zip_entry(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    entry_name: &str,
) -> Result<Vec<u8>, String> {
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|_| "invalid PowerPoint presentation".to_string())?;
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|_| "invalid PowerPoint presentation".to_string())?;
    Ok(bytes)
}

fn ensure_supported_powerpoint_file(path: &Path) -> Result<(), String> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("pptx") => Ok(()),
        Some("ppt") => Err("legacy .ppt files are not supported; expected .pptx".to_string()),
        Some(extension) => Err(format!(
            "unsupported PowerPoint extension .{extension}; expected pptx"
        )),
        None => Err(format!(
            "could not infer PowerPoint type for {}; expected .pptx",
            path.display()
        )),
    }
}

fn slide_text(elements: &[SlideElement]) -> String {
    let mut sorted_elements = elements.to_vec();
    sorted_elements.sort_by_key(|element| {
        let position = element.position();
        (position.y, position.x)
    });

    sorted_elements
        .iter()
        .filter_map(element_text)
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn chunk_nodes_from_text(
    text: &str,
    slide_ranges: &[SlideTextRange],
    document_id: &str,
    user_id: &str,
) -> Vec<DocumentChunk> {
    token_bounded_ranges(text)
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            let sequence_number =
                u32::try_from(index + 1).expect("PPTX chunk count should fit in u32");
            let chunk_text = &text[range.start_offset..range.end_offset];
            let content_hash = sha256_bytes(chunk_text.as_bytes());
            let chunk_id = sha256_bytes(
                format!("{user_id}\0{document_id}\0{sequence_number}\0{content_hash}").as_bytes(),
            );
            let page_numbers = slide_ranges
                .iter()
                .filter(|slide| {
                    range.start_offset < slide.end_offset && range.end_offset > slide.start_offset
                })
                .map(|slide| slide.slide_number)
                .collect::<Vec<_>>();

            DocumentChunk {
                chunk_id,
                document_id: document_id.to_string(),
                user_id: user_id.to_string(),
                text: chunk_text.to_string(),
                embedding: None,
                sequence_number,
                page_numbers: (!page_numbers.is_empty()).then_some(page_numbers),
                start_offset: range.start_offset,
                end_offset: range.end_offset,
                token_count: u32::try_from(range.token_count)
                    .expect("PPTX chunk token count should fit in u32"),
                content_hash,
                section_title: None,
            }
        })
        .collect()
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn element_text(element: &SlideElement) -> Option<String> {
    match element {
        SlideElement::Text(text, _) => Some(
            text.runs
                .iter()
                .map(|run| run.extract())
                .collect::<String>(),
        ),
        SlideElement::List(list, _) => Some(
            list.items
                .iter()
                .map(|item| {
                    item.runs
                        .iter()
                        .map(|run| run.extract())
                        .collect::<String>()
                })
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        SlideElement::Table(table, _) => Some(
            table
                .rows
                .iter()
                .map(|row| {
                    row.cells
                        .iter()
                        .map(|cell| {
                            cell.runs
                                .iter()
                                .map(|run| run.extract())
                                .collect::<String>()
                        })
                        .collect::<Vec<_>>()
                        .join("\t")
                })
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        // TODO: Add call to open ai for description to of image for later embedding
        SlideElement::Image(_, _) | SlideElement::Unknown => None,
    }
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/formats/powerpoint_tests.rs"]
mod tests;

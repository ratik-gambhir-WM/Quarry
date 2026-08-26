#![allow(dead_code)]

use crate::core::text_chunking::token_bounded_ranges;
use crate::services::document_ingestion_service::{Document, DocumentChunk};
use crate::utils::document_id_from_content;
use docx_rust::{
    app::{App, AppNoApNamespace, AppWithApNamespace},
    core::{Core, CoreNamespace, CoreNoNamespace},
    document::{
        Body, BodyContent, Comment, Comments, Drawing, EndNotes, FootNotes, Footer, Header,
        HeaderFooterReference, Hyperlink, Paragraph, ParagraphContent, Run, RunContent, SDTContent,
        Table, TableCell, TableCellContent, TableRowContent,
    },
    formatting::SectionProperty,
    Docx, DocxFile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File},
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocxAssembly {
    pub document: Document,
    pub chunks: Vec<DocumentChunk>,
}

/// Parses raw DOCX bytes into a DOCX package.
pub fn parse_docx_file_from_bytes(bytes: Vec<u8>) -> Result<DocxFile, String> {
    let docx_file = DocxFile::from_reader(Cursor::new(bytes)).map_err(|err| err.to_string())?;

    Ok(docx_file)
}

/// Parses plain text directly from raw DOCX file bytes.
pub fn parse_docx_from_bytes(bytes: Vec<u8>) -> Result<String, String> {
    let docx_file = parse_docx_file_from_bytes(bytes)?;

    parse_docx_file(&docx_file)
}

pub fn parse_docx_from_path(path: &Path) -> Result<String, String> {
    println!("parse_docx_from_path");
    let file = DocxFile::from_file(path);
    match file {
        Ok(file) => {
            println!("parse_docx_from_path - ok");
            parse_docx_file(&file)
        }
        Err(err) => {
            println!("parse_docx_from_path - error {err}");
            Err(err.to_string())
        }
    }
}

/// Parses a DOCX using the canonical plain-text parser and divides that exact
/// text into chunks. Offsets are UTF-8 byte offsets into the string returned by
/// [`parse_docx_from_path`], with an exclusive `end_offset`.
pub fn parse_docx_chunks_from_path(
    path: &Path,
    user_id: impl Into<String>,
) -> Result<DocxAssembly, String> {
    let text = parse_docx_from_path(path)?;
    let source_path = path.canonicalize().unwrap_or_else(|_| PathBuf::from(path));
    let file_size_bytes = fs::metadata(&source_path)
        .map_err(|err| {
            format!(
                "failed to read DOCX metadata for {}: {err}",
                source_path.display()
            )
        })?
        .len();
    let content_hash = sha256_file(&source_path)?;

    Ok(parse_docx_file_with_metadata(
        Some(&source_path),
        &user_id.into(),
        &text,
        file_size_bytes,
        content_hash,
    ))
}

/// Parses and chunks raw DOCX bytes into the same graph-ready assembly as the
/// path-based parser. An optional path provides the document's filename and
/// stable local-path identity; when omitted, `local_path` remains `None`.
pub fn parse_docx_chunks_from_bytes(
    bytes: Vec<u8>,
    path: Option<&Path>,
    user_id: impl Into<String>,
) -> Result<DocxAssembly, String> {
    let file_size_bytes = u64::try_from(bytes.len())
        .map_err(|_| format!("DOCX byte length `{}` does not fit in u64", bytes.len()))?;
    let content_hash = sha256_bytes(&bytes);
    let text = parse_docx_from_bytes(bytes)?;
    let source_path = path.map(|path| path.canonicalize().unwrap_or_else(|_| PathBuf::from(path)));

    Ok(parse_docx_file_with_metadata(
        source_path.as_deref(),
        &user_id.into(),
        &text,
        file_size_bytes,
        content_hash,
    ))
}

fn parse_docx_file_with_metadata(
    path: Option<&Path>,
    user_id: &str,
    text: &str,
    file_size_bytes: u64,
    content_hash: String,
) -> DocxAssembly {
    let local_path = path.map(|path| path.to_string_lossy().into_owned());
    let document_id = document_id_from_content(user_id, &content_hash);
    let chunks = chunk_nodes_from_text(text, &document_id, user_id);
    let token_count = chunks
        .iter()
        .map(|chunk| u64::from(chunk.token_count))
        .sum();
    let document = Document {
        file_id: Uuid::new_v4().to_string(),
        document_id,
        user_id: user_id.to_string(),
        file_name: path
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("Document.docx")
            .to_string(),
        source_type: "docx".to_string(),
        local_path,
        file_size_bytes,
        token_count,
        content_hash,
        rendered_pdf_path: None,
    };

    DocxAssembly { document, chunks }
}

fn chunk_nodes_from_text(text: &str, document_id: &str, user_id: &str) -> Vec<DocumentChunk> {
    token_bounded_ranges(text)
        .into_iter()
        .enumerate()
        .map(|(sequence_index, range)| {
            let sequence_number =
                u32::try_from(sequence_index + 1).expect("DOCX chunk count should fit in u32");
            let start_offset = range.start_offset;
            let end_offset = range.end_offset;
            let chunk_text = &text[start_offset..end_offset];
            let content_hash = content_hash_for_text(chunk_text);
            let chunk_id = content_hash_for_text(&format!(
                "{user_id}\0{document_id}\0{sequence_number}\0{content_hash}"
            ));

            DocumentChunk {
                chunk_id,
                document_id: document_id.to_string(),
                user_id: user_id.to_string(),
                text: chunk_text.to_string(),
                embedding: None,
                sequence_number,
                page_numbers: None,
                section_title: None,
                start_offset,
                end_offset,
                token_count: u32::try_from(range.token_count)
                    .expect("DOCX chunk token count should fit in u32"),
                content_hash,
            }
        })
        .collect()
}

fn content_hash_for_text(text: &str) -> String {
    sha256_bytes(text.as_bytes())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|err| format!("failed to open DOCX for hashing {}: {err}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|err| format!("failed to hash DOCX {}: {err}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

pub fn parse_docx_file(file: &DocxFile) -> Result<String, String> {
    println!("parse_docx_file");

    let docx = file.parse().map_err(|err| err.to_string())?;
    let full_text = collect_docx_text(&docx);

    if full_text.trim().is_empty() {
        Err("DOCX did not contain readable text".to_string())
    } else {
        Ok(full_text)
    }
}

struct DocxTextContext {
    footnotes: HashMap<String, String>,
    endnotes: HashMap<String, String>,
    comments: HashMap<String, String>,
    referenced_headers: BTreeSet<String>,
    referenced_footers: BTreeSet<String>,
    referenced_footnotes: BTreeSet<String>,
    referenced_endnotes: BTreeSet<String>,
    referenced_comments: BTreeSet<String>,
}

impl DocxTextContext {
    fn new(docx: &Docx<'_>) -> Self {
        Self {
            footnotes: footnote_texts(docx.footnotes.as_ref()),
            endnotes: endnote_texts(docx.endnotes.as_ref()),
            comments: comment_texts(docx.comments.as_ref()),
            referenced_headers: BTreeSet::new(),
            referenced_footers: BTreeSet::new(),
            referenced_footnotes: BTreeSet::new(),
            referenced_endnotes: BTreeSet::new(),
            referenced_comments: BTreeSet::new(),
        }
    }

    fn footnote_text(&mut self, id: &str) -> Option<String> {
        self.referenced_footnotes.insert(id.to_string());
        self.footnotes.get(id).cloned()
    }

    fn endnote_text(&mut self, id: &str) -> Option<String> {
        self.referenced_endnotes.insert(id.to_string());
        self.endnotes.get(id).cloned()
    }

    fn comment_text(&mut self, id: &str) -> Option<String> {
        self.referenced_comments.insert(id.to_string());
        self.comments.get(id).cloned()
    }

    fn unreferenced_text(&self) -> String {
        join_blocks(
            sorted_unreferenced_text(&self.footnotes, &self.referenced_footnotes)
                .into_iter()
                .chain(sorted_unreferenced_text(
                    &self.endnotes,
                    &self.referenced_endnotes,
                ))
                .chain(sorted_unreferenced_text(
                    &self.comments,
                    &self.referenced_comments,
                )),
        )
    }
}

fn collect_docx_text(docx: &Docx<'_>) -> String {
    let mut context = DocxTextContext::new(docx);

    let metadata = metadata_text(docx);
    let body = collect_body_text(&docx.document.body, docx, &mut context);
    let referenced_headers = context.referenced_headers.clone();
    let fallback_headers = unreferenced_docx_part_text(
        docx.headers.iter(),
        &referenced_headers,
        |header, context| header_text(header, context),
        &mut context,
    );
    let referenced_footers = context.referenced_footers.clone();
    let fallback_footers = unreferenced_docx_part_text(
        docx.footers.iter(),
        &referenced_footers,
        |footer, context| footer_text(footer, context),
        &mut context,
    );
    let unreferenced_notes = context.unreferenced_text();

    join_blocks(
        [
            metadata,
            fallback_headers,
            body,
            fallback_footers,
            unreferenced_notes,
        ]
        .into_iter()
        .filter(|text| !text.trim().is_empty()),
    )
}

fn unreferenced_docx_part_text<'a, T: 'a>(
    parts: impl Iterator<Item = (&'a String, &'a T)>,
    referenced_parts: &BTreeSet<String>,
    mut text: impl FnMut(&'a T, &mut DocxTextContext) -> String,
    context: &mut DocxTextContext,
) -> String {
    let mut parts = parts.collect::<Vec<_>>();
    parts.sort_by(|left, right| left.0.cmp(right.0));
    join_blocks(
        parts
            .into_iter()
            .filter(|(name, _)| !referenced_parts.contains(name.as_str()))
            .map(|(_, part)| text(part, context)),
    )
}

fn collect_body_text(body: &Body<'_>, docx: &Docx<'_>, context: &mut DocxTextContext) -> String {
    let mut document_blocks = Vec::new();
    let mut section_blocks = Vec::new();
    let mut section_had_boundary = false;

    for content in &body.content {
        if let Some(text) = body_content_text(content, context) {
            section_blocks.push(text);
        }

        if let Some(section_property) = body_content_section_property(content) {
            section_had_boundary = true;
            document_blocks.push(section_text(
                section_property,
                &section_blocks,
                docx,
                context,
            ));
            section_blocks.clear();
        }
    }

    if !section_blocks.is_empty() || !section_had_boundary {
        document_blocks.push(join_blocks(section_blocks.into_iter()));
    }

    join_blocks(document_blocks.into_iter())
}

fn body_content_section_property<'a>(
    content: &'a BodyContent<'a>,
) -> Option<&'a SectionProperty<'a>> {
    match content {
        BodyContent::Paragraph(paragraph) => paragraph
            .property
            .as_ref()
            .and_then(|property| property.section_property.as_ref()),
        BodyContent::SectionProperty(section_property) => Some(section_property),
        BodyContent::Table(_)
        | BodyContent::Sdt(_)
        | BodyContent::TableCell(_)
        | BodyContent::Run(_) => None,
    }
}

fn section_text(
    section_property: &SectionProperty<'_>,
    section_blocks: &[String],
    docx: &Docx<'_>,
    context: &mut DocxTextContext,
) -> String {
    let headers = section_headers_text(section_property, docx, context);
    let body = join_blocks(section_blocks.iter().cloned());
    let footers = section_footers_text(section_property, docx, context);

    join_blocks([headers, body, footers].into_iter())
}

fn section_headers_text(
    section_property: &SectionProperty<'_>,
    docx: &Docx<'_>,
    context: &mut DocxTextContext,
) -> String {
    let header_texts = section_property
        .header_footer_references
        .iter()
        .filter_map(|reference| match reference {
            HeaderFooterReference::Header(header) => header.id.as_ref(),
            HeaderFooterReference::Footer(_) => None,
        })
        .filter_map(|id| {
            let target = docx.document_rels.as_ref()?.get_target(id.as_ref())?;
            let part_name = normalize_word_part_name(target);
            let header = docx.headers.get(part_name)?;
            context.referenced_headers.insert(part_name.to_string());
            non_empty(header_text(header, context))
        });

    join_blocks(header_texts)
}

fn section_footers_text(
    section_property: &SectionProperty<'_>,
    docx: &Docx<'_>,
    context: &mut DocxTextContext,
) -> String {
    let footer_texts = section_property
        .header_footer_references
        .iter()
        .filter_map(|reference| match reference {
            HeaderFooterReference::Footer(footer) => footer.id.as_ref(),
            HeaderFooterReference::Header(_) => None,
        })
        .filter_map(|id| {
            let target = docx.document_rels.as_ref()?.get_target(id.as_ref())?;
            let part_name = normalize_word_part_name(target);
            let footer = docx.footers.get(part_name)?;
            context.referenced_footers.insert(part_name.to_string());
            non_empty(footer_text(footer, context))
        });

    join_blocks(footer_texts)
}

fn normalize_word_part_name(target: &str) -> &str {
    target
        .strip_prefix("word/")
        .or_else(|| target.strip_prefix("/word/"))
        .or_else(|| target.strip_prefix("./"))
        .unwrap_or(target)
}

fn sorted_unreferenced_text(
    text_by_id: &HashMap<String, String>,
    referenced_ids: &BTreeSet<String>,
) -> Vec<String> {
    let mut items = text_by_id
        .iter()
        .filter(|(id, _)| !referenced_ids.contains(*id))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.0.cmp(right.0));
    items.into_iter().map(|(_, text)| text.clone()).collect()
}

fn header_text(header: &Header<'_>, context: &mut DocxTextContext) -> String {
    join_blocks(
        header
            .content
            .iter()
            .filter_map(|content| body_content_text(content, context)),
    )
}

fn footer_text(footer: &Footer<'_>, context: &mut DocxTextContext) -> String {
    join_blocks(
        footer
            .content
            .iter()
            .filter_map(|content| body_content_text(content, context)),
    )
}

fn body_content_text(content: &BodyContent<'_>, context: &mut DocxTextContext) -> Option<String> {
    match content {
        BodyContent::Paragraph(paragraph) => non_empty(paragraph_text(paragraph, context)),
        BodyContent::Table(table) => non_empty(table_text(table, context)),
        BodyContent::Sdt(sdt) => sdt
            .content
            .as_ref()
            .and_then(|content| non_empty(sdt_content_text(content, context))),
        BodyContent::TableCell(cell) => non_empty(table_cell_text(cell, context)),
        BodyContent::Run(run) => non_empty(run_text(run, context)),
        BodyContent::SectionProperty(_) => None,
    }
}

fn sdt_content_text(content: &SDTContent<'_>, context: &mut DocxTextContext) -> String {
    join_blocks(
        content
            .content
            .iter()
            .filter_map(|content| body_content_text(content, context)),
    )
}

fn paragraph_text(paragraph: &Paragraph<'_>, context: &mut DocxTextContext) -> String {
    paragraph
        .content
        .iter()
        .filter_map(|content| match content {
            ParagraphContent::Run(run) => non_empty(run_text(run, context)),
            ParagraphContent::Link(link) => non_empty(hyperlink_text(link, context)),
            ParagraphContent::SDT(sdt) => sdt
                .content
                .as_ref()
                .and_then(|content| non_empty(sdt_content_text(content, context))),
            ParagraphContent::CommentRangeStart(_)
            | ParagraphContent::CommentRangeEnd(_)
            | ParagraphContent::BookmarkStart(_)
            | ParagraphContent::BookmarkEnd(_) => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn hyperlink_text(link: &Hyperlink<'_>, context: &mut DocxTextContext) -> String {
    let link_run_text = link
        .content
        .as_ref()
        .map(|run| run_text(run, context))
        .unwrap_or_default();
    let mut bidi_blocks = Vec::new();
    if let Some(bidi) = &link.bidirectional_embedding {
        for run in &bidi.runs {
            if let Some(text) = non_empty(run_text(run, context)) {
                bidi_blocks.push(text);
            }
        }

        for nested in &bidi.nested_levels {
            for run in &nested.runs {
                if let Some(text) = non_empty(run_text(run, context)) {
                    bidi_blocks.push(text);
                }
            }
        }
    }
    let bidi_text = join_blocks(bidi_blocks.into_iter());

    format!("{link_run_text}{bidi_text}")
}

fn run_text(run: &Run<'_>, context: &mut DocxTextContext) -> String {
    run.content
        .iter()
        .filter_map(|content| match content {
            RunContent::Text(text) => Some(text.text.to_string()),
            RunContent::DelText(text) => Some(text.text.to_string()),
            RunContent::InstrText(text) => Some(text.text.to_string()),
            RunContent::DelInstrText(text) => Some(text.text.to_string()),
            RunContent::Break(_) | RunContent::CarriageReturn(_) => Some("\n".to_string()),
            RunContent::Tab(_) => Some("\t".to_string()),
            RunContent::FootnoteReference(reference) => reference
                .id
                .as_ref()
                .and_then(|id| context.footnote_text(id.as_ref())),
            RunContent::EndnoteReference(reference) => reference
                .id
                .as_ref()
                .and_then(|id| context.endnote_text(id.as_ref())),
            RunContent::CommentReference(reference) => reference
                .id
                .as_ref()
                .and_then(|id| context.comment_text(id.as_ref())),
            RunContent::Drawing(drawing) => non_empty(drawing_text(drawing)),
            RunContent::NoBreakHyphen(_) => Some("-".to_string()),
            RunContent::SoftHyphen(_) => Some("-".to_string()),
            RunContent::DayShort(_)
            | RunContent::MonthShort(_)
            | RunContent::YearShort(_)
            | RunContent::DayLong(_)
            | RunContent::MonthLong(_)
            | RunContent::YearLong(_)
            | RunContent::AnnotationRef(_)
            | RunContent::FootnoteRef(_)
            | RunContent::EndnoteRef(_)
            | RunContent::Separator(_)
            | RunContent::ContinuationSeparator(_)
            | RunContent::Sym(_)
            | RunContent::PgNum(_)
            | RunContent::FieldChar(_)
            | RunContent::PTab(_)
            | RunContent::LastRenderedPageBreak(_) => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn drawing_text(drawing: &Drawing<'_>) -> String {
    let anchor_text = drawing
        .anchor
        .as_ref()
        .map(|anchor| {
            drawing_doc_property_text(
                anchor.doc_property.name.as_ref(),
                anchor.doc_property.descr.as_ref(),
            )
        })
        .unwrap_or_default();
    let inline_text = drawing
        .inline
        .as_ref()
        .map(|inline| {
            drawing_doc_property_text(
                inline.doc_property.name.as_ref(),
                inline.doc_property.descr.as_ref(),
            )
        })
        .unwrap_or_default();

    join_blocks([anchor_text, inline_text].into_iter())
}

fn drawing_doc_property_text(
    name: Option<&std::borrow::Cow<'_, str>>,
    description: Option<&std::borrow::Cow<'_, str>>,
) -> String {
    join_blocks(
        [
            name.map(ToString::to_string),
            description.map(ToString::to_string),
        ]
        .into_iter()
        .flatten(),
    )
}

fn table_text(table: &Table<'_>, context: &mut DocxTextContext) -> String {
    table
        .rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .filter_map(|cell| match cell {
                    TableRowContent::TableCell(cell) => non_empty(table_cell_text(cell, context)),
                    TableRowContent::SDT(sdt) => sdt
                        .content
                        .as_ref()
                        .and_then(|content| non_empty(sdt_content_text(content, context))),
                })
                .collect::<Vec<_>>()
                .join("\t")
        })
        .filter(|row| !row.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn table_cell_text(cell: &TableCell<'_>, context: &mut DocxTextContext) -> String {
    join_blocks(cell.content.iter().filter_map(|content| match content {
        TableCellContent::Paragraph(paragraph) => non_empty(paragraph_text(paragraph, context)),
    }))
}

fn footnote_texts(footnotes: Option<&FootNotes<'_>>) -> HashMap<String, String> {
    footnotes
        .into_iter()
        .flat_map(|notes| notes.content.iter())
        .filter(|note| note.ty.is_none())
        .filter_map(|note| {
            let id = note.id?.to_string();
            let mut context = empty_note_context();
            let text = join_blocks(
                note.content
                    .iter()
                    .filter_map(|content| body_content_text(content, &mut context)),
            );
            non_empty(text).map(|text| (id, text))
        })
        .collect()
}

fn endnote_texts(endnotes: Option<&EndNotes<'_>>) -> HashMap<String, String> {
    endnotes
        .into_iter()
        .flat_map(|notes| notes.content.iter())
        .filter(|note| note.ty.is_none())
        .filter_map(|note| {
            let id = note.id?.to_string();
            let mut context = empty_note_context();
            let text = join_blocks(
                note.content
                    .iter()
                    .filter_map(|content| body_content_text(content, &mut context)),
            );
            non_empty(text).map(|text| (id, text))
        })
        .collect()
}

fn comment_texts(comments: Option<&Comments<'_>>) -> HashMap<String, String> {
    comments
        .into_iter()
        .flat_map(|comments| comments.comments.iter())
        .filter_map(comment_text_entry)
        .collect()
}

fn comment_text_entry(comment: &Comment<'_>) -> Option<(String, String)> {
    let id = comment.id?.to_string();
    let mut context = empty_note_context();
    let text = paragraph_text(&comment.content, &mut context);
    non_empty(text).map(|text| (id, text))
}

fn empty_note_context() -> DocxTextContext {
    DocxTextContext {
        footnotes: HashMap::new(),
        endnotes: HashMap::new(),
        comments: HashMap::new(),
        referenced_headers: BTreeSet::new(),
        referenced_footers: BTreeSet::new(),
        referenced_footnotes: BTreeSet::new(),
        referenced_endnotes: BTreeSet::new(),
        referenced_comments: BTreeSet::new(),
    }
}

fn metadata_text(docx: &Docx<'_>) -> String {
    let core = docx.core.as_ref().map(core_text).unwrap_or_default();
    let app = docx.app.as_ref().map(app_text).unwrap_or_default();

    join_blocks([core, app].into_iter())
}

fn core_text(core: &Core<'_>) -> String {
    match core {
        Core::CoreNamespace(core) => core_namespace_text(core),
        Core::CoreNoNamespace(core) => core_no_namespace_text(core),
    }
}

fn core_namespace_text(core: &CoreNamespace<'_>) -> String {
    join_blocks(
        [
            core.title.as_ref(),
            core.subject.as_ref(),
            core.creator.as_ref(),
            core.keywords.as_ref(),
            core.description.as_ref(),
            core.last_modified_by.as_ref(),
            core.revision.as_ref(),
            core.created.as_ref(),
            core.modified.as_ref(),
            core.content_status.as_ref(),
            core.language.as_ref(),
            core.category.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(ToString::to_string),
    )
}

fn core_no_namespace_text(core: &CoreNoNamespace<'_>) -> String {
    join_blocks(
        [
            core.title.as_ref(),
            core.subject.as_ref(),
            core.creator.as_ref(),
            core.keywords.as_ref(),
            core.description.as_ref(),
            core.last_modified_by.as_ref(),
            core.revision.as_ref(),
            core.created.as_ref(),
            core.modified.as_ref(),
            core.content_status.as_ref(),
            core.language.as_ref(),
            core.category.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(ToString::to_string),
    )
}

fn app_text(app: &App<'_>) -> String {
    match app {
        App::AppNoApNamespace(app) => app_no_namespace_text(app),
        App::AppWithApNamespace(app) => app_with_namespace_text(app),
    }
}

fn app_no_namespace_text(app: &AppNoApNamespace<'_>) -> String {
    join_blocks(
        [
            app.template.as_ref(),
            app.total_time.as_ref(),
            app.pages.as_ref(),
            app.words.as_ref(),
            app.characters.as_ref(),
            app.application.as_ref(),
            app.doc_security.as_ref(),
            app.lines.as_ref(),
            app.paragraphs.as_ref(),
            app.scale_crop.as_ref(),
            app.company.as_ref(),
            app.links_up_to_date.as_ref(),
            app.characters_with_spaces.as_ref(),
            app.shared_doc.as_ref(),
            app.hyperlinks_changed.as_ref(),
            app.app_version.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(ToString::to_string),
    )
}

fn app_with_namespace_text(app: &AppWithApNamespace<'_>) -> String {
    join_blocks(
        [
            app.template.as_ref(),
            app.total_time.as_ref(),
            app.pages.as_ref(),
            app.words.as_ref(),
            app.characters.as_ref(),
            app.application.as_ref(),
            app.doc_security.as_ref(),
            app.lines.as_ref(),
            app.paragraphs.as_ref(),
            app.scale_crop.as_ref(),
            app.company.as_ref(),
            app.links_up_to_date.as_ref(),
            app.characters_with_spaces.as_ref(),
            app.shared_doc.as_ref(),
            app.hyperlinks_changed.as_ref(),
            app.app_version.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(ToString::to_string),
    )
}

fn join_blocks(blocks: impl Iterator<Item = String>) -> String {
    blocks
        .map(|block| block.trim().to_string())
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
#[path = "../../../tests/core/parsers/docx_tests.rs"]
mod tests;

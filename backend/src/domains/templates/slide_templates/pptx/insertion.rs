use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read, Write},
};

use anyhow::{ensure, Context};
use roxmltree::Document;
use zip_slide::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_EXPANDED_BYTES: u64 = 250 * 1024 * 1024;
const SLIDE_RELATIONSHIP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";

pub(super) fn insert(
    generated_bytes: &[u8],
    target_bytes: &[u8],
    insert_after_slide: usize,
) -> anyhow::Result<Vec<u8>> {
    let source = read_package(generated_bytes)?;
    let mut target = read_package(target_bytes)?;
    let presentation_path = "ppt/presentation.xml";
    let presentation_rels_path = "ppt/_rels/presentation.xml.rels";
    let content_types_path = "[Content_Types].xml";
    let presentation_xml = target.text(presentation_path)?;
    let presentation_rels_xml = target.text(presentation_rels_path)?;
    let mut content_types_xml = target.text(content_types_path)?;

    let source_slide_numbers = source.slide_numbers();
    ensure!(
        !source_slide_numbers.is_empty(),
        "generated deck has no slides"
    );
    let target_slide_ids = parse_slide_ids(&presentation_xml)?;
    let insert_after_slide = insert_after_slide.min(target_slide_ids.len());
    let target_layout = find_default_slide_layout(&target)?;
    let mut next_slide_number = target
        .slide_numbers()
        .into_iter()
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    let mut next_slide_id = target_slide_ids
        .iter()
        .map(|entry| entry.id)
        .max()
        .unwrap_or(255)
        .max(255)
        .saturating_add(1);
    let mut presentation_relationships = parse_relationships(&presentation_rels_xml)?;
    let mut inserted_entries = Vec::with_capacity(source_slide_numbers.len());
    let mut inserted_ids = Vec::with_capacity(source_slide_numbers.len());

    for source_slide_number in source_slide_numbers {
        let source_slide_path = format!("ppt/slides/slide{source_slide_number}.xml");
        let source_slide_rels_path =
            format!("ppt/slides/_rels/slide{source_slide_number}.xml.rels");
        let source_slide_xml = source
            .parts
            .get(&source_slide_path)
            .context("generated slide is missing")?
            .clone();
        let target_slide_path = format!("ppt/slides/slide{next_slide_number}.xml");
        let target_slide_rels_path = format!("ppt/slides/_rels/slide{next_slide_number}.xml.rels");
        let relationship_id = next_relationship_id(&presentation_relationships);

        target.set(target_slide_path.clone(), source_slide_xml);
        let rewritten_relationships = if let Some(bytes) = source.parts.get(&source_slide_rels_path)
        {
            rewrite_slide_relationships(
                &source,
                &mut target,
                std::str::from_utf8(bytes)?,
                target_layout.as_deref(),
            )?
        } else {
            build_relationships(&[])
        };
        target.set(target_slide_rels_path, rewritten_relationships.into_bytes());
        presentation_relationships.push(Relationship {
            id: relationship_id.clone(),
            relationship_type: SLIDE_RELATIONSHIP.to_owned(),
            target: format!("slides/slide{next_slide_number}.xml"),
            target_mode: None,
        });
        ensure_content_type_override(
            &mut content_types_xml,
            &format!("/{target_slide_path}"),
            "application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
        )?;
        inserted_entries.push(format!(
            "<p:sldId id=\"{next_slide_id}\" r:id=\"{}\"/>",
            xml_escape(&relationship_id)
        ));
        inserted_ids.push(next_slide_id);
        next_slide_number = next_slide_number.saturating_add(1);
        next_slide_id = next_slide_id.saturating_add(1);
    }

    ensure_media_content_types(&mut content_types_xml)?;
    let presentation_xml =
        insert_slide_ids(&presentation_xml, &inserted_entries, insert_after_slide)?;
    let presentation_xml = insert_section_ids(
        &presentation_xml,
        &inserted_ids,
        &target_slide_ids,
        insert_after_slide,
    );
    target.set(presentation_path.to_owned(), presentation_xml.into_bytes());
    target.set(
        presentation_rels_path.to_owned(),
        build_relationships(&presentation_relationships).into_bytes(),
    );
    target.set(
        content_types_path.to_owned(),
        content_types_xml.into_bytes(),
    );
    target.write()
}

struct Package {
    order: Vec<String>,
    parts: HashMap<String, Vec<u8>>,
}

impl Package {
    fn text(&self, path: &str) -> anyhow::Result<String> {
        String::from_utf8(
            self.parts
                .get(path)
                .with_context(|| format!("PowerPoint package is missing {path}"))?
                .clone(),
        )
        .with_context(|| format!("PowerPoint part {path} is not UTF-8 XML"))
    }

    fn set(&mut self, path: String, bytes: Vec<u8>) {
        if !self.parts.contains_key(&path) {
            self.order.push(path.clone());
        }
        self.parts.insert(path, bytes);
    }

    fn slide_numbers(&self) -> Vec<usize> {
        let mut numbers = self
            .parts
            .keys()
            .filter_map(|path| {
                path.strip_prefix("ppt/slides/slide")?
                    .strip_suffix(".xml")?
                    .parse::<usize>()
                    .ok()
            })
            .filter(|number| *number > 0)
            .collect::<Vec<_>>();
        numbers.sort_unstable();
        numbers
    }

    fn write(self) -> anyhow::Result<Vec<u8>> {
        let cursor = super::output::BoundedCursor::new();
        let mut archive = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for path in self.order {
            let Some(bytes) = self.parts.get(&path) else {
                continue;
            };
            archive.start_file(path, options)?;
            archive.write_all(bytes)?;
        }
        Ok(archive.finish()?.into_inner())
    }
}

fn read_package(bytes: &[u8]) -> anyhow::Result<Package> {
    ensure!(
        bytes.starts_with(b"PK"),
        "PowerPoint package is not a ZIP archive"
    );
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    ensure!(
        archive.len() <= MAX_ARCHIVE_ENTRIES,
        "PowerPoint package has too many parts"
    );
    let mut order = Vec::with_capacity(archive.len());
    let mut parts = HashMap::with_capacity(archive.len());
    let mut expanded_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        let path = entry.name().to_owned();
        ensure!(
            is_safe_part_path(&path),
            "PowerPoint package contains an unsafe part path"
        );
        expanded_bytes = expanded_bytes
            .checked_add(entry.size())
            .context("PowerPoint package expansion size overflow")?;
        ensure!(
            expanded_bytes <= MAX_EXPANDED_BYTES,
            "PowerPoint package expands too far"
        );
        let mut part = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or(0));
        entry.read_to_end(&mut part)?;
        ensure!(
            parts.insert(path.clone(), part).is_none(),
            "duplicate PowerPoint part"
        );
        order.push(path);
    }
    Ok(Package { order, parts })
}

fn is_safe_part_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path
            .split('/')
            .any(|segment| segment == ".." || segment.is_empty())
}

#[derive(Clone)]
struct Relationship {
    id: String,
    relationship_type: String,
    target: String,
    target_mode: Option<String>,
}

fn parse_relationships(xml: &str) -> anyhow::Result<Vec<Relationship>> {
    let document = Document::parse(xml)?;
    Ok(document
        .descendants()
        .filter(|node| node.has_tag_name("Relationship"))
        .filter_map(|node| {
            Some(Relationship {
                id: node.attribute("Id")?.to_owned(),
                relationship_type: node.attribute("Type")?.to_owned(),
                target: node.attribute("Target")?.to_owned(),
                target_mode: node.attribute("TargetMode").map(str::to_owned),
            })
        })
        .collect())
}

fn build_relationships(relationships: &[Relationship]) -> String {
    let body = relationships
        .iter()
        .map(|relationship| {
            let target_mode = relationship
                .target_mode
                .as_ref()
                .map_or_else(String::new, |mode| {
                    format!(" TargetMode=\"{}\"", xml_escape(mode))
                });
            format!(
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{target_mode}/>",
                xml_escape(&relationship.id),
                xml_escape(&relationship.relationship_type),
                xml_escape(&relationship.target)
            )
        })
        .collect::<String>();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{body}</Relationships>"
    )
}

fn next_relationship_id(relationships: &[Relationship]) -> String {
    let used = relationships
        .iter()
        .map(|relationship| relationship.id.as_str())
        .collect::<HashSet<_>>();
    let mut next = relationships
        .iter()
        .filter_map(|relationship| relationship.id.strip_prefix("rId")?.parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    while used.contains(format!("rId{next}").as_str()) {
        next = next.saturating_add(1);
    }
    format!("rId{next}")
}

fn find_default_slide_layout(package: &Package) -> anyhow::Result<Option<String>> {
    let Some(first) = package.slide_numbers().first().copied() else {
        return Ok(None);
    };
    let path = format!("ppt/slides/_rels/slide{first}.xml.rels");
    let Some(bytes) = package.parts.get(&path) else {
        return Ok(None);
    };
    Ok(parse_relationships(std::str::from_utf8(bytes)?)?
        .into_iter()
        .find(|relationship| relationship.relationship_type.ends_with("/slideLayout"))
        .map(|relationship| relationship.target))
}

fn rewrite_slide_relationships(
    source: &Package,
    target: &mut Package,
    xml: &str,
    target_layout: Option<&str>,
) -> anyhow::Result<String> {
    let mut rewritten = Vec::new();
    for mut relationship in parse_relationships(xml)? {
        if relationship.relationship_type.ends_with("/slideLayout") {
            if let Some(layout) = target_layout {
                relationship.target = layout.to_owned();
            }
            rewritten.push(relationship);
        } else if relationship.relationship_type.ends_with("/notesSlide") {
            continue;
        } else if relationship.relationship_type.contains("/image") {
            let source_path = resolve_part_path("ppt/slides", &relationship.target)?;
            let Some(bytes) = source.parts.get(&source_path) else {
                continue;
            };
            let extension = source_path
                .rsplit_once('.')
                .map_or("bin", |(_, ext)| ext)
                .to_ascii_lowercase();
            let file_name = next_media_file_name(target, &extension);
            target.set(format!("ppt/media/{file_name}"), bytes.clone());
            relationship.target = format!("../media/{file_name}");
            rewritten.push(relationship);
        } else {
            rewritten.push(relationship);
        }
    }
    Ok(build_relationships(&rewritten))
}

fn resolve_part_path(base: &str, target: &str) -> anyhow::Result<String> {
    let mut resolved = base.split('/').map(str::to_owned).collect::<Vec<_>>();
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                ensure!(
                    resolved.pop().is_some(),
                    "relationship target escapes package"
                );
            }
            value => resolved.push(value.to_owned()),
        }
    }
    Ok(resolved.join("/"))
}

fn next_media_file_name(package: &Package, extension: &str) -> String {
    let suffix = format!(".{extension}");
    let next = package
        .parts
        .keys()
        .filter_map(|path| {
            path.strip_prefix("ppt/media/image")?
                .strip_suffix(&suffix)?
                .parse::<usize>()
                .ok()
        })
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    format!("image{next}.{extension}")
}

struct SlideId {
    id: u32,
}

fn parse_slide_ids(xml: &str) -> anyhow::Result<Vec<SlideId>> {
    let document = Document::parse(xml)?;
    document
        .descendants()
        .filter(|node| node.tag_name().name() == "sldId" && node.tag_name().namespace().is_some())
        .filter(|node| {
            node.attribute((
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                "id",
            ))
            .is_some()
        })
        .map(|node| {
            Ok(SlideId {
                id: node
                    .attribute("id")
                    .context("slide ID is missing")?
                    .parse()?,
            })
        })
        .collect()
}

fn insert_slide_ids(xml: &str, entries: &[String], after: usize) -> anyhow::Result<String> {
    let list_start = xml.find("<p:sldIdLst").context("slide list is missing")?;
    let opening_end = xml[list_start..]
        .find('>')
        .map(|offset| list_start + offset + 1)
        .context("slide list opening is invalid")?;
    let list_end = xml[opening_end..]
        .find("</p:sldIdLst>")
        .map(|offset| opening_end + offset)
        .context("slide list closing is missing")?;
    let mut ranges = Vec::new();
    let mut cursor = opening_end;
    while let Some(offset) = xml[cursor..list_end].find("<p:sldId ") {
        let start = cursor + offset;
        let end = xml[start..list_end]
            .find("/>")
            .map(|value| start + value + 2)
            .context("slide ID entry is invalid")?;
        ranges.push((start, end));
        cursor = end;
    }
    ensure!(after <= ranges.len(), "insert position exceeds slide count");
    let insertion = if after == 0 {
        opening_end
    } else {
        ranges[after - 1].1
    };
    Ok(format!(
        "{}{}{}",
        &xml[..insertion],
        entries.concat(),
        &xml[insertion..]
    ))
}

fn insert_section_ids(
    xml: &str,
    inserted_ids: &[u32],
    existing_ids: &[SlideId],
    after: usize,
) -> String {
    if inserted_ids.is_empty() || !xml.contains(":sectionLst") {
        return xml.to_owned();
    }
    let entries = inserted_ids
        .iter()
        .map(|id| format!("<p14:sldId id=\"{id}\"/>"))
        .collect::<String>();
    if let Some(insertion) = after
        .checked_sub(1)
        .and_then(|i| existing_ids.get(i))
        .and_then(|anchor| find_tag_with_attribute(xml, "<p14:sldId", "id", &anchor.id.to_string()))
        .and_then(|start| xml[start..].find("/>").map(|end| start + end + 2))
    {
        return format!("{}{}{}", &xml[..insertion], entries, &xml[insertion..]);
    }
    if let Some(insertion) = xml
        .find("<p14:sldIdLst")
        .and_then(|start| xml[start..].find('>').map(|end| start + end + 1))
    {
        return format!("{}{}{}", &xml[..insertion], entries, &xml[insertion..]);
    }
    xml.to_owned()
}

fn find_tag_with_attribute(xml: &str, tag: &str, attribute: &str, value: &str) -> Option<usize> {
    let mut cursor = 0;
    while let Some(offset) = xml[cursor..].find(tag) {
        let start = cursor + offset;
        let end = xml[start..].find('>').map(|end| start + end)?;
        if xml[start..=end].contains(&format!("{attribute}=\"{value}\"")) {
            return Some(start);
        }
        cursor = end + 1;
    }
    None
}

fn ensure_content_type_override(
    xml: &mut String,
    part_name: &str,
    content_type: &str,
) -> anyhow::Result<()> {
    if xml.contains(&format!("PartName=\"{part_name}\"")) {
        return Ok(());
    }
    let closing = xml
        .rfind("</Types>")
        .context("content types closing tag is missing")?;
    xml.insert_str(
        closing,
        &format!(
            "<Override PartName=\"{}\" ContentType=\"{}\"/>",
            xml_escape(part_name),
            xml_escape(content_type)
        ),
    );
    Ok(())
}

fn ensure_media_content_types(xml: &mut String) -> anyhow::Result<()> {
    for (extension, content_type) in [
        ("png", "image/png"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("gif", "image/gif"),
        ("svg", "image/svg+xml"),
        ("emf", "image/emf"),
    ] {
        if xml.contains(&format!("Extension=\"{extension}\"")) {
            continue;
        }
        let closing = xml
            .rfind("</Types>")
            .context("content types closing tag is missing")?;
        xml.insert_str(
            closing,
            &format!("<Default Extension=\"{extension}\" ContentType=\"{content_type}\"/>"),
        );
    }
    Ok(())
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

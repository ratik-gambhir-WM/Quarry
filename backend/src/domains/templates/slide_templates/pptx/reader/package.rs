//! Bounded OOXML package loading and relationship resolution.

use super::super::super::SlideResult;
use super::{
    invalid_powerpoint, powerpoint_too_large, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_EXPANDED_BYTES,
};
use roxmltree::Document;
use std::{
    collections::HashMap,
    io::{Cursor, Read},
};
use zip::ZipArchive;

/// Checks the non-ZIP64 central directory before any package entry is expanded.
/// The same validation runs for imported decks and insertion targets.
pub(in super::super) fn validate_package(source: &[u8]) -> SlideResult<()> {
    fn u16_at(source: &[u8], offset: usize) -> Option<u16> {
        Some(u16::from_le_bytes(
            source.get(offset..offset + 2)?.try_into().ok()?,
        ))
    }
    fn u32_at(source: &[u8], offset: usize) -> Option<u32> {
        Some(u32::from_le_bytes(
            source.get(offset..offset + 4)?.try_into().ok()?,
        ))
    }
    let start = source.len().saturating_sub(65_535 + 22);
    let end = source
        .len()
        .checked_sub(22)
        .ok_or_else(invalid_powerpoint)?;
    let directory_end = (start..=end)
        .rev()
        .find(|offset| u32_at(source, *offset) == Some(0x0605_4b50))
        .ok_or_else(invalid_powerpoint)?;
    let disk = u16_at(source, directory_end + 4).ok_or_else(invalid_powerpoint)?;
    let directory_disk = u16_at(source, directory_end + 6).ok_or_else(invalid_powerpoint)?;
    let count = u16_at(source, directory_end + 10).ok_or_else(invalid_powerpoint)?;
    let directory_size = u32_at(source, directory_end + 12).ok_or_else(invalid_powerpoint)?;
    let directory_offset = u32_at(source, directory_end + 16).ok_or_else(invalid_powerpoint)?;
    if disk != 0
        || directory_disk != 0
        || count == u16::MAX
        || usize::from(count) > MAX_ENTRIES
        || directory_size == u32::MAX
        || directory_offset == u32::MAX
        || u64::from(directory_offset) + u64::from(directory_size) > directory_end as u64
    {
        return Err(invalid_powerpoint());
    }
    let mut offset = directory_offset as usize;
    let mut total = 0_u64;
    let mut required = std::collections::HashSet::from([
        "[Content_Types].xml",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
    ]);
    for _ in 0..count {
        if offset + 46 > source.len() || u32_at(source, offset) != Some(0x0201_4b50) {
            return Err(invalid_powerpoint());
        }
        let entry_bytes = u32_at(source, offset + 24).ok_or_else(invalid_powerpoint)?;
        let name_length = usize::from(u16_at(source, offset + 28).ok_or_else(invalid_powerpoint)?);
        let extra_length = usize::from(u16_at(source, offset + 30).ok_or_else(invalid_powerpoint)?);
        let comment_length =
            usize::from(u16_at(source, offset + 32).ok_or_else(invalid_powerpoint)?);
        let next = offset + 46 + name_length + extra_length + comment_length;
        if entry_bytes == u32::MAX || next > source.len() {
            return Err(invalid_powerpoint());
        }
        total += u64::from(entry_bytes);
        if u64::from(entry_bytes) > MAX_ENTRY_BYTES || total > MAX_EXPANDED_BYTES {
            return Err(powerpoint_too_large());
        }
        let name = String::from_utf8_lossy(&source[offset + 46..offset + 46 + name_length]);
        if !safe_part_name(&name) {
            return Err(invalid_powerpoint());
        }
        required.remove(name.as_ref());
        offset = next;
    }
    if offset != directory_offset as usize + directory_size as usize || !required.is_empty() {
        return Err(invalid_powerpoint());
    }
    Ok(())
}

pub(super) fn relationship_target_with_segment<'a>(
    relationships: &'a HashMap<String, String>,
    segment: &str,
) -> Option<&'a str> {
    relationships
        .values()
        .find(|path| path.contains(segment))
        .map(String::as_str)
}

pub(super) fn part_relationships(
    parts: &HashMap<String, Vec<u8>>,
    part_path: &str,
) -> Option<HashMap<String, String>> {
    let bytes = parts.get(&relationship_part_path(part_path))?;
    let xml = std::str::from_utf8(bytes).ok()?;
    relationships(xml, part_path).ok()
}

pub(super) fn read_package(source: &[u8]) -> SlideResult<HashMap<String, Vec<u8>>> {
    validate_package(source)?;
    let mut archive = ZipArchive::new(Cursor::new(source)).map_err(|_| invalid_powerpoint())?;
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(invalid_powerpoint());
    }
    let mut total = 0_u64;
    let mut parts = HashMap::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|_| invalid_powerpoint())?;
        let name = entry.name().map_err(|_| invalid_powerpoint())?.into_owned();
        if !safe_part_name(&name) || entry.size() > MAX_ENTRY_BYTES {
            return Err(if entry.size() > MAX_ENTRY_BYTES {
                powerpoint_too_large()
            } else {
                invalid_powerpoint()
            });
        }
        if entry.is_dir() {
            continue;
        }
        let declared_size = entry.size();
        let bytes = read_part(entry, declared_size, &mut total)?;
        parts.insert(name, bytes);
    }
    for required in [
        "[Content_Types].xml",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
    ] {
        if !parts.contains_key(required) {
            return Err(invalid_powerpoint());
        }
    }
    Ok(parts)
}

/// Bound decompression independently of attacker-controlled ZIP size declarations.
/// Shared with insertion so copied target parts have the same expansion budgets.
pub(in super::super) fn read_part(
    entry: impl Read,
    declared_size: u64,
    total: &mut u64,
) -> SlideResult<Vec<u8>> {
    let limit = MAX_ENTRY_BYTES.min(MAX_EXPANDED_BYTES.saturating_sub(*total));
    if declared_size > limit {
        return Err(powerpoint_too_large());
    }
    let mut bytes = Vec::new();
    // One extra byte distinguishes an exact fit from an oversized stream without
    // decompressing the rest of a forged entry. Reaching EOF still checks its CRC.
    entry
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid_powerpoint())?;
    if bytes.len() as u64 > limit {
        return Err(powerpoint_too_large());
    }
    *total += bytes.len() as u64;
    Ok(bytes)
}

pub(super) fn relationships(xml: &str, owning_part: &str) -> SlideResult<HashMap<String, String>> {
    let document = parse_xml(xml)?;
    let mut result = HashMap::new();
    for node in document
        .descendants()
        .filter(|node| node.tag_name().name() == "Relationship")
    {
        let (Some(id), Some(target)) = (node.attribute("Id"), node.attribute("Target")) else {
            continue;
        };
        let target = resolve_target(owning_part, target).map_err(|_| invalid_powerpoint())?;
        result.insert(id.to_owned(), target);
    }
    Ok(result)
}

fn resolve_target(owning_part: &str, target: &str) -> Result<String, ()> {
    if target.contains('\0') || target.contains("\\") {
        return Err(());
    }
    let mut components = if target.starts_with('/') {
        Vec::new()
    } else {
        owning_part
            .rsplit_once('/')
            .map_or(Vec::new(), |(directory, _)| {
                directory.split('/').collect::<Vec<_>>()
            })
    };
    for component in target.trim_start_matches('/').split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop().ok_or(())?;
            }
            value => components.push(value),
        }
    }
    Ok(components.join("/"))
}

pub(super) fn relationship_part_path(part: &str) -> String {
    let (directory, file) = part.rsplit_once('/').unwrap_or(("", part));
    format!("{directory}/_rels/{file}.rels")
}

fn safe_part_name(name: &str) -> bool {
    !name.starts_with('/')
        && !name.contains('\0')
        && !name.contains('\\')
        && !name.split('/').any(|component| component == "..")
}

pub(super) fn utf8_part<'a>(
    parts: &'a HashMap<String, Vec<u8>>,
    path: &str,
) -> SlideResult<&'a str> {
    std::str::from_utf8(parts.get(path).ok_or_else(invalid_powerpoint)?)
        .map_err(|_| invalid_powerpoint())
}

pub(super) fn parse_xml(xml: &str) -> SlideResult<Document<'_>> {
    Document::parse(xml).map_err(|_| invalid_powerpoint())
}

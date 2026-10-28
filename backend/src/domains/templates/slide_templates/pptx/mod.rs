//! Native Rust PowerPoint OOXML reader and writer.

mod insertion;
mod reader;
mod writer;

use super::{CanvasDocument, Conversion, SlideResult};

/// Renders compact canvas JSON into a PowerPoint OOXML ZIP package.
pub fn write(document: &CanvasDocument) -> anyhow::Result<Vec<u8>> {
    writer::write(document)
}

/// Imports supported PowerPoint OOXML drawables into compact canvas JSON.
pub fn read(source: &[u8]) -> SlideResult<Conversion> {
    reader::read(source)
}

/// Inserts generated slides into an existing PowerPoint OOXML package.
pub(super) fn insert(
    generated: &[u8],
    target: &[u8],
    insert_after_slide: usize,
) -> anyhow::Result<Vec<u8>> {
    insertion::insert(generated, target, insert_after_slide)
}

/// Validates a PowerPoint ZIP package before decompression, including ZIP64 and expansion limits.
pub fn validate_package(source: &[u8]) -> SlideResult<()> {
    reader::validate_package(source)
}
mod output;

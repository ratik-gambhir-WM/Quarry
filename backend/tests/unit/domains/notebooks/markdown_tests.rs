use super::*;
use crate::domains::notebooks::{model::*, test_support};
#[test]
fn notebook_markdown_exports_supported_native_shapes_deterministically() {
    let note = Note {
        summary: NoteSummary {
            note_id: "note".into(),
            notebook_id: "book".into(),
            folder_id: None,
            title: "Research".into(),
            content_schema_version: 1,
            revision: 1,
            created_at: now(),
            updated_at: now(),
        },
        blocks: test_support::blocks(),
    };
    let result = export(&note).unwrap();
    assert_eq!(result, export(&note).unwrap());
    for expected in [
        "# Research",
        "**Native café **",
        "[*source*](<https://example.com/research>)",
        "## Architecture",
        "- Bullet",
        "3. Number",
        "- [x] Task",
        "> Quoted",
        "```rust",
        "let value = 1;",
        "| Key | Value |",
        "Nested",
    ] {
        assert!(result.contains(expected), "missing {expected}: {result}");
    }
}
#[test]
fn notebook_chunks_are_bounded_ordered_unicode_safe_and_empty_aware() {
    assert!(chunks(" \n").unwrap().is_empty());
    let text = format!("## Heading\n{}", "界".repeat(1600));
    let result = chunks(&text).unwrap();
    assert!(result.len() > 2);
    assert!(result.iter().all(|c| c.len() <= 2561));
    assert!(result.iter().all(|c| c.contains("Heading")));
    assert_eq!(result, chunks(&text).unwrap());
}

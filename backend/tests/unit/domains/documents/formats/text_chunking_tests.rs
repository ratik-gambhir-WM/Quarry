use super::*;

#[test]
fn ranges_are_contiguous_utf8_slices() {
    let text = "Quarry 🏗️ ".repeat(2_000);
    let ranges = token_bounded_ranges(&text);

    assert!(!ranges.is_empty());
    assert_eq!(ranges.first().unwrap().start_offset, 0);
    assert_eq!(ranges.last().unwrap().end_offset, text.len());
    assert!(ranges
        .iter()
        .all(|range| range.token_count <= MAX_TOKEN_CHUNK));
    assert!(ranges
        .windows(2)
        .all(|pair| pair[0].end_offset == pair[1].start_offset));
    for range in ranges {
        assert!(text.get(range.start_offset..range.end_offset).is_some());
    }
}

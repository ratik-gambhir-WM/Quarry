use super::*;
use tiktoken_rs::cl100k_base;

#[test]
fn ranges_count_tokens_with_the_openai_embedding_encoding() {
    let text = "Quarry 🏗️ analyzes evidence.";

    let ranges = token_bounded_ranges(text);
    let expected_token_count = cl100k_base()
        .expect("cl100k tokenizer data should be valid")
        .count_with_special_tokens(text);

    assert_eq!(
        ranges,
        vec![TokenChunkRange {
            start_offset: 0,
            end_offset: text.len(),
            token_count: expected_token_count,
        }]
    );
}

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

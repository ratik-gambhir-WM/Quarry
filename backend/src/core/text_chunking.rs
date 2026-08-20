use tiktoken_rs::o200k_base;

pub const MAX_TOKEN_CHUNK: usize = 800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenChunkRange {
    pub start_offset: usize,
    pub end_offset: usize,
    pub token_count: usize,
}

/// Returns contiguous token-bounded ranges with exclusive UTF-8 byte offsets.
pub fn token_bounded_ranges(text: &str) -> Vec<TokenChunkRange> {
    if text.is_empty() {
        return Vec::new();
    }

    let tokenizer = o200k_base().expect("o200k tokenizer data should be valid");
    let boundaries = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .collect::<Vec<_>>();
    let mut ranges = Vec::new();
    let mut start_index = 0;

    while start_index + 1 < boundaries.len() {
        let start_offset = boundaries[start_index];
        let mut low = start_index + 1;
        let mut high = boundaries.len() - 1;
        let mut best_end_index = low;
        let mut best_token_count =
            tokenizer.count_with_special_tokens(&text[start_offset..boundaries[best_end_index]]);

        while low <= high {
            let midpoint = low + (high - low) / 2;
            let end_offset = boundaries[midpoint];
            let token_count = tokenizer.count_with_special_tokens(&text[start_offset..end_offset]);
            if token_count <= MAX_TOKEN_CHUNK {
                best_end_index = midpoint;
                best_token_count = token_count;
                low = midpoint + 1;
            } else {
                high = midpoint - 1;
            }
        }

        let end_offset = boundaries[best_end_index];
        ranges.push(TokenChunkRange {
            start_offset,
            end_offset,
            token_count: best_token_count,
        });
        start_index = best_end_index;
    }

    ranges
}

#[cfg(test)]
#[path = "../../tests/core/text_chunking_tests.rs"]
mod tests;

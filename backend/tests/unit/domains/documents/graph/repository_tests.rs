use super::*;

fn chunk(id: &str, text: &str) -> FileChunkResult {
    FileChunkResult {
        chunk_id: id.into(),
        deal_id: "DEAL-SEARCH".into(),
        file_id: "file-search".into(),
        version_id: "version-search".into(),
        index_generation: "generation-search".into(),
        chunk_index: 1,
        text: text.into(),
        chunk_sha256: "0".repeat(64),
        token_count: 10,
        page_start: None,
        page_end: None,
        char_start: 0,
        char_end: text.len() as i64,
        section_path: "Architecture".into(),
        created_at: "2026-11-06T00:00:00Z".into(),
    }
}

#[test]
fn keyword_search_requires_every_word_instead_of_matching_common_substrings() {
    let candidates = vec![
        chunk(
            "unrelated",
            "Confidential information and security provisions.",
        ),
        chunk("rails-only", "The rails are maintained on site."),
        chunk("ruby-only", "The service uses Ruby."),
        chunk("framework", "The application uses Ruby on Rails."),
    ];
    let hits = rank_keywords(candidates.clone(), "Ruby on Rails", 30);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk.chunk_id, "framework");
    let hits = rank_keywords(candidates, "Ruby", 30);
    assert_eq!(hits.len(), 2);
    assert!(hits
        .iter()
        .all(|hit| ["ruby-only", "framework"].contains(&hit.chunk.chunk_id.as_str())));
}

#[test]
fn keyword_search_matches_case_and_punctuation_but_not_partial_words() {
    let hits = rank_keywords(
        vec![
            chunk("partial", "RubyGems and railroad infrastructure"),
            chunk("exact", "RUBY (on Rails), café."),
        ],
        "ruby, ON rails! CAFÉ",
        10,
    );
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk.chunk_id, "exact");
    assert!(rank_keywords(
        vec![chunk("substring", "Confidential provisions")],
        "on",
        10
    )
    .is_empty());
}

#[test]
fn keyword_search_keeps_programming_language_suffixes_distinct() {
    let candidates = vec![
        chunk("cpp", "C++ service"),
        chunk("csharp", "C# service"),
        chunk("c", "C service"),
    ];
    for (query, expected) in [("C++", "cpp"), ("C#", "csharp"), ("C", "c")] {
        let hits = rank_keywords(candidates.clone(), query, 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk.chunk_id, expected);
    }
}

#[test]
fn keyword_search_is_bounded_deterministic_and_ignores_duplicate_query_words() {
    let candidates = vec![chunk("b", "Ruby service"), chunk("a", "Ruby service")];
    let hits = rank_keywords(candidates.clone(), "ruby ruby", 1);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].chunk.chunk_id, "a");
    assert_eq!(hits, rank_keywords(candidates.clone(), "ruby", 1));
    assert!(rank_keywords(candidates.clone(), "... + #", 10).is_empty());
    assert!(rank_keywords(candidates, "ruby", 0).is_empty());
}

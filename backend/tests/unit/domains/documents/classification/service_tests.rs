use super::*;

#[test]
fn retry_delays_honor_bounded_hints_and_back_off_with_jitter() {
    let hinted = DecisionsError::RateLimited {
        retry_after: Some(Duration::from_secs(12)),
    };
    assert_eq!(retry_delay(hinted, 1), 12);
    for _ in 0..20 {
        let first = retry_delay(DecisionsError::Timeout, 1);
        let second = retry_delay(DecisionsError::Transport, 2);
        assert!(
            (2..=2).contains(&first) || (2..=3).contains(&first),
            "{first}"
        );
        assert!((4..=5).contains(&second), "{second}");
    }
    assert!(retry_delay(DecisionsError::Timeout, 50) <= 60);
}

#[test]
fn transport_errors_map_to_sanitized_codes() {
    assert_eq!(
        error_code(DecisionsError::Unauthorized),
        ErrorCode::Unauthorized
    );
    assert_eq!(
        error_code(DecisionsError::ResponseTooLarge),
        ErrorCode::InvalidResponse
    );
    assert_eq!(
        error_code(DecisionsError::Rejected { status: 422 }),
        ErrorCode::RequestRejected
    );
}

#[test]
fn ingestion_and_stored_metadata_produce_identical_evidence() {
    let chunks: Vec<DocumentChunk> = (1..=3)
        .map(|index| DocumentChunk {
            chunk_id: format!("c{index}"),
            document_id: "d".into(),
            deal_id: "DEAL".into(),
            text: format!("Chunk {index} discusses data pipelines."),
            embedding: None,
            sequence_number: index,
            page_numbers: Some(vec![index, index + 1]),
            start_offset: 0,
            end_offset: 10,
            token_count: 8,
            content_hash: crate::shared::ids::sha256_hex(index.to_string().as_bytes()),
            section_title: (index != 2).then(|| "Pipelines".to_string()),
        })
        .collect();
    let pages: Vec<_> = chunks.iter().map(page_range).collect();
    let live: Vec<ChunkMeta<'_>> = chunks
        .iter()
        .zip(&pages)
        .map(|(c, p)| ChunkMeta {
            index: i64::from(c.sequence_number),
            token_count: u64::from(c.token_count),
            sha256: &c.content_hash,
            section: c.section_title.as_deref(),
            pages: *p,
        })
        .collect();
    let stored: Vec<StoredChunk> = chunks
        .iter()
        .map(|c| StoredChunk {
            index: i64::from(c.sequence_number),
            token_count: u64::from(c.token_count),
            sha256: c.content_hash.clone(),
            section: c.section_title.clone(),
            pages: page_range(c),
        })
        .collect();
    let texts: HashMap<i64, &str> = chunks
        .iter()
        .map(|c| (i64::from(c.sequence_number), c.text.as_str()))
        .collect();
    let context = context("pipelines.docx", "docx");
    let a = evidence(context, &live, &texts, &"a".repeat(64));
    let b = evidence(context, &stored_meta(&stored), &texts, &"a".repeat(64));
    assert_eq!(a.sample, b.sample);
    assert_eq!(a.fingerprint, b.fingerprint);
    assert!(!a.empty);
}

#[test]
fn completed_result_encoding_enforces_the_persisted_size_bound() {
    let mut result = policy::insufficient(&Provenance {
        fingerprint: "a".repeat(64),
        sampled: false,
    });
    assert!(matches!(
        encode_completed(&result),
        Ok(ClassificationRun::Completed { .. })
    ));
    result.input_fingerprint = "a".repeat(MAX_RESULT_BYTES);
    assert_eq!(encode_completed(&result), Err(ErrorCode::InvalidResponse));
}

use super::*;
use crate::domains::notebooks::{model::*, test_support};

async fn admitted(
    repo: &NotebookRepository,
    book: &str,
    id: Option<String>,
    title: &str,
    expected: Option<i64>,
) -> ClaimedJob {
    let mut input = test_support::input(title);
    input.expected_revision = expected.map_or(PatchField::Missing, PatchField::Value);
    repo.admit(1, book.into(), id, input).await.unwrap();
    repo.claim().await.unwrap().unwrap()
}
fn prepared(claim: &ClaimedJob, count: usize) -> PreparedNote {
    let text = "Revenue retention café C++ API_v2";
    PreparedNote::new(
        &claim.input.note,
        claim.input.pipeline_version.clone(),
        1536,
        text.into(),
        (0..count).map(|_| text.into()).collect(),
        (0..count)
            .map(|_| CosineVector::new(vec![0.5; 1536], 1536).unwrap())
            .collect(),
    )
    .unwrap()
}
async fn setup() -> (NotebookRepository, String) {
    let repo = NotebookRepository::new(test_support::database().await);
    let book = repo
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    (repo, book)
}
#[tokio::test]
async fn notebook_bad_vectors_rejected_at_first_middle_last_without_native_publication() {
    let (repo, book) = setup().await;
    let claim = admitted(&repo, &book, None, "Unpublished", None).await;
    for bad in [0, 64, 129] {
        let mut vectors = vec![CosineVector::new(vec![0.5; 1536], 1536).unwrap(); 130];
        // Numerically valid vectors with a different profile dimension are still forbidden.
        vectors[bad] = CosineVector::new(vec![1.0; 2], 2).unwrap();
        assert!(PreparedNote::new(
            &claim.input.note,
            PIPELINE.into(),
            1536,
            "input".into(),
            vec!["text".into(); 130],
            vectors
        )
        .is_err());
    }
    assert!(repo.get_note(1, book, claim.job.note_id).await.is_err());
}
#[tokio::test]
async fn notebook_late_batch_and_deferred_failure_roll_back_native_artifact_and_job() {
    let (repo, book) = setup().await;
    let initial = admitted(&repo, &book, None, "Original", None).await;
    repo.commit_test(&initial).await.unwrap();
    let claim = admitted(
        &repo,
        &book,
        Some(initial.job.note_id.clone()),
        "Replacement",
        Some(1),
    )
    .await;
    // Fail after at least one successful 64-row batch, then at deferred complete-set validation.
    sqlx::raw_sql("CREATE FUNCTION reject_late_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.ordinal=65 THEN RAISE EXCEPTION 'injected'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_late_chunk BEFORE INSERT ON notebook_chunks FOR EACH ROW EXECUTE FUNCTION reject_late_chunk()")
        .execute(repo.postgres.pool()).await.unwrap();
    assert!(repo
        .persist(&claim, Some(prepared(&claim, 130)), None)
        .await
        .is_err());
    sqlx::raw_sql(
        "DROP TRIGGER reject_late_chunk ON notebook_chunks; DROP FUNCTION reject_late_chunk()",
    )
    .execute(repo.postgres.pool())
    .await
    .unwrap();
    let batch = prepared(&claim, 130);
    sqlx::raw_sql("CREATE FUNCTION alter_last_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.ordinal=129 THEN NEW.ordinal=150; END IF; RETURN NEW; END $$; CREATE TRIGGER alter_last_chunk BEFORE INSERT ON notebook_chunks FOR EACH ROW EXECUTE FUNCTION alter_last_chunk()")
        .execute(repo.postgres.pool()).await.unwrap();
    assert!(repo.persist(&claim, Some(batch), None).await.is_err());
    let old = repo
        .get_note(1, book.clone(), initial.job.note_id)
        .await
        .unwrap();
    assert_eq!(old.summary.title, "Original");
    assert_eq!(old.summary.revision, 1);
    let pending = repo.job(1, book, claim.job.job_id).await.unwrap();
    assert!(!pending.sqlite_committed);
    assert_eq!(pending.status, "processing");
    let sets: i64 = sqlx::query_scalar("SELECT count(*) FROM notebook_search_sets")
        .fetch_one(repo.postgres.pool())
        .await
        .unwrap();
    assert_eq!(sets, 1);
}
#[tokio::test]
async fn notebook_folder_only_save_reuses_set_and_current_revision_search_join() {
    let (repo, book) = setup().await;
    let original = admitted(&repo, &book, None, "Revenue", None).await;
    repo.commit_test(&original).await.unwrap();
    let folder = repo
        .create_folder(1, book.clone(), "Folder".into())
        .await
        .unwrap();
    let mut input:SaveInput=serde_json::from_value(serde_json::json!({"operationId":uuid::Uuid::new_v4().to_string(),"expectedRevision":1,"folderId":folder.folder_id})).unwrap();
    let job = repo
        .admit(
            1,
            book.clone(),
            Some(original.job.note_id.clone()),
            input.clone(),
        )
        .await
        .unwrap();
    let claim = repo.claim().await.unwrap().unwrap();
    let reuse = repo
        .reusable(&claim.input.note, PIPELINE)
        .await
        .unwrap()
        .unwrap();
    repo.persist(&claim, None, Some(reuse.clone()))
        .await
        .unwrap();
    let completed = repo.job(1, book.clone(), job.job_id.clone()).await.unwrap();
    assert_eq!(completed.committed_revision, Some(2));
    let replay = repo
        .admit(
            1,
            book.clone(),
            Some(original.job.note_id.clone()),
            input.clone(),
        )
        .await
        .unwrap();
    assert_eq!(replay.job_id, job.job_id);
    assert_eq!(replay.committed_revision, Some(2));
    // Folder relocation advances revision but preserves the immutable set.
    repo.delete_folder(1, book.clone(), folder.folder_id)
        .await
        .unwrap();
    let result = repo
        .search(
            1,
            book.clone(),
            PIPELINE.into(),
            SearchInput {
                query_text: "revenue".into(),
                mode: SearchMode::Hybrid,
                limit: 5,
            },
            Some(CosineVector::new(vec![0.5; 1536], 1536).unwrap()),
        )
        .await
        .unwrap();
    assert_eq!(result.hits[0].revision, 3);
    assert_eq!(result.hits[0].set_id, reuse);
    assert_eq!(result.hits[0].match_sources, vec!["keyword", "semantic"]);
    input.operation_id = uuid::Uuid::new_v4().to_string();
    input.expected_revision = PatchField::Value(3);
    input.title = PatchField::Value("Different".into());
    input.folder_id = PatchField::Null;
    repo.admit(1, book, Some(original.job.note_id), input)
        .await
        .unwrap();
    let changed = repo.claim().await.unwrap().unwrap();
    assert!(repo
        .reusable(&changed.input.note, PIPELINE)
        .await
        .unwrap()
        .is_none());
}
#[tokio::test]
async fn notebook_expired_attempt_and_deleted_parent_cannot_publish_or_release_successor_slot() {
    let (repo, book) = setup().await;
    let old = admitted(&repo, &book, None, "Late", None).await;
    sqlx::query("UPDATE notebook_save_jobs SET lease_until=0 WHERE job_id=$1")
        .bind(&old.job.job_id)
        .execute(repo.postgres.pool())
        .await
        .unwrap();
    let successor = repo.claim().await.unwrap().unwrap();
    assert_ne!(old.token, successor.token);
    assert!(repo
        .persist(&old, Some(prepared(&old, 1)), None)
        .await
        .is_err());
    repo.fail(&old, "embedding_failed").await.unwrap();
    let occupied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notebook_save_slots WHERE job_id IS NOT NULL")
            .fetch_one(repo.postgres.pool())
            .await
            .unwrap();
    assert_eq!(occupied, 1);
    repo.delete(1, book.clone()).await.unwrap();
    assert!(repo
        .persist(&successor, Some(prepared(&successor, 1)), None)
        .await
        .is_err());
    let occupied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notebook_save_slots WHERE job_id IS NOT NULL")
            .fetch_one(repo.postgres.pool())
            .await
            .unwrap();
    assert_eq!(occupied, 0);
}

#[tokio::test]
async fn notebook_independent_pools_bound_user_and_global_admission_and_replay() {
    use crate::adapters::postgres::client::PostgresClient;
    let (a, b) = PostgresClient::concurrent_test_clients().await;
    for index in 0..9u8 {
        crate::integration_support::seed_identity(
            &a,
            &format!("capacity-{index}@example.invalid"),
            char::from(b'a' + index),
        )
        .await;
    }
    let ra = NotebookRepository::new(a.clone());
    let rb = NotebookRepository::new(b.clone());
    let mut books = Vec::new();
    for owner in 1..=9 {
        books.push(
            ra.create(owner, "Book".into(), None)
                .await
                .unwrap()
                .notebook_id,
        );
    }
    let original = test_support::input("Replay");
    let first = ra
        .admit(1, books[0].clone(), None, original.clone())
        .await
        .unwrap();
    // Race different connections with the production global guard disabled only in this schema.
    for _ in 0..16 {
        let (left, right) = tokio::join!(
            ra.admit(1, books[0].clone(), None, test_support::input("A")),
            rb.admit(1, books[0].clone(), None, test_support::input("B"))
        );
        assert!(left.is_ok() || right.is_ok());
    }
    assert!(ra
        .admit(1, books[0].clone(), None, test_support::input("overflow"))
        .await
        .is_err());
    for owner in 2..=8i64 {
        let book = &books[usize::try_from(owner - 1).unwrap()];
        for _ in 0..16 {
            let (left, right) = tokio::join!(
                ra.admit(owner, book.clone(), None, test_support::input("A")),
                rb.admit(owner, book.clone(), None, test_support::input("B"))
            );
            left.unwrap();
            right.unwrap();
        }
    }
    assert!(matches!(
        rb.admit(
            9,
            books[8].clone(),
            None,
            test_support::input("global overflow")
        )
        .await,
        Err(Error::Capacity)
    ));
    assert_eq!(
        rb.admit(1, books[0].clone(), None, original)
            .await
            .unwrap()
            .job_id,
        first.job_id
    );
    let occupied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM notebook_save_slots WHERE job_id IS NOT NULL")
            .fetch_one(a.pool())
            .await
            .unwrap();
    assert_eq!(occupied, 256);
    // Parent cascade and another user's admission may race without taking the admission row backwards.
    let (deleted, admitted) = tokio::join!(
        ra.delete(1, books[0].clone()),
        rb.admit(
            9,
            books[8].clone(),
            None,
            test_support::input("racing cascade")
        )
    );
    deleted.unwrap();
    if admitted.is_err() {
        rb.admit(
            9,
            books[8].clone(),
            None,
            test_support::input("after cascade"),
        )
        .await
        .unwrap();
    }
    assert!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM notebook_save_slots WHERE job_id IS NOT NULL"
        )
        .fetch_one(a.pool())
        .await
        .unwrap()
            <= 256
    );
    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn notebook_lexical_unicode_phrase_technical_prefix_and_cosine_are_scoped_in_sql() {
    let (repo, book) = setup().await;
    let claim = admitted(
        &repo,
        &book,
        None,
        "C++ API_v2 café revenue retention",
        None,
    )
    .await;
    repo.commit_test(&claim).await.unwrap();
    for query in ["C++", "API_v2", "café", "\"revenue retention\"", "reven*"] {
        let result = repo
            .search(
                1,
                book.clone(),
                PIPELINE.into(),
                SearchInput {
                    query_text: query.into(),
                    mode: SearchMode::Keyword,
                    limit: 5,
                },
                None,
            )
            .await
            .unwrap();
        assert_eq!(result.hits.len(), 1, "{query}");
    }
    let profile = sha256_hex(PIPELINE.as_bytes());
    let vector = CosineVector::new(vec![0.5; 1536], 1536).unwrap();
    let distance: f64 = repo
        .postgres
        .read_one_with_async(
            ranked(1, &book, &profile)
                .expression(
                    E::CosineDistance {
                        column: "notebook_chunks.embedding".into(),
                        vector,
                    },
                    "score",
                )
                .limit(1)
                .build(),
            |r| r.get("score"),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(distance.abs() < 1e-6);
    let plan:Vec<String>=sqlx::query_scalar("EXPLAIN SELECT * FROM notebook_chunks WHERE search_vector @@ plainto_tsquery('english','revenue')").fetch_all(repo.postgres.pool()).await.unwrap();
    assert!(!plan.is_empty()); // Small tables may correctly choose a sequential scan.
    let index:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid JOIN pg_am a ON a.oid=c.relam WHERE c.relnamespace=current_schema()::regnamespace AND c.relname='idx_notebook_chunks_lexical' AND a.amname='gin' AND i.indisvalid)").fetch_one(repo.postgres.pool()).await.unwrap();
    assert!(index);
}

#[tokio::test]
async fn notebook_technical_phrases_preserve_positions_and_english_stemming() {
    let (repo, book) = setup().await;
    let claim = admitted(&repo, &book, None, "C++ guides API_v2", None).await;
    repo.commit_test(&claim).await.unwrap();
    let mut input = test_support::input("Body phrases");
    input.blocks = PatchField::Value(serde_json::json!([{
        "id": "paragraph", "type": "paragraph", "props": {}, "children": [],
        "content": [{"type":"text", "text":"C# reference API_v3. C++ unrelated API_v3.", "styles":{}}]
    }]));
    repo.admit(1, book.clone(), None, input).await.unwrap();
    let body = repo.claim().await.unwrap().unwrap();
    repo.commit_test(&body).await.unwrap();
    let pair = admitted(&repo, &book, None, "C++ API_v4", None).await;
    repo.commit_test(&pair).await.unwrap();
    // Escaping can split one technical title token across Markdown chunks. The title's
    // positional projection must still be searchable without a chunk-level token filter.
    let long_title = format!("a{}", "_".repeat(299));
    let long = admitted(&repo, &book, None, &long_title, None).await;
    repo.commit_test(&long).await.unwrap();
    let long_query = format!("\"{long_title}\"");
    for (query, matches) in [
        ("\"C++ guide API_v2\"", true),
        ("\"API_v2 guide C++\"", false),
        ("\"C++ API_v2\"", false),
        ("\"C# guide API_v2\"", false),
        ("C++ API_v2", true),
        ("\"C# reference API_v3\"", true),
        ("\"C++ reference API_v3\"", false),
        ("\"C++ API_v4\"", true),
        ("\"API_v4 C++\"", false),
        (long_query.as_str(), true),
    ] {
        let result = repo
            .search(
                1,
                book.clone(),
                PIPELINE.into(),
                SearchInput {
                    query_text: query.into(),
                    mode: SearchMode::Keyword,
                    limit: 5,
                },
                None,
            )
            .await
            .unwrap();
        assert_eq!(!result.hits.is_empty(), matches, "{query}");
    }
    let index: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid JOIN pg_am a ON a.oid=c.relam WHERE c.relnamespace=current_schema()::regnamespace AND c.relname='idx_notebook_chunks_phrase' AND a.amname='gin' AND i.indisvalid)")
        .fetch_one(repo.postgres.pool()).await.unwrap();
    assert!(index);
}

#[tokio::test]
async fn notebook_unicode_prefix_accepts_combining_marks() {
    let (repo, book) = setup().await;
    let claim = admitted(&repo, &book, None, "cafe\u{301} revenue", None).await;
    repo.commit_test(&claim).await.unwrap();
    let result = repo
        .search(
            1,
            book,
            PIPELINE.into(),
            SearchInput {
                query_text: "cafe\u{301}*".into(),
                mode: SearchMode::Keyword,
                limit: 5,
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.hits.len(), 1);
}

#[tokio::test]
async fn notebook_old_read_snapshot_retains_complete_set_after_atomic_replacement() {
    let (repo, book) = setup().await;
    let old = admitted(&repo, &book, None, "Original", None).await;
    repo.commit_test(&old).await.unwrap();
    let mut snapshot = repo.postgres.pool().begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *snapshot)
        .await
        .unwrap();
    let before:(i64,String,i64)=sqlx::query_as("SELECT n.revision,n.title,count(c.ordinal) FROM notebook_notes n JOIN notebook_chunks c ON c.set_id=n.active_set_id GROUP BY n.note_id").fetch_one(&mut *snapshot).await.unwrap();
    let new = admitted(&repo, &book, Some(old.job.note_id), "Replacement", Some(1)).await;
    repo.commit_test(&new).await.unwrap();
    let retained:(i64,String,i64)=sqlx::query_as("SELECT n.revision,n.title,count(c.ordinal) FROM notebook_notes n JOIN notebook_chunks c ON c.set_id=n.active_set_id GROUP BY n.note_id").fetch_one(&mut *snapshot).await.unwrap();
    assert_eq!(before, retained);
    snapshot.commit().await.unwrap();
    let current:(i64,String,i64)=sqlx::query_as("SELECT n.revision,n.title,count(c.ordinal) FROM notebook_notes n JOIN notebook_chunks c ON c.set_id=n.active_set_id GROUP BY n.note_id").fetch_one(repo.postgres.pool()).await.unwrap();
    assert_eq!(current, (2, "Replacement".into(), 1));
}

#[tokio::test]
async fn notebook_final_fence_loss_after_chunk_insertion_rolls_back_creation() {
    let (repo, book) = setup().await;
    let claim = admitted(&repo, &book, None, "Unpublished", None).await;
    sqlx::raw_sql("CREATE FUNCTION steal_attempt() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN UPDATE notebook_save_jobs SET lease_token='superseded' WHERE status='processing'; RETURN NEW; END $$; CREATE TRIGGER steal_attempt AFTER INSERT ON notebook_chunks FOR EACH ROW EXECUTE FUNCTION steal_attempt()")
        .execute(repo.postgres.pool()).await.unwrap();
    assert!(repo
        .persist(&claim, Some(prepared(&claim, 130)), None)
        .await
        .is_err());
    assert!(repo
        .get_note(1, book.clone(), claim.job.note_id)
        .await
        .is_err());
    assert!(
        !repo
            .job(1, book, claim.job.job_id)
            .await
            .unwrap()
            .sqlite_committed
    );
    let sets: i64 = sqlx::query_scalar("SELECT count(*) FROM notebook_search_sets")
        .fetch_one(repo.postgres.pool())
        .await
        .unwrap();
    assert_eq!(sets, 0);
}

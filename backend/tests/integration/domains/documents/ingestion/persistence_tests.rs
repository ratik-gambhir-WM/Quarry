use super::*;
use crate::domains::documents::test_support::*;
#[tokio::test]
async fn failure_in_first_middle_and_final_batch_rolls_back_complete_document() {
    for ordinal in [1, 130, 260] {
        let db = database().await;
        save(&db, "file", "DEAL-SQL", "previous", 2).await;
        sqlx::raw_sql(&format!("CREATE FUNCTION reject_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.char_start={} THEN RAISE EXCEPTION 'injected chunk failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_chunk BEFORE INSERT ON document_chunks FOR EACH ROW EXECUTE FUNCTION reject_chunk();",ordinal*100)).execute(db.pool()).await.unwrap();
        let (doc, chunks, bytes) = sample("file", "DEAL-SQL", "replacement", 260);
        assert!(persist_document_and_chunks(
            &DocumentStore::new(db.clone()),
            "DEAL-SQL",
            1,
            doc,
            chunks,
            bytes,
            None
        )
        .await
        .is_err());
        let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM quarry_file_versions),(SELECT count(*) FROM document_chunks),(SELECT count(*) FROM deal_activity_events WHERE kind='document_uploaded')").fetch_one(db.pool()).await.unwrap();
        assert_eq!(counts, (1, 2, 1));
        let (hits, _) = search(db.clone())
            .branches(1, "DEAL-SQL", Some("replacement"), None, 50)
            .await
            .unwrap();
        assert!(hits.is_empty());
        let (hits, _) = search(db)
            .branches(1, "DEAL-SQL", Some("previous"), None, 50)
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
    }
}
#[tokio::test]
async fn deferred_commit_failure_leaves_no_canonical_document() {
    let db = database().await;
    sqlx::raw_sql("CREATE FUNCTION reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected deferred failure'; END $$; CREATE CONSTRAINT TRIGGER reject_commit AFTER INSERT ON quarry_file_versions DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION reject_commit();").execute(db.pool()).await.unwrap();
    let (doc, chunks, bytes) = sample("file", "DEAL-SQL", "new", 3);
    assert!(persist_document_and_chunks(
        &DocumentStore::new(db.clone()),
        "DEAL-SQL",
        1,
        doc,
        chunks,
        bytes,
        None
    )
    .await
    .is_err());
    for table in [
        "quarry_files",
        "quarry_file_versions",
        "quarry_file_blobs",
        "document_identity_manifests",
        "document_chunk_identities",
        "document_chunks",
        "deal_activity_events",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
}
#[tokio::test]
async fn exact_retry_preserves_chunk_ids_activity_and_attribution() {
    let db = database().await;
    save(&db, "file", "DEAL-SQL", "unchanged", 3).await;
    let before: Vec<String> =
        sqlx::query_scalar("SELECT chunk_id FROM document_chunk_identities ORDER BY chunk_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    save(&db, "file", "DEAL-SQL", "unchanged", 3).await;
    let after: Vec<String> =
        sqlx::query_scalar("SELECT chunk_id FROM document_chunk_identities ORDER BY chunk_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(before, after);
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM deal_activity_events WHERE kind='document_uploaded'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(events, 1);
}
#[tokio::test]
async fn invalid_vectors_and_ordinal_gaps_reject_before_canonical_writes() {
    let db = database().await;
    for invalid in [0, 1, 2] {
        let (doc, mut chunks, bytes) = sample("file", "DEAL-SQL", "new", 3);
        match invalid {
            0 => chunks[1].embedding = Some(vec![0.0; 1536]),
            1 => chunks[1].embedding = Some(vec![1.0; 2]),
            _ => chunks[1].sequence_number = 5,
        }
        assert!(persist_document_and_chunks(
            &DocumentStore::new(db.clone()),
            "DEAL-SQL",
            1,
            doc,
            chunks,
            bytes,
            None
        )
        .await
        .is_err());
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM quarry_files")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn readers_see_only_complete_versions_and_cancelled_batches_never_publish() {
    for cancel in [false, true] {
        let db = database().await;
        save(&db, "file", "DEAL-SQL", "previous", 2).await;
        let key = i64::from(u32::from_be_bytes(
            uuid::Uuid::new_v4().as_bytes()[..4].try_into().unwrap(),
        ));
        let mut blocker = db.pool().begin().await.unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(&mut *blocker)
            .await
            .unwrap();
        sqlx::raw_sql(&format!("CREATE FUNCTION pause_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.char_start=13000 THEN PERFORM pg_advisory_xact_lock({key}); END IF; RETURN NEW; END $$; CREATE TRIGGER pause_chunk BEFORE INSERT ON document_chunks FOR EACH ROW EXECUTE FUNCTION pause_chunk();")).execute(db.pool()).await.unwrap();
        let writer = db.clone();
        let task = tokio::spawn(async move {
            let (doc, chunks, bytes) = sample("file", "DEAL-SQL", "replacement", 260);
            persist_document_and_chunks(
                &DocumentStore::new(writer),
                "DEAL-SQL",
                1,
                doc,
                chunks,
                bytes,
                None,
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5),async{loop{
   let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND classid=0 AND objid=$1::bigint::oid AND NOT granted)").bind(key).fetch_one(db.pool()).await.unwrap();
   if waiting{break;}tokio::time::sleep(std::time::Duration::from_millis(10)).await;
  }}).await.unwrap();
        let repo = search(db.clone());
        assert_eq!(
            repo.branches(1, "DEAL-SQL", Some("previous"), None, 100)
                .await
                .unwrap()
                .0
                .len(),
            2
        );
        assert!(repo
            .branches(1, "DEAL-SQL", Some("replacement"), None, 100)
            .await
            .unwrap()
            .0
            .is_empty());
        if cancel {
            task.abort();
        }
        blocker.rollback().await.unwrap();
        if cancel {
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            task.await.unwrap().unwrap();
        }
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM document_chunks c JOIN document_chunk_identities i USING(chunk_id) JOIN quarry_file_versions v USING(version_id) WHERE v.is_current=1").fetch_one(db.pool()).await.unwrap();
        assert_eq!(count, if cancel { 2 } else { 260 });
    }
}

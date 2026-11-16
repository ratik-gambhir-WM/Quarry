use super::*;
use crate::domains::documents::{
    ingestion::persistence::persist_document_and_chunks, store::repository::DocumentStore,
    test_support::*,
};
fn repository(db: PostgresClient) -> DocumentJobRepository {
    DocumentJobRepository {
        db,
        profile: EmbeddingProfile::new("text-embedding-3-small".into(), 1536),
    }
}
fn input(content: &str) -> UploadedDocument {
    UploadedDocument {
        filename: "evidence.docx".into(),
        bytes: content.as_bytes().to_vec(),
    }
}
#[tokio::test]
async fn lost_admission_replays_one_job_and_exact_content_has_one_logical_winner() {
    let db = database().await;
    let jobs = repository(db.clone());
    let (id, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("same"), true)
        .await
        .unwrap();
    let (replay, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("same"), true)
        .await
        .unwrap();
    assert_eq!(id, replay);
    sqlx::query("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('DEAL-SQL',2,'contributor','now','now')").execute(db.pool()).await.unwrap();
    let (other, _) = jobs
        .admit("DEAL-SQL".into(), 2, input("same"), true)
        .await
        .unwrap();
    assert_ne!(id, other);
    let identities: Vec<String> =
        sqlx::query_scalar("SELECT file_id FROM document_jobs ORDER BY actor_id")
            .fetch_all(db.pool())
            .await
            .unwrap();
    assert_eq!(identities.len(), 2);
    assert_eq!(identities[0], identities[1]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM quarry_files")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn expired_attempt_cannot_publish_or_overwrite_a_recovered_committed_result() {
    let db = database().await;
    let jobs = repository(db.clone());
    let (id, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("new"), true)
        .await
        .unwrap();
    let first = jobs.claim(Some(&id)).await.unwrap().unwrap();
    sqlx::query("UPDATE document_jobs SET lease_until=0 WHERE job_id=$1")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();
    let second = jobs.claim(Some(&id)).await.unwrap().unwrap();
    assert_ne!(first.attempt.token, second.attempt.token);
    for (attempt, success) in [
        (first.attempt.clone(), false),
        (second.attempt.clone(), true),
    ] {
        let (doc, chunks, bytes) = sample(&attempt.file_id, "DEAL-SQL", "new", 3);
        let result = persist_document_and_chunks(
            &DocumentStore::new(db.clone()),
            "DEAL-SQL",
            1,
            doc,
            chunks,
            bytes,
            Some(attempt),
        )
        .await;
        assert_eq!(result.is_ok(), success);
    }
    jobs.fail(&first.attempt).await.unwrap();
    let state = jobs.job(&id).await.unwrap();
    assert_eq!(state.status, "completed");
    assert!(state.result.unwrap().sqlite_committed);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM quarry_files")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    let (replay, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("new"), true)
        .await
        .unwrap();
    assert_eq!(id, replay);
    assert!(jobs.claim(Some(&id)).await.unwrap().is_none());
}
#[tokio::test]
async fn archive_or_delete_after_preparation_prevents_publication_and_resurrection() {
    for deleted in [false, true] {
        let db = database().await;
        let jobs = repository(db.clone());
        let (id, _) = jobs
            .admit("DEAL-SQL".into(), 1, input("new"), true)
            .await
            .unwrap();
        let claim = jobs.claim(Some(&id)).await.unwrap().unwrap();
        if deleted {
            sqlx::query("DELETE FROM deals WHERE deal_id='DEAL-SQL'")
                .execute(db.pool())
                .await
                .unwrap();
        } else {
            sqlx::query("UPDATE deals SET archived_at='now' WHERE deal_id='DEAL-SQL'")
                .execute(db.pool())
                .await
                .unwrap();
        }
        let (doc, chunks, bytes) = sample(&claim.attempt.file_id, "DEAL-SQL", "new", 3);
        assert!(persist_document_and_chunks(
            &DocumentStore::new(db.clone()),
            "DEAL-SQL",
            1,
            doc,
            chunks,
            bytes,
            Some(claim.attempt)
        )
        .await
        .is_err());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM quarry_files")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            0
        );
    }
}
#[tokio::test]
async fn failed_jobs_keep_immutable_accepted_image_text_for_explicit_retry() {
    let db = database().await;
    let jobs = repository(db.clone());
    let (id, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("image"), false)
        .await
        .unwrap();
    let claim = jobs.claim(Some(&id)).await.unwrap().unwrap();
    let (document, chunks, _) = sample(&claim.attempt.file_id, "DEAL-SQL", "image", 1);
    let assembly = crate::domains::documents::formats::image::ImageAssembly { document, chunks };
    let json = serde_json::to_string(&assembly).unwrap();
    jobs.checkpoint_image(&claim.attempt, json.clone())
        .await
        .unwrap();
    assert!(jobs
        .checkpoint_image(&claim.attempt, "{}".into())
        .await
        .is_err());
    jobs.fail(&claim.attempt).await.unwrap();
    let sequence = jobs.job(&id).await.unwrap().sequence;
    let (retry, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("image"), false)
        .await
        .unwrap();
    assert_eq!(id, retry);
    let recovered = jobs.claim(Some(&id)).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_string(&recovered.image.unwrap()).unwrap(),
        json
    );
    assert!(jobs.job(&id).await.unwrap().sequence > sequence);
}
#[tokio::test]
async fn actor_queue_capacity_is_bounded_and_terminal_work_releases_admission() {
    let jobs = repository(database().await);
    let mut ids = Vec::new();
    for n in 0..8 {
        ids.push(
            jobs.admit("DEAL-SQL".into(), 1, input(&format!("input-{n}")), true)
                .await
                .unwrap()
                .0,
        );
    }
    assert!(matches!(
        jobs.admit("DEAL-SQL".into(), 1, input("excess"), true)
            .await,
        Err(RepositoryError::Capacity)
    ));
    let claim = jobs.claim(Some(&ids[0])).await.unwrap().unwrap();
    jobs.fail(&claim.attempt).await.unwrap();
    assert!(jobs
        .admit("DEAL-SQL".into(), 1, input("excess"), true)
        .await
        .is_ok());
}

#[tokio::test]
async fn a_batch_that_outlives_its_attempt_fails_the_final_database_clock_fence() {
    let db = database().await;
    let jobs = repository(db.clone());
    let (id, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("new"), false)
        .await
        .unwrap();
    let claim = jobs.claim(Some(&id)).await.unwrap().unwrap();
    sqlx::query("UPDATE document_jobs SET lease_until=floor(extract(epoch FROM clock_timestamp()))::bigint+2 WHERE job_id=$1").bind(&id).execute(db.pool()).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION slow_final_chunk() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.char_start=300 THEN PERFORM pg_sleep(2.1); END IF; RETURN NEW; END $$; CREATE TRIGGER slow_final_chunk BEFORE INSERT ON document_chunks FOR EACH ROW EXECUTE FUNCTION slow_final_chunk();").execute(db.pool()).await.unwrap();
    let (doc, chunks, bytes) = sample(&claim.attempt.file_id, "DEAL-SQL", "new", 3);
    assert!(persist_document_and_chunks(
        &DocumentStore::new(db.clone()),
        "DEAL-SQL",
        1,
        doc,
        chunks,
        bytes,
        Some(claim.attempt)
    )
    .await
    .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM quarry_files")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(jobs.job(&id).await.unwrap().status, "processing");
}

#[tokio::test]
async fn independent_pools_recover_input_and_replay_committed_result_after_restart() {
    let path = format!("document-restart-{}", uuid::Uuid::new_v4());
    let db = database_at(&path).await;
    let jobs = repository(db.clone());
    let (id, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("restart"), true)
        .await
        .unwrap();
    let first = jobs.claim(Some(&id)).await.unwrap().unwrap();
    let (document, chunks, _) = sample(&first.attempt.file_id, "DEAL-SQL", "restart", 1);
    let image = crate::domains::documents::formats::image::ImageAssembly { document, chunks };
    jobs.checkpoint_image(&first.attempt, serde_json::to_string(&image).unwrap())
        .await
        .unwrap();
    drop(jobs);
    db.close().await;
    let reopened = PostgresClient::test_database_at(&path).await.unwrap();
    crate::app::migrations::migrate(&reopened).await.unwrap();
    sqlx::query("UPDATE document_jobs SET lease_until=0 WHERE job_id=$1")
        .bind(&id)
        .execute(reopened.pool())
        .await
        .unwrap();
    let jobs = repository(reopened.clone());
    let claim = jobs.claim(None).await.unwrap().unwrap();
    assert_eq!(claim.file.bytes, b"restart");
    assert!(claim.image.is_some());
    assert_ne!(claim.attempt.token, first.attempt.token);
    let (doc, chunks, bytes) = sample(&claim.attempt.file_id, "DEAL-SQL", "restart", 1);
    persist_document_and_chunks(
        &DocumentStore::new(reopened.clone()),
        "DEAL-SQL",
        1,
        doc,
        chunks,
        bytes,
        Some(claim.attempt),
    )
    .await
    .unwrap();
    let third = repository(PostgresClient::test_database_at(&path).await.unwrap());
    assert!(
        third
            .job(&id)
            .await
            .unwrap()
            .result
            .unwrap()
            .sqlite_committed
    );
    assert!(third.claim(None).await.unwrap().is_none());
}

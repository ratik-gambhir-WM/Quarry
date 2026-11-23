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
async fn cancelled_sync_requests_expire_release_capacity_and_retry_with_fenced_attempts() {
    let db = database().await;
    let jobs = repository(db.clone());
    let mut abandoned = Vec::new();
    for n in 0..8 {
        let request_jobs = jobs.clone();
        let (accepted, received) = tokio::sync::oneshot::channel();
        let request = tokio::spawn(async move {
            let (id, _) = request_jobs
                .admit(
                    "DEAL-SQL".into(),
                    1,
                    input(&format!("cancelled-{n}")),
                    false,
                )
                .await
                .unwrap();
            let attempt = if n % 2 == 0 {
                Some(
                    request_jobs
                        .claim(Some(&id))
                        .await
                        .unwrap()
                        .unwrap()
                        .attempt,
                )
            } else {
                None
            };
            accepted
                .send((id, attempt))
                .unwrap_or_else(|_| panic!("request receiver dropped"));
            std::future::pending::<()>().await;
        });
        abandoned.push(received.await.unwrap());
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
    }
    assert!(matches!(
        jobs.admit("DEAL-SQL".into(), 1, input("new input"), true)
            .await,
        Err(RepositoryError::Capacity)
    ));
    sqlx::query("UPDATE document_jobs SET updated_at=0, lease_until=CASE WHEN status='processing' THEN 0 ELSE NULL END")
        .execute(db.pool()).await.unwrap();
    jobs.prune(600).await.unwrap();
    jobs.prune(600).await.unwrap();
    for (id, attempt) in &abandoned {
        let job = jobs.job(id).await.unwrap();
        assert_eq!(job.status, "failed");
        assert_eq!(job.sequence, if attempt.is_some() { 3 } else { 2 });
        let result = job.result.unwrap();
        assert!(!result.success);
        assert!(!result.sqlite_committed);
        if let Some(attempt) = attempt {
            assert!(db
                .transaction_result_async(|tx| async move { attempt.lock_and_check(&tx).await })
                .await
                .is_err());
        }
    }
    jobs.admit("DEAL-SQL".into(), 1, input("new input"), true)
        .await
        .unwrap();
    let (retry, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("cancelled-0"), false)
        .await
        .unwrap();
    assert_eq!(retry, abandoned[0].0);
    let recovered = jobs.claim(Some(&retry)).await.unwrap().unwrap();
    assert_eq!(recovered.file.bytes, b"cancelled-0");
    let original = abandoned.remove(0).1.unwrap();
    assert_ne!(recovered.attempt.token, original.token);
    jobs.fail(&original).await.unwrap();
    assert_eq!(jobs.job(&retry).await.unwrap().status, "processing");
    let (doc, chunks, bytes) = sample(&recovered.attempt.file_id, "DEAL-SQL", "cancelled-0", 1);
    persist_document_and_chunks(
        &DocumentStore::new(db),
        "DEAL-SQL",
        1,
        doc,
        chunks,
        bytes,
        crate::domains::documents::ingestion::persistence::CommitContext {
            attempt: Some(recovered.attempt),
            classification: None,
        },
    )
    .await
    .unwrap();
    assert!(jobs.job(&retry).await.unwrap().result.unwrap().success);
}

#[tokio::test]
async fn expiration_preserves_live_work_but_requires_retry_for_incomplete_admissions() {
    let db = database().await;
    let jobs = repository(db.clone());
    let mut preserved = Vec::new();
    for (content, background, processing) in [
        ("live queued", false, false),
        ("live processing", false, true),
        ("background queued", true, false),
        ("background processing", true, true),
        ("promoted", false, true),
    ] {
        let (id, _) = jobs
            .admit("DEAL-SQL".into(), 1, input(content), background)
            .await
            .unwrap();
        if processing {
            jobs.claim(Some(&id)).await.unwrap().unwrap();
        }
        if background || content == "promoted" {
            sqlx::query("UPDATE document_jobs SET updated_at=0, lease_until=CASE WHEN status='processing' THEN 0 ELSE NULL END WHERE job_id=$1")
                .bind(&id).execute(db.pool()).await.unwrap();
        }
        if content == "promoted" {
            jobs.admit("DEAL-SQL".into(), 1, input(content), true)
                .await
                .unwrap();
        }
        preserved.push((
            id,
            if background || content == "promoted" {
                "failed"
            } else if processing {
                "processing"
            } else {
                "queued"
            },
        ));
    }
    let (id, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("image checkpoint"), false)
        .await
        .unwrap();
    let claim = jobs.claim(Some(&id)).await.unwrap().unwrap();
    let (document, chunks, _) = sample(&claim.attempt.file_id, "DEAL-SQL", "image checkpoint", 1);
    let assembly = crate::domains::documents::formats::image::ImageAssembly { document, chunks };
    let json = serde_json::to_string(&assembly).unwrap();
    jobs.checkpoint_image(&claim.attempt, json.clone())
        .await
        .unwrap();
    sqlx::query("UPDATE document_jobs SET lease_until=0 WHERE job_id=$1")
        .bind(&id)
        .execute(db.pool())
        .await
        .unwrap();
    jobs.prune(600).await.unwrap();
    for (id, status) in preserved {
        assert_eq!(jobs.job(&id).await.unwrap().status, status);
    }
    assert_eq!(jobs.job(&id).await.unwrap().status, "failed");
    jobs.admit("DEAL-SQL".into(), 1, input("image checkpoint"), false)
        .await
        .unwrap();
    let retry = jobs.claim(Some(&id)).await.unwrap().unwrap();
    assert_eq!(serde_json::to_string(&retry.image.unwrap()).unwrap(), json);
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
            crate::domains::documents::ingestion::persistence::CommitContext {
                attempt: Some(attempt),
                classification: None,
            },
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
            crate::domains::documents::ingestion::persistence::CommitContext {
                attempt: Some(claim.attempt),
                classification: None
            },
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
        crate::domains::documents::ingestion::persistence::CommitContext {
            attempt: Some(claim.attempt),
            classification: None
        },
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
    crate::app::migrations::migrate(&reopened, Default::default())
        .await
        .unwrap();
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
        crate::domains::documents::ingestion::persistence::CommitContext {
            attempt: Some(claim.attempt),
            classification: None,
        },
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

#[tokio::test]
async fn asynchronous_retry_promotes_sync_work_without_replacing_a_live_attempt() {
    for processing in [false, true] {
        let db = database().await;
        let jobs = repository(db.clone());
        let (id, filename) = jobs
            .admit("DEAL-SQL".into(), 1, input("abandoned"), false)
            .await
            .unwrap();
        let original = if processing {
            Some(jobs.claim(Some(&id)).await.unwrap().unwrap().attempt)
        } else {
            None
        };
        let (retry, accepted_filename) = jobs
            .admit("DEAL-SQL".into(), 1, input("abandoned"), true)
            .await
            .unwrap();
        assert_eq!(retry, id);
        assert_eq!(accepted_filename, filename);
        if let Some(original) = &original {
            // Promotion schedules recovery; it must not steal a live worker's lease.
            assert!(jobs.claim(None).await.unwrap().is_none());
            let token: String =
                sqlx::query_scalar("SELECT attempt_token FROM document_jobs WHERE job_id=$1")
                    .bind(&id)
                    .fetch_one(db.pool())
                    .await
                    .unwrap();
            assert_eq!(token, original.token);
            sqlx::query("UPDATE document_jobs SET lease_until=0 WHERE job_id=$1")
                .bind(&id)
                .execute(db.pool())
                .await
                .unwrap();
        }
        let recovered = jobs.claim(None).await.unwrap().unwrap();
        assert_eq!(recovered.attempt.id, id);
        assert_eq!(recovered.file.bytes, b"abandoned");
        if let Some(original) = original {
            assert_ne!(original.token, recovered.attempt.token);
            assert!(db
                .transaction_result_async(|tx| async move { original.lock_and_check(&tx).await })
                .await
                .is_err());
        }
        jobs.fail(&recovered.attempt).await.unwrap();
        assert_eq!(jobs.job(&id).await.unwrap().status, "failed");
    }
}

#[tokio::test]
async fn status_reads_do_not_fetch_accepted_input() {
    let db = database().await;
    let jobs = repository(db.clone());
    let (id, filename) = jobs
        .admit("DEAL-SQL".into(), 1, input("accepted input"), true)
        .await
        .unwrap();
    let claim = jobs.claim(None).await.unwrap().unwrap();
    jobs.fail(&claim.attempt).await.unwrap();
    // A view makes fetching the payload fail, while leaving status columns readable.
    // This exercises the actual repository query rather than duplicating its SQL.
    sqlx::raw_sql(
        "ALTER TABLE document_jobs RENAME TO document_jobs_storage;
         CREATE FUNCTION reject_job_payload_read() RETURNS bytea LANGUAGE plpgsql STABLE AS $$
         BEGIN RAISE EXCEPTION 'status polling must not read accepted input'; END $$;
         CREATE VIEW document_jobs AS
         SELECT job_id,deal_id,filename,status,sequence,result_json,stage,admission_complete,
                reject_job_payload_read() AS input_bytes
         FROM document_jobs_storage;",
    )
    .execute(db.pool())
    .await
    .unwrap();
    assert!(sqlx::query("SELECT * FROM document_jobs")
        .fetch_all(db.pool())
        .await
        .is_err());
    let status = jobs.job(&id).await.unwrap();
    assert_eq!(status.deal, "DEAL-SQL");
    assert_eq!(status.filename, filename);
    assert_eq!(status.status, "failed");
    assert_eq!(status.sequence, 3);
    let result = status.result.unwrap();
    assert!(!result.success);
    assert!(!result.sqlite_committed);
    assert_eq!(result.filename, filename);
}

#[tokio::test]
async fn only_durable_admissions_reach_embedding_and_stale_admission_cannot_replace_them() {
    let db = database().await;
    let jobs = repository(db.clone());
    let (id, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("prepared admission"), false)
        .await
        .unwrap();
    assert!(jobs.claim_ready().await.unwrap().is_none());
    let admission = jobs.claim(Some(&id)).await.unwrap().unwrap();
    jobs.classifying(&admission.attempt).await.unwrap();
    assert_eq!(jobs.job(&id).await.unwrap().stage, "classifying");
    let (document, mut chunks, _) = sample(
        &admission.attempt.file_id,
        "DEAL-SQL",
        "prepared admission",
        1,
    );
    for chunk in &mut chunks {
        chunk.embedding = None;
    }
    let prepared =
        serde_json::json!({ "document": document, "chunks": chunks, "classification": null })
            .to_string();
    jobs.save_prepared(&admission.attempt, prepared.clone(), true)
        .await
        .unwrap();
    assert!(jobs.job(&id).await.unwrap().prepared);
    assert!(jobs
        .save_prepared(&admission.attempt, "{}".into(), true)
        .await
        .is_err());
    let embedding = jobs.claim_ready().await.unwrap().unwrap();
    assert_eq!(embedding.prepared.as_deref(), Some(prepared.as_str()));
    assert_ne!(embedding.attempt.token, admission.attempt.token);
    assert!(jobs.claim_ready().await.unwrap().is_none());
    jobs.fail(&embedding.attempt).await.unwrap();
    let (retry, _) = jobs
        .admit("DEAL-SQL".into(), 1, input("prepared admission"), false)
        .await
        .unwrap();
    assert_eq!(retry, id);
    assert!(jobs.job(&id).await.unwrap().prepared);
    jobs.schedule_prepared(&id, true).await.unwrap();
    let retry = jobs.claim_ready().await.unwrap().unwrap();
    assert_eq!(retry.prepared.as_deref(), Some(prepared.as_str()));
    assert!(db
        .transaction_result_async(|tx| async move { embedding.attempt.lock_and_check(&tx).await })
        .await
        .is_err());
}

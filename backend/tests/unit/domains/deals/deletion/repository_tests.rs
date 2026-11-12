use super::*;
use crate::{
    app::migrations,
    domains::memberships::{model::Capability, require_deal},
    integration_support::seed_identity,
};
async fn fixture() -> DeletionRepository {
    let postgres = PostgresClient::test_database().await.unwrap();
    migrations::migrate(&postgres).await.unwrap();
    seed_identity(&postgres, "deletion-owner@example.com", 'a').await;
    seed_identity(&postgres, "deletion-member@example.com", 'b').await;
    postgres.with_connection(async |c| { c.execute_batch("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES('DELETE-A',1,'Synthetic','buy_side','unknown','pipeline','now','now'),('KEEP-B',2,'Synthetic','buy_side','unknown','pipeline','now','now');
    INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('DELETE-A',1,'owner','now','now'),('DELETE-A',2,'manager','now','now'),('KEEP-B',2,'owner','now','now');
    INSERT INTO deal_access_revisions(deal_id) VALUES('DELETE-A'),('KEEP-B');
    INSERT INTO quarry_files(file_id,deal_id,display_name,metadata_json,created_at,updated_at) VALUES('owned-file','DELETE-A','synthetic','{}','now','now'),('other-file','KEEP-B','synthetic','{}','now','now');
    INSERT INTO assistant_threads(thread_id,user_id,deal_id,title) VALUES('owned-thread',1,'DELETE-A','synthetic'),('private-thread',2,'DELETE-A','synthetic'),('free-thread',2,NULL,'synthetic'),('other-thread',2,'KEEP-B','synthetic');
    INSERT INTO notebooks(notebook_id,user_id,deal_id,title,created_at,updated_at) VALUES('private-book',2,'DELETE-A','synthetic','now','now');
    INSERT INTO notebook_notes(note_id,notebook_id,title,blocks_json,created_at,updated_at) VALUES('private-note','private-book','synthetic','[]','now','now');
    INSERT INTO notebook_save_jobs(job_id,user_id,notebook_id,note_id,operation_id,request_hash,input_json,status,stage,created_at,updated_at) VALUES('save-job',2,'private-book','private-note','operation','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','{}','queued','persisting','now','now');
    ").await }).await.unwrap();
    DeletionRepository::new(postgres)
}
async fn count(repo: &DeletionRepository, table: &str) -> i64 {
    // Test-only table names are fixed by the test, never request input.
    repo.postgres
        .with_connection(async |c| {
            c.query_row(
                &format!("SELECT count(*) FROM {table}"),
                Vec::<crate::adapters::postgres::query::SqlValue>::new(),
                |r| r.get(0),
            )
            .await
        })
        .await
        .unwrap()
}
#[tokio::test]
async fn acceptance_is_durable_idempotent_owner_scoped_and_hides_all_member_access() {
    let repo = fixture().await;
    assert!(matches!(
        repo.preflight("DELETE-A".into(), 2).await,
        Err(RepositoryError::Forbidden)
    ));
    let (a, b) = tokio::join!(
        repo.accept("DELETE-A".into(), 1),
        repo.accept("DELETE-A".into(), 1)
    );
    let a = a.unwrap();
    assert_eq!(a.deletion_id, b.unwrap().deletion_id);
    assert_eq!(count(&repo, "deal_deletions").await, 1);
    assert!(matches!(
        repo.status(a.deletion_id.clone(), 2).await,
        Err(RepositoryError::NotFound(_))
    ));
    assert!(matches!(
        repo.accept("DELETE-A".into(), 2).await,
        Err(RepositoryError::NotFound(_))
    ));
    let access = repo
        .postgres
        .read_transaction_result_async(move |tx| async move {
            let tx = &tx;
            require_deal(tx, "DELETE-A", 1, Capability::Read).await
        })
        .await;
    assert!(matches!(access, Err(RepositoryError::NotFound(_))));
    let write = repo
        .postgres
        .read_transaction_result_async(move |tx| async move {
            let tx = &tx;
            require_deal(tx, "DELETE-A", 2, Capability::ManageTeams).await
        })
        .await;
    assert!(matches!(write, Err(RepositoryError::Conflict(_))));
    assert_eq!(
        count(&repo, "quarry_files").await,
        2,
        "acceptance has not purged canonical data"
    );
    repo.failure(a.deletion_id.clone(), false).await.unwrap();
    let retry = repo.accept("DELETE-A".into(), 1).await.unwrap();
    assert_eq!(retry.deletion_id, a.deletion_id);
    assert_eq!(retry.status, DeletionStatus::Pending);
}
#[tokio::test]
async fn aggregate_purge_preserves_other_deals_free_chats_and_private_notebook_jobs() {
    let repo = fixture().await;
    let op = repo.accept("DELETE-A".into(), 1).await.unwrap();
    repo.snapshot_and_purge(op.clone()).await.unwrap();
    repo.snapshot_and_purge(op.clone()).await.unwrap();
    assert_eq!(
        repo.files(op.deletion_id.clone()).await.unwrap(),
        vec!["owned-file"]
    );
    assert_eq!(count(&repo, "deals").await, 1);
    assert_eq!(count(&repo, "quarry_files").await, 1);
    assert_eq!(count(&repo, "assistant_threads").await, 2);
    assert_eq!(count(&repo, "notebooks").await, 1);
    assert_eq!(count(&repo, "notebook_notes").await, 1);
    assert_eq!(count(&repo, "notebook_save_jobs").await, 1);
    let linked: Option<String> = repo
        .postgres
        .with_connection(async |c| {
            c.query_row(
                "SELECT deal_id FROM notebooks",
                Vec::<crate::adapters::postgres::query::SqlValue>::new(),
                |r| r.get(0),
            )
            .await
        })
        .await
        .unwrap();
    assert_eq!(linked, None);
    assert!(
        repo.complete(op.deletion_id.clone()).await.is_err(),
        "cannot complete with unverified targets"
    );
    repo.purged_file(op.deletion_id.clone(), "owned-file".into())
        .await
        .unwrap();
    repo.complete(op.deletion_id.clone()).await.unwrap();
    assert_eq!(count(&repo, "deal_deletion_files").await, 0);
    assert_eq!(count(&repo, "deal_deletion_threads").await, 0);
    assert_eq!(
        repo.accept("DELETE-A".into(), 1).await.unwrap().status,
        DeletionStatus::Completed
    );
    assert!(repo.status(op.deletion_id, 2).await.is_err());
    repo.postgres
        .with_connection(async |c| {
            let mut s = c.prepare("SELECT conname FROM pg_constraint WHERE connamespace=current_schema()::regnamespace AND NOT convalidated").await.unwrap();
            assert!(s.query(Vec::<crate::adapters::postgres::query::SqlValue>::new()).await.unwrap().next().unwrap().is_none());
            Ok(())
         }).await
        .unwrap();
}
#[tokio::test]
async fn purge_rollback_keeps_notebook_links_threads_targets_and_phase_consistent() {
    let repo = fixture().await;
    let op = repo.accept("DELETE-A".into(), 1).await.unwrap();
    repo.postgres.with_connection(async |c| { c.execute_batch("CREATE OR REPLACE FUNCTION stop_purge_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic failure' USING ERRCODE='23514'; END $$; CREATE TRIGGER stop_purge BEFORE DELETE ON deals FOR EACH ROW EXECUTE FUNCTION stop_purge_fn();").await }).await.unwrap();
    assert!(repo.snapshot_and_purge(op.clone()).await.is_err());
    assert_eq!(count(&repo, "deals").await, 2);
    assert_eq!(count(&repo, "assistant_threads").await, 4);
    assert_eq!(count(&repo, "deal_deletion_files").await, 0);
    let linked: String = repo
        .postgres
        .with_connection(async |c| {
            c.query_row(
                "SELECT deal_id FROM notebooks",
                Vec::<crate::adapters::postgres::query::SqlValue>::new(),
                |r| r.get(0),
            )
            .await
        })
        .await
        .unwrap();
    assert_eq!(linked, "DELETE-A");
    assert_eq!(
        repo.status(op.deletion_id, 1).await.unwrap().phase,
        DeletionPhase::Draining
    );
}
#[tokio::test]
async fn manifest_covers_more_than_one_batch_including_soft_deleted_files() {
    let repo = fixture().await;
    repo.postgres.with_connection(async |c| {
        for n in 0..520 { c.execute("INSERT INTO quarry_files(file_id,deal_id,display_name,metadata_json,deleted_at,created_at,updated_at) VALUES(?1,'DELETE-A','synthetic','{}','now','now','now')",[format!("batch-{n:04}")]).await?; } Ok(())
     }).await.unwrap();
    let op = repo.accept("DELETE-A".into(), 1).await.unwrap();
    repo.snapshot_and_purge(op).await.unwrap();
    assert_eq!(count(&repo, "deal_deletion_files").await, 521);
    assert_eq!(count(&repo, "quarry_files").await, 1);
}

#[tokio::test]
async fn transferred_membership_owner_can_delete_but_creator_provenance_does_not_authorize_it() {
    let repo = fixture().await;
    repo.postgres.with_connection(async |c| { c.execute_batch("UPDATE deal_memberships SET role='viewer' WHERE deal_id='DELETE-A' AND user_id=1; UPDATE deal_memberships SET role='owner' WHERE deal_id='DELETE-A' AND user_id=2;").await }).await.unwrap();
    assert!(matches!(
        repo.accept("DELETE-A".into(), 1).await,
        Err(RepositoryError::Forbidden)
    ));
    let op = repo.accept("DELETE-A".into(), 2).await.unwrap();
    repo.snapshot_and_purge(op.clone()).await.unwrap();
    assert!(repo.status(op.deletion_id.clone(), 1).await.is_err());
    assert!(repo.status(op.deletion_id, 2).await.is_ok());
}

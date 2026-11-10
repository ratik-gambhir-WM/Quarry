use super::*;
use crate::{
    app::migrations,
    domains::memberships::{model::Capability, require_deal},
    integration_support::seed_identity,
};
fn fixture() -> DeletionRepository {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrations::migrate(&sqlite).unwrap();
    seed_identity(&sqlite, "deletion-owner@example.com", 'a');
    seed_identity(&sqlite, "deletion-member@example.com", 'b');
    sqlite.with_connection(|c|c.execute_batch("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES('DELETE-A',1,'Synthetic','buy_side','unknown','pipeline','now','now'),('KEEP-B',2,'Synthetic','buy_side','unknown','pipeline','now','now');
    INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES('DELETE-A',1,'owner','now','now'),('DELETE-A',2,'manager','now','now'),('KEEP-B',2,'owner','now','now');
    INSERT INTO deal_access_revisions(deal_id) VALUES('DELETE-A'),('KEEP-B');
    INSERT INTO quarry_files(file_id,deal_id,display_name,metadata_json,created_at,updated_at) VALUES('owned-file','DELETE-A','synthetic','{}','now','now'),('other-file','KEEP-B','synthetic','{}','now','now');
    INSERT INTO assistant_threads(thread_id,user_id,deal_id,title) VALUES('owned-thread',1,'DELETE-A','synthetic'),('private-thread',2,'DELETE-A','synthetic'),('free-thread',2,NULL,'synthetic'),('other-thread',2,'KEEP-B','synthetic');
    INSERT INTO notebooks(notebook_id,user_id,deal_id,title,created_at,updated_at) VALUES('private-book',2,'DELETE-A','synthetic','now','now');
    INSERT INTO notebook_notes(note_id,notebook_id,title,blocks_json,created_at,updated_at) VALUES('private-note','private-book','synthetic','[]','now','now');
    INSERT INTO notebook_save_jobs(job_id,user_id,notebook_id,note_id,operation_id,request_hash,input_json,status,stage,created_at,updated_at) VALUES('save-job',2,'private-book','private-note','operation','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa','{}','queued','persisting','now','now');
    ")).unwrap();
    DeletionRepository::new(sqlite)
}
fn count(repo: &DeletionRepository, table: &str) -> i64 {
    // Test-only table names are fixed by the test, never request input.
    repo.sqlite
        .with_connection(|c| {
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        })
        .unwrap()
}
#[tokio::test]
async fn acceptance_is_durable_idempotent_owner_scoped_and_hides_all_member_access() {
    let repo = fixture();
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
    assert_eq!(count(&repo, "deal_deletions"), 1);
    assert!(matches!(
        repo.status(a.deletion_id.clone(), 2).await,
        Err(RepositoryError::NotFound(_))
    ));
    assert!(matches!(
        repo.accept("DELETE-A".into(), 2).await,
        Err(RepositoryError::NotFound(_))
    ));
    let access = repo
        .sqlite
        .read_transaction_result_async(move |tx| require_deal(tx, "DELETE-A", 1, Capability::Read))
        .await;
    assert!(matches!(access, Err(RepositoryError::NotFound(_))));
    let write = repo
        .sqlite
        .read_transaction_result_async(move |tx| {
            require_deal(tx, "DELETE-A", 2, Capability::ManageTeams)
        })
        .await;
    assert!(matches!(write, Err(RepositoryError::Conflict(_))));
    assert_eq!(
        count(&repo, "quarry_files"),
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
    let repo = fixture();
    let op = repo.accept("DELETE-A".into(), 1).await.unwrap();
    repo.snapshot_and_purge(op.clone()).await.unwrap();
    repo.snapshot_and_purge(op.clone()).await.unwrap();
    assert_eq!(
        repo.files(op.deletion_id.clone()).await.unwrap(),
        vec!["owned-file"]
    );
    assert_eq!(count(&repo, "deals"), 1);
    assert_eq!(count(&repo, "quarry_files"), 1);
    assert_eq!(count(&repo, "assistant_threads"), 2);
    assert_eq!(count(&repo, "notebooks"), 1);
    assert_eq!(count(&repo, "notebook_notes"), 1);
    assert_eq!(count(&repo, "notebook_save_jobs"), 1);
    let linked: Option<String> = repo
        .sqlite
        .with_connection(|c| c.query_row("SELECT deal_id FROM notebooks", [], |r| r.get(0)))
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
    assert_eq!(count(&repo, "deal_deletion_files"), 0);
    assert_eq!(count(&repo, "deal_deletion_threads"), 0);
    assert_eq!(
        repo.accept("DELETE-A".into(), 1).await.unwrap().status,
        DeletionStatus::Completed
    );
    assert!(repo.status(op.deletion_id, 2).await.is_err());
    repo.sqlite
        .with_connection(|c| {
            let mut s = c.prepare("PRAGMA foreign_key_check").unwrap();
            assert!(s.query([]).unwrap().next().unwrap().is_none());
            Ok(())
        })
        .unwrap();
}
#[tokio::test]
async fn purge_rollback_keeps_notebook_links_threads_targets_and_phase_consistent() {
    let repo = fixture();
    let op = repo.accept("DELETE-A".into(), 1).await.unwrap();
    repo.sqlite.with_connection(|c|c.execute_batch("CREATE TRIGGER stop_purge BEFORE DELETE ON deals BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;")).unwrap();
    assert!(repo.snapshot_and_purge(op.clone()).await.is_err());
    assert_eq!(count(&repo, "deals"), 2);
    assert_eq!(count(&repo, "assistant_threads"), 4);
    assert_eq!(count(&repo, "deal_deletion_files"), 0);
    let linked: String = repo
        .sqlite
        .with_connection(|c| c.query_row("SELECT deal_id FROM notebooks", [], |r| r.get(0)))
        .unwrap();
    assert_eq!(linked, "DELETE-A");
    assert_eq!(
        repo.status(op.deletion_id, 1).await.unwrap().phase,
        DeletionPhase::Draining
    );
}
#[tokio::test]
async fn manifest_covers_more_than_one_batch_including_soft_deleted_files() {
    let repo = fixture();
    repo.sqlite.with_connection(|c|{
        for n in 0..520 { c.execute("INSERT INTO quarry_files(file_id,deal_id,display_name,metadata_json,deleted_at,created_at,updated_at) VALUES(?1,'DELETE-A','synthetic','{}','now','now','now')",[format!("batch-{n:04}")])?; } Ok(())
    }).unwrap();
    let op = repo.accept("DELETE-A".into(), 1).await.unwrap();
    repo.snapshot_and_purge(op).await.unwrap();
    assert_eq!(count(&repo, "deal_deletion_files"), 521);
    assert_eq!(count(&repo, "quarry_files"), 1);
}

#[tokio::test]
async fn transferred_membership_owner_can_delete_but_creator_provenance_does_not_authorize_it() {
    let repo = fixture();
    repo.sqlite.with_connection(|c| c.execute_batch("UPDATE deal_memberships SET role='viewer' WHERE deal_id='DELETE-A' AND user_id=1; UPDATE deal_memberships SET role='owner' WHERE deal_id='DELETE-A' AND user_id=2;")).unwrap();
    assert!(matches!(
        repo.accept("DELETE-A".into(), 1).await,
        Err(RepositoryError::Forbidden)
    ));
    let op = repo.accept("DELETE-A".into(), 2).await.unwrap();
    repo.snapshot_and_purge(op.clone()).await.unwrap();
    assert!(repo.status(op.deletion_id.clone(), 1).await.is_err());
    assert!(repo.status(op.deletion_id, 2).await.is_ok());
}

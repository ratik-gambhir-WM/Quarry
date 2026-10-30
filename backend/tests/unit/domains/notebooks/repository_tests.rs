use super::*;
use crate::domains::notebooks::test_support;
fn repository() -> NotebookRepository {
    NotebookRepository::new(test_support::database())
}
async fn saved(
    repo: &NotebookRepository,
    book: &str,
    folder_id: Option<String>,
    title: &str,
) -> Note {
    let mut input = test_support::input(title);
    input.folder_id = folder_id.map_or(PatchField::Null, PatchField::Value);
    let job = repo.admit(1, book.into(), None, input).await.unwrap();
    let claim = repo.claim().await.unwrap().unwrap();
    assert_eq!(job.job_id, claim.job.job_id);
    repo.persist(&claim).await.unwrap();
    repo.checkpoint(&claim, "indexing", Some("test".into()), true)
        .await
        .unwrap();
    repo.get_note(1, book.into(), job.note_id).await.unwrap()
}
#[tokio::test]
async fn notebook_repository_lifecycle_ownership_relocation_and_cascades() {
    let repo = repository();
    let b = repo.create(1, "Notebook".into(), None).await.unwrap();
    let other = repo.create(2, "Other".into(), None).await.unwrap();
    assert!(repo.get(2, b.notebook_id.clone()).await.is_err());
    let f = repo
        .create_folder(1, b.notebook_id.clone(), "Folder".into())
        .await
        .unwrap();
    let f2 = repo
        .create_folder(1, b.notebook_id.clone(), "Folder".into())
        .await
        .unwrap();
    let root = saved(&repo, &b.notebook_id, None, "Root").await;
    let child = saved(&repo, &b.notebook_id, Some(f.folder_id.clone()), "Child").await;
    saved(&repo, &b.notebook_id, Some(f2.folder_id.clone()), "Second").await;
    assert!(repo
        .get_folder(1, other.notebook_id.clone(), f.folder_id.clone())
        .await
        .is_err());
    repo.delete_folder(1, b.notebook_id.clone(), f.folder_id)
        .await
        .unwrap();
    let relocated = repo
        .get_note(1, b.notebook_id.clone(), child.summary.note_id.clone())
        .await
        .unwrap();
    assert_eq!(relocated.summary.revision, 2);
    assert_eq!(relocated.summary.folder_id, None);
    assert_eq!(relocated.blocks, child.blocks);
    assert_eq!(
        repo.get_note(1, b.notebook_id.clone(), root.summary.note_id)
            .await
            .unwrap()
            .summary
            .revision,
        1
    );
    repo.delete(1, b.notebook_id.clone()).await.unwrap();
    assert!(repo
        .get_note(1, b.notebook_id.clone(), child.summary.note_id)
        .await
        .is_err());
    assert!(repo.get(2, other.notebook_id).await.is_ok());
    assert_eq!(repo.cleanup_batch().await.unwrap().len(), 1);
}
#[tokio::test]
async fn notebook_revision_races_active_jobs_idempotence_and_content_clear() {
    let repo = repository();
    let book = repo
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let note = saved(&repo, &book, None, "Initial").await;
    let mut input = test_support::input("Updated");
    input.expected_revision = PatchField::Value(1);
    input.blocks = PatchField::Value(test_support::blocks());
    let a = repo.admit(
        1,
        book.clone(),
        Some(note.summary.note_id.clone()),
        input.clone(),
    );
    let mut competing = input.clone();
    competing.operation_id = uuid::Uuid::new_v4().to_string();
    let b = repo.admit(
        1,
        book.clone(),
        Some(note.summary.note_id.clone()),
        competing.clone(),
    );
    let (a, b) = tokio::join!(a, b);
    assert_ne!(a.is_ok(), b.is_ok());
    let (job, original) = if let Ok(job) = a {
        (job, input)
    } else {
        (b.unwrap(), competing)
    };
    let replay = repo
        .admit(
            1,
            book.clone(),
            Some(note.summary.note_id.clone()),
            original.clone(),
        )
        .await
        .unwrap();
    assert_eq!(job.job_id, replay.job_id);
    let mut different = original;
    different.title = PatchField::Value("Different".into());
    assert!(repo
        .admit(
            1,
            book.clone(),
            Some(note.summary.note_id.clone()),
            different
        )
        .await
        .is_err());
    let claim = repo.claim().await.unwrap().unwrap();
    repo.persist(&claim).await.unwrap();
    repo.checkpoint(&claim, "indexing", None, true)
        .await
        .unwrap();
    let loaded = repo
        .get_note(1, book.clone(), note.summary.note_id.clone())
        .await
        .unwrap();
    assert_eq!(loaded.summary.revision, 2);
    assert_eq!(loaded.blocks, test_support::blocks());
    let mut stale = test_support::input("Stale");
    stale.expected_revision = PatchField::Value(1);
    assert!(repo
        .admit(1, book.clone(), Some(note.summary.note_id.clone()), stale)
        .await
        .is_err());
    let mut clear = test_support::input("");
    clear.expected_revision = PatchField::Value(2);
    repo.admit(1, book.clone(), Some(note.summary.note_id.clone()), clear)
        .await
        .unwrap();
    let claim = repo.claim().await.unwrap().unwrap();
    repo.persist(&claim).await.unwrap();
    let cleared = repo.get_note(1, book, note.summary.note_id).await.unwrap();
    assert_eq!(cleared.blocks, serde_json::json!([]));
    assert_eq!(cleared.summary.revision, 3);
}
#[tokio::test]
async fn notebook_folder_delete_rolls_back_typed_domain_failure_and_overflow() {
    let repo = repository();
    let book = repo
        .create(1, "Book".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let f = repo
        .create_folder(1, book.clone(), "Folder".into())
        .await
        .unwrap();
    let n = saved(&repo, &book, Some(f.folder_id.clone()), "Note").await;
    let fid = f.folder_id.clone();
    let result: Result<()> = repo
        .sqlite
        .transaction_result_async(move |tx| {
            relocate_folder(tx, &fid)?;
            Err(conflict())
        })
        .await;
    assert!(result.is_err());
    let n = repo
        .get_note(1, book.clone(), n.summary.note_id)
        .await
        .unwrap();
    assert_eq!(n.summary.folder_id, Some(f.folder_id.clone()));
    assert_eq!(n.summary.revision, 1);
    repo.sqlite
        .write_async(
            Q::update("notebook_notes")
                .set("revision", MAX_REVISION)
                .and_where(C::equal("note_id", &n.summary.note_id))
                .build(),
        )
        .await
        .unwrap();
    assert!(repo
        .delete_folder(1, book.clone(), f.folder_id.clone())
        .await
        .is_err());
    assert!(repo
        .get_folder(1, book.clone(), f.folder_id.clone())
        .await
        .is_ok());
    assert_eq!(
        repo.get_note(1, book, n.summary.note_id)
            .await
            .unwrap()
            .summary
            .folder_id,
        Some(f.folder_id)
    );
}
#[tokio::test]
async fn notebook_constraints_parent_membership_deal_ownership_and_literal_search() {
    let repo = repository();
    repo.sqlite.with_connection(|c| { c.execute("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES('DEAL-A',1,'Deal','buy_side','unknown','pipeline','now','now')",[])?; Ok(()) }).unwrap();
    assert!(repo
        .create(2, "Foreign".into(), Some("DEAL-A".into()))
        .await
        .is_err());
    let book = repo
        .create(1, "Linked".into(), Some("DEAL-A".into()))
        .await
        .unwrap()
        .notebook_id;
    assert!(repo
        .sqlite
        .with_connection(|c| c.execute("DELETE FROM deals WHERE deal_id='DEAL-A'", []))
        .is_err());
    repo.update(1, book.clone(), None, PatchField::Null)
        .await
        .unwrap();
    let other = repo
        .create(1, "Other".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let folder = repo.create_folder(1, other, "Other".into()).await.unwrap();
    let mut invalid = test_support::input("Foreign folder");
    invalid.folder_id = PatchField::Value(folder.folder_id);
    assert!(repo.admit(1, book.clone(), None, invalid).await.is_err());
    let n = saved(&repo, &book, None, "Literal %_\\ title").await;
    saved(&repo, &book, None, "Other").await;
    let page = repo
        .list_notes(
            1,
            book.clone(),
            ListOptions {
                limit: 1,
                offset: 0,
                query: Some("%_\\".into()),
                relationship: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items.notes.len(), 1);
    assert!(!page.has_more);
    assert_eq!(page.items.notes[0].note_id, n.summary.note_id);
    // Summary reads exclude corrupt native content, while full reads fail closed.
    repo.sqlite
        .write_async(
            Q::update("notebook_notes")
                .set("blocks_json", "[123]")
                .and_where(C::equal("note_id", &n.summary.note_id))
                .build(),
        )
        .await
        .unwrap();
    assert!(repo
        .get_note(1, book.clone(), n.summary.note_id)
        .await
        .is_err());
    assert!(
        repo.list_notes(
            1,
            book,
            ListOptions {
                limit: 1,
                ..ListOptions::default()
            }
        )
        .await
        .unwrap()
        .has_more
    );
}

#[tokio::test]
async fn notebook_queue_capacity_and_claims_are_atomic_and_idempotent() {
    let repo = repository();
    let book = repo
        .create(1, "Queue".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let original = test_support::input("Accepted");
    let first = repo
        .admit(1, book.clone(), None, original.clone())
        .await
        .unwrap();
    for _ in 1..32 {
        repo.admit(1, book.clone(), None, test_support::input("Queued"))
            .await
            .unwrap();
    }
    assert!(matches!(
        repo.admit(1, book.clone(), None, test_support::input("Overflow"))
            .await,
        Err(Error::Capacity)
    ));
    assert_eq!(
        repo.admit(1, book, None, original).await.unwrap().job_id,
        first.job_id
    );
    let (a, b) = tokio::join!(repo.claim(), repo.claim());
    let a = a.unwrap().unwrap();
    let b = b.unwrap().unwrap();
    assert_ne!(a.job.job_id, b.job.job_id);
    assert_ne!(a.token, b.token);
}

#[tokio::test]
async fn notebook_corrupt_checkpoint_fails_closed_without_poisoning_the_queue() {
    let repo = repository();
    let book = repo
        .create(1, "Queue".into(), None)
        .await
        .unwrap()
        .notebook_id;
    let bad = repo
        .admit(1, book.clone(), None, test_support::input("Bad checkpoint"))
        .await
        .unwrap();
    repo.sqlite
        .with_connection(|c| {
            c.execute(
                "UPDATE notebook_save_jobs SET input_json='{}' WHERE job_id=?1",
                [&bad.job_id],
            )
        })
        .unwrap();
    assert!(repo.claim().await.unwrap().is_none());
    let failed = repo.job(1, book.clone(), bad.job_id).await.unwrap();
    assert_eq!(failed.status, "failed");
    assert!(!failed.sqlite_committed);
    assert!(!failed.retryable);
    repo.admit(1, book, None, test_support::input("Healthy"))
        .await
        .unwrap();
    assert!(repo.claim().await.unwrap().is_some());
}

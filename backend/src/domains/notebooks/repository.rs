//! Owner-scoped aggregate reads and immediate transactions; no network work under the DB lock.
use super::model::*;
use crate::adapters::postgres::client::Row;
use crate::adapters::postgres::client::RowResult;
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresTransaction},
        query::{Condition as C, LockWait, RowLock, SortDirection as Sort, SqlBuilder as Q},
    },
    shared::error::RepositoryError as Error,
};
use serde_json::Value;

pub(super) type Result<T> = std::result::Result<T, Error>;
pub(super) const SUMMARY: [&str; 8] = [
    "note_id",
    "notebook_id",
    "folder_id",
    "title",
    "content_schema_version",
    "revision",
    "created_at",
    "updated_at",
];
#[derive(Clone)]
pub(crate) struct NotebookRepository {
    pub(super) postgres: PostgresClient,
}
pub(super) fn missing() -> Error {
    Error::NotFound("resource not found".into())
}
pub(super) fn conflict() -> Error {
    Error::Conflict("note revision conflict; reload before saving".into())
}
pub(super) async fn owned(
    tx: &PostgresTransaction,
    owner: i64,
    notebook: &str,
) -> Result<Notebook> {
    tx.read_one_with(
        Q::select("notebooks")
            .and_where(C::equal("notebook_id", notebook))
            .and_where(C::equal("user_id", owner))
            .build(),
        notebook_row,
    )
    .await?
    .ok_or_else(missing)
}
pub(super) async fn lock_parent(
    tx: &PostgresTransaction,
    owner: i64,
    notebook: &str,
    mode: RowLock,
) -> Result<Notebook> {
    tx.lock_with(
        Q::select("notebooks")
            .and_where(C::equal("notebook_id", notebook))
            .and_where(C::equal("user_id", owner))
            .lock(mode, LockWait::Wait)
            .build(),
        notebook_row,
    )
    .await?
    .pop()
    .ok_or_else(missing)
}
pub(super) async fn lock_folder(
    tx: &PostgresTransaction,
    notebook: &str,
    id: &str,
    mode: RowLock,
) -> Result<Folder> {
    tx.lock_with(
        Q::select("notebook_folders")
            .and_where(C::equal("notebook_id", notebook))
            .and_where(C::equal("folder_id", id))
            .lock(mode, LockWait::Wait)
            .build(),
        folder_row,
    )
    .await?
    .pop()
    .ok_or_else(missing)
}
pub(super) async fn deal_owned(tx: &PostgresTransaction, owner: i64, deal: &str) -> Result<()> {
    crate::domains::deals::coordination::lock_authority(tx, deal, RowLock::Share).await?;
    crate::domains::memberships::require_deal(
        tx,
        deal,
        owner,
        crate::domains::memberships::model::Capability::Read,
    )
    .await?;
    Ok(())
}

pub(super) async fn folder(tx: &PostgresTransaction, notebook: &str, id: &str) -> Result<Folder> {
    tx.read_one_with(
        Q::select("notebook_folders")
            .and_where(C::equal("notebook_id", notebook))
            .and_where(C::equal("folder_id", id))
            .build(),
        folder_row,
    )
    .await?
    .ok_or_else(missing)
}
pub(super) async fn note(tx: &PostgresTransaction, notebook: &str, id: &str) -> Result<Note> {
    let note = tx
        .read_one_with(
            Q::select("notebook_notes")
                .and_where(C::equal("notebook_id", notebook))
                .and_where(C::equal("note_id", id))
                .build(),
            note_row,
        )
        .await?
        .ok_or_else(missing)?;
    if note.summary.content_schema_version != 1
        || !(1..=MAX_REVISION).contains(&note.summary.revision)
    {
        return Err(Error::storage("invalid stored note version"));
    }
    super::content::validate(&note.blocks)
        .map_err(|_| Error::storage("invalid stored note document"))?;
    Ok(note)
}
impl NotebookRepository {
    pub(crate) fn new(postgres: PostgresClient) -> Self {
        Self { postgres }
    }
    pub(super) async fn create(
        &self,
        owner: i64,
        title: String,
        deal: Option<String>,
    ) -> Result<Notebook> {
        let id = uuid::Uuid::new_v4().to_string();
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                if let Some(deal) = &deal {
                    deal_owned(tx, owner, deal).await?;
                }
                let time = now();
                tx.write(
                    Q::insert_into("notebooks")
                        .value("notebook_id", &id)
                        .value("user_id", owner)
                        .value("title", title)
                        .value("deal_id", deal)
                        .value("created_at", &time)
                        .value("updated_at", time)
                        .build(),
                )
                .await?;
                owned(tx, owner, &id).await
            })
            .await
    }
    pub(super) async fn get(&self, owner: i64, id: String) -> Result<Notebook> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                owned(tx, owner, &id).await
            })
            .await
    }
    pub(super) async fn update(
        &self,
        owner: i64,
        id: String,
        title: Option<String>,
        deal: PatchField<String>,
    ) -> Result<Notebook> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                if let PatchField::Value(deal) = &deal {
                    deal_owned(tx, owner, deal).await?;
                }
                lock_parent(tx, owner, &id, RowLock::Update).await?;
                let mut update = Q::update("notebooks").set("updated_at", now());
                if let Some(title) = title {
                    update = update.set("title", title);
                }
                match deal {
                    PatchField::Missing => {}
                    PatchField::Null => update = update.set("deal_id", None::<String>),
                    PatchField::Value(value) => update = update.set("deal_id", value),
                }
                tx.write(update.and_where(C::equal("notebook_id", &id)).build())
                    .await?;
                owned(tx, owner, &id).await
            })
            .await
    }
    pub(super) async fn list(&self, owner: i64, options: ListOptions) -> Result<Page<Notebooks>> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                let mut q = Q::select("notebooks").and_where(C::equal("user_id", owner));
                if let Some(deal) = &options.relationship {
                    if deal == "none" {
                        q = q.and_where(C::is_null("deal_id"));
                    } else {
                        crate::domains::memberships::require_deal(
                            tx,
                            deal,
                            owner,
                            crate::domains::memberships::model::Capability::Read,
                        )
                        .await?;
                        q = q.and_where(C::equal("deal_id", deal.as_str()));
                    }
                }
                let rows = tx
                    .read_with(
                        q.order_by("updated_at", Sort::Descending)
                            .order_by("notebook_id", Sort::Ascending)
                            .limit(options.limit + 1)
                            .offset(options.offset)
                            .build(),
                        notebook_row,
                    )
                    .await?;
                Ok(Page::from_rows(rows, &options, |notebooks| Notebooks {
                    notebooks,
                }))
            })
            .await
    }
    pub(super) async fn delete(&self, owner: i64, id: String) -> Result<()> {
        self.postgres
            .transaction_result_async(move |tx| async move { let tx = &tx;
                lock_parent(tx, owner, &id, RowLock::Update).await?;
                // Include admitted creations that have not reached canonical persistence yet.
                for table in ["notebook_notes", "notebook_save_jobs"] {
                    let mut after = String::new();
                    loop {
                        let ids = tx.read_with(
                            Q::select(table)
                                .columns(["note_id"])
                                .and_where(C::equal("notebook_id", &id))
                                .and_where(C::compare(
                                    "note_id",
                                    crate::adapters::postgres::query::ComparisonOperator::GreaterThan,
                                    after,
                                ))
                                .order_by("note_id", Sort::Ascending)
                                .limit(500)
                                .build(),
                            |r| r.get::<_, String>(0),
                        ).await?;
                        let Some(last) = ids.last() else {
                            break;
                        };
                        after = last.clone();
                        for note in ids {
                            enqueue_cleanup(tx, owner, &note).await?;
                        }
                    }
                }
                tx.write(
                    Q::delete_from("notebooks")
                        .and_where(C::equal("notebook_id", id))
                        .build(),
                ).await?;
                Ok(())
             })
            .await
    }
    pub(super) async fn create_folder(
        &self,
        owner: i64,
        notebook: String,
        name: String,
    ) -> Result<Folder> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                lock_parent(tx, owner, &notebook, RowLock::Share).await?;
                let id = uuid::Uuid::new_v4().to_string();
                let time = now();
                tx.write(
                    Q::insert_into("notebook_folders")
                        .value("folder_id", &id)
                        .value("notebook_id", &notebook)
                        .value("name", name)
                        .value("created_at", &time)
                        .value("updated_at", time)
                        .build(),
                )
                .await?;
                folder(tx, &notebook, &id).await
            })
            .await
    }
    pub(super) async fn get_folder(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> Result<Folder> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                owned(tx, owner, &notebook).await?;
                folder(tx, &notebook, &id).await
            })
            .await
    }
    pub(super) async fn update_folder(
        &self,
        owner: i64,
        notebook: String,
        id: String,
        name: String,
    ) -> Result<Folder> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                lock_parent(tx, owner, &notebook, RowLock::Share).await?;
                folder(tx, &notebook, &id).await?;
                tx.write(
                    Q::update("notebook_folders")
                        .set("name", name)
                        .set("updated_at", now())
                        .and_where(C::equal("folder_id", &id))
                        .build(),
                )
                .await?;
                folder(tx, &notebook, &id).await
            })
            .await
    }
    pub(super) async fn list_folders(
        &self,
        owner: i64,
        notebook: String,
        options: ListOptions,
    ) -> Result<Page<Folders>> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                owned(tx, owner, &notebook).await?;
                let rows = tx
                    .read_with(
                        Q::select("notebook_folders")
                            .and_where(C::equal("notebook_id", notebook))
                            .order_by("folder_id", Sort::Ascending)
                            .limit(options.limit + 1)
                            .offset(options.offset)
                            .build(),
                        folder_row,
                    )
                    .await?;
                Ok(Page::from_rows(rows, &options, |folders| Folders {
                    folders,
                }))
            })
            .await
    }
    pub(super) async fn delete_folder(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> Result<()> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                lock_parent(tx, owner, &notebook, RowLock::Share).await?;
                lock_folder(tx, &notebook, &id, RowLock::Update).await?;
                relocate_folder(tx, &id).await?;
                tx.write(
                    Q::delete_from("notebook_folders")
                        .and_where(C::equal("folder_id", id))
                        .build(),
                )
                .await?;
                Ok(())
            })
            .await
    }
    pub(super) async fn get_note(&self, owner: i64, notebook: String, id: String) -> Result<Note> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                owned(tx, owner, &notebook).await?;
                note(tx, &notebook, &id).await
            })
            .await
    }
    pub(super) async fn list_notes(
        &self,
        owner: i64,
        notebook: String,
        options: ListOptions,
    ) -> Result<Page<Notes>> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                owned(tx, owner, &notebook).await?;
                let mut q = Q::select("notebook_notes")
                    .columns(SUMMARY)
                    .and_where(C::equal("notebook_id", &notebook));
                if let Some(id) = &options.relationship {
                    if id == "none" {
                        q = q.and_where(C::is_null("folder_id"));
                    } else {
                        folder(tx, &notebook, id).await?;
                        q = q.and_where(C::equal("folder_id", id.as_str()));
                    }
                }
                if let Some(text) = &options.query {
                    q = q.and_where(C::literal_substring("title", text));
                }
                let rows = tx
                    .read_with(
                        q.order_by("updated_at", Sort::Descending)
                            .order_by("note_id", Sort::Ascending)
                            .limit(options.limit + 1)
                            .offset(options.offset)
                            .build(),
                        summary_row,
                    )
                    .await?;
                Ok(Page::from_rows(rows, &options, |notes| Notes { notes }))
            })
            .await
    }
    pub(super) async fn delete_note(
        &self,
        owner: i64,
        notebook: String,
        id: String,
        expected_revision: i64,
    ) -> Result<()> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                lock_parent(tx, owner, &notebook, RowLock::Share).await?;
                let changed = tx
                    .write(
                        Q::delete_from("notebook_notes")
                            .and_where(C::equal("notebook_id", &notebook))
                            .and_where(C::equal("note_id", &id))
                            .and_where(C::equal("revision", expected_revision))
                            .build(),
                    )
                    .await?;
                if changed.rows_affected != 1 {
                    note(tx, &notebook, &id).await?;
                    return Err(conflict());
                }
                enqueue_cleanup(tx, owner, &id).await?;
                tx.write(
                    Q::update("notebook_save_jobs")
                        .set("status", "failed")
                        .set("error_code", "resource_not_found")
                        .set("lease_token", None::<String>)
                        .set("updated_at", now())
                        .increment("sequence")
                        .and_where(C::equal("note_id", &id))
                        .and_where(C::in_values("status", ["queued", "processing"]))
                        .build(),
                )
                .await?;
                Ok(())
            })
            .await
    }
}
pub(super) async fn relocate_folder(tx: &PostgresTransaction, id: &str) -> Result<()> {
    if tx
        .read_one(
            Q::select("notebook_notes")
                .and_where(C::equal("folder_id", id))
                .and_where(C::equal("revision", MAX_REVISION))
                .limit(1)
                .build(),
        )
        .await?
        .is_some()
    {
        return Err(conflict());
    }
    tx.write(
        Q::update("notebook_notes")
            .set("folder_id", None::<String>)
            .set("updated_at", now())
            .increment("revision")
            .and_where(C::equal("folder_id", id))
            .build(),
    )
    .await?;
    Ok(())
}
async fn enqueue_cleanup(tx: &PostgresTransaction, owner: i64, note: &str) -> Result<()> {
    tx.write(
        Q::insert_into("notebook_cleanup")
            .value("note_id", note)
            .value("user_id", owner)
            .value("created_at", now())
            .on_conflict_do_nothing(["note_id"])
            .build(),
    )
    .await?;
    Ok(())
}
fn notebook_row(r: &Row<'_>) -> RowResult<Notebook> {
    Ok(Notebook {
        notebook_id: r.get("notebook_id")?,
        title: r.get("title")?,
        deal_id: r.get("deal_id")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}
fn folder_row(r: &Row<'_>) -> RowResult<Folder> {
    Ok(Folder {
        folder_id: r.get("folder_id")?,
        notebook_id: r.get("notebook_id")?,
        name: r.get("name")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}
fn summary_row(r: &Row<'_>) -> RowResult<NoteSummary> {
    Ok(NoteSummary {
        note_id: r.get("note_id")?,
        notebook_id: r.get("notebook_id")?,
        folder_id: r.get("folder_id")?,
        title: r.get("title")?,
        content_schema_version: r.get("content_schema_version")?,
        revision: r.get("revision")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}
fn note_row(r: &Row<'_>) -> RowResult<Note> {
    let json: String = r.get("blocks_json")?;
    let blocks: Value = serde_json::from_str(&json)
        .map_err(crate::adapters::postgres::client::PostgresError::decode)?;
    Ok(Note {
        summary: summary_row(r)?,
        blocks,
    })
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/repository_tests.rs"]
mod tests;

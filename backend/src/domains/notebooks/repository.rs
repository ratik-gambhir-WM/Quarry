//! Owner-scoped aggregate reads and immediate transactions; no network work under the DB lock.
use super::model::*;
use crate::{
    adapters::sqlite::{
        client::{SqliteClient, SqliteTransaction},
        query::{Condition as C, SortDirection as Sort, SqlBuilder as Q},
    },
    shared::error::RepositoryError as Error,
};
use rusqlite::Row;
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
    pub(super) sqlite: SqliteClient,
}
pub(super) fn missing() -> Error {
    Error::NotFound("resource not found".into())
}
pub(super) fn conflict() -> Error {
    Error::Conflict("note revision conflict; reload before saving".into())
}
pub(super) fn owned(tx: &SqliteTransaction<'_>, owner: i64, notebook: &str) -> Result<Notebook> {
    tx.read_one_with(
        Q::select("notebooks")
            .and_where(C::equal("notebook_id", notebook))
            .and_where(C::equal("user_id", owner))
            .build(),
        notebook_row,
    )?
    .ok_or_else(missing)
}
pub(super) fn deal_owned(tx: &SqliteTransaction<'_>, owner: i64, deal: &str) -> Result<()> {
    tx.read_one(
        Q::select("deals")
            .columns(["deal_id"])
            .and_where(C::equal("deal_id", deal))
            .and_where(C::equal("user_id", owner))
            .build(),
    )?
    .ok_or_else(missing)?;
    Ok(())
}
pub(super) fn folder(tx: &SqliteTransaction<'_>, notebook: &str, id: &str) -> Result<Folder> {
    tx.read_one_with(
        Q::select("notebook_folders")
            .and_where(C::equal("notebook_id", notebook))
            .and_where(C::equal("folder_id", id))
            .build(),
        folder_row,
    )?
    .ok_or_else(missing)
}
pub(super) fn note(tx: &SqliteTransaction<'_>, notebook: &str, id: &str) -> Result<Note> {
    let note = tx
        .read_one_with(
            Q::select("notebook_notes")
                .and_where(C::equal("notebook_id", notebook))
                .and_where(C::equal("note_id", id))
                .build(),
            note_row,
        )?
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
    pub(crate) fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }
    pub(super) async fn create(
        &self,
        owner: i64,
        title: String,
        deal: Option<String>,
    ) -> Result<Notebook> {
        let id = uuid::Uuid::new_v4().to_string();
        self.sqlite
            .transaction_result_async(move |tx| {
                if let Some(deal) = &deal {
                    deal_owned(tx, owner, deal)?;
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
                )?;
                owned(tx, owner, &id)
            })
            .await
    }
    pub(super) async fn get(&self, owner: i64, id: String) -> Result<Notebook> {
        self.sqlite
            .transaction_result_async(move |tx| owned(tx, owner, &id))
            .await
    }
    pub(super) async fn update(
        &self,
        owner: i64,
        id: String,
        title: Option<String>,
        deal: PatchField<String>,
    ) -> Result<Notebook> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let previous = owned(tx, owner, &id)?;
                if let PatchField::Value(deal) = &deal {
                    deal_owned(tx, owner, deal)?;
                }
                tx.write(
                    Q::update("notebooks")
                        .set("title", title.unwrap_or(previous.title))
                        .set("deal_id", deal.apply(previous.deal_id))
                        .set("updated_at", now())
                        .and_where(C::equal("notebook_id", &id))
                        .build(),
                )?;
                owned(tx, owner, &id)
            })
            .await
    }
    pub(super) async fn list(&self, owner: i64, options: ListOptions) -> Result<Page<Notebooks>> {
        self.sqlite
            .transaction_result_async(move |tx| {
                let mut q = Q::select("notebooks").and_where(C::equal("user_id", owner));
                if let Some(deal) = &options.relationship {
                    if deal == "none" {
                        q = q.and_where(C::is_null("deal_id"));
                    } else {
                        deal_owned(tx, owner, deal)?;
                        q = q.and_where(C::equal("deal_id", deal.as_str()));
                    }
                }
                let rows = tx.read_with(
                    q.order_by("updated_at", Sort::Descending)
                        .order_by("notebook_id", Sort::Ascending)
                        .limit(options.limit + 1)
                        .offset(options.offset)
                        .build(),
                    notebook_row,
                )?;
                Ok(Page::from_rows(rows, &options, |notebooks| Notebooks {
                    notebooks,
                }))
            })
            .await
    }
    pub(super) async fn delete(&self, owner: i64, id: String) -> Result<()> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &id)?;
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
                                    crate::adapters::sqlite::query::ComparisonOperator::GreaterThan,
                                    after,
                                ))
                                .order_by("note_id", Sort::Ascending)
                                .limit(500)
                                .build(),
                            |r| r.get::<_, String>(0),
                        )?;
                        let Some(last) = ids.last() else {
                            break;
                        };
                        after = last.clone();
                        for note in ids {
                            enqueue_cleanup(tx, owner, &note)?;
                        }
                    }
                }
                tx.write(
                    Q::delete_from("notebooks")
                        .and_where(C::equal("notebook_id", id))
                        .build(),
                )?;
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
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
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
                )?;
                folder(tx, &notebook, &id)
            })
            .await
    }
    pub(super) async fn get_folder(
        &self,
        owner: i64,
        notebook: String,
        id: String,
    ) -> Result<Folder> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                folder(tx, &notebook, &id)
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
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                folder(tx, &notebook, &id)?;
                tx.write(
                    Q::update("notebook_folders")
                        .set("name", name)
                        .set("updated_at", now())
                        .and_where(C::equal("folder_id", &id))
                        .build(),
                )?;
                folder(tx, &notebook, &id)
            })
            .await
    }
    pub(super) async fn list_folders(
        &self,
        owner: i64,
        notebook: String,
        options: ListOptions,
    ) -> Result<Page<Folders>> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                let rows = tx.read_with(
                    Q::select("notebook_folders")
                        .and_where(C::equal("notebook_id", notebook))
                        .order_by("folder_id", Sort::Ascending)
                        .limit(options.limit + 1)
                        .offset(options.offset)
                        .build(),
                    folder_row,
                )?;
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
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                folder(tx, &notebook, &id)?;
                relocate_folder(tx, &id)?;
                tx.write(
                    Q::delete_from("notebook_folders")
                        .and_where(C::equal("folder_id", id))
                        .build(),
                )?;
                Ok(())
            })
            .await
    }
    pub(super) async fn get_note(&self, owner: i64, notebook: String, id: String) -> Result<Note> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                note(tx, &notebook, &id)
            })
            .await
    }
    pub(super) async fn list_notes(
        &self,
        owner: i64,
        notebook: String,
        options: ListOptions,
    ) -> Result<Page<Notes>> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                let mut q = Q::select("notebook_notes")
                    .columns(SUMMARY)
                    .and_where(C::equal("notebook_id", &notebook));
                if let Some(id) = &options.relationship {
                    if id == "none" {
                        q = q.and_where(C::is_null("folder_id"));
                    } else {
                        folder(tx, &notebook, id)?;
                        q = q.and_where(C::equal("folder_id", id.as_str()));
                    }
                }
                if let Some(text) = &options.query {
                    q = q.and_where(C::literal_substring("title", text));
                }
                let rows = tx.read_with(
                    q.order_by("updated_at", Sort::Descending)
                        .order_by("note_id", Sort::Ascending)
                        .limit(options.limit + 1)
                        .offset(options.offset)
                        .build(),
                    summary_row,
                )?;
                Ok(Page::from_rows(rows, &options, |notes| Notes { notes }))
            })
            .await
    }
    pub(super) async fn delete_note(&self, owner: i64, notebook: String, id: String) -> Result<()> {
        self.sqlite
            .transaction_result_async(move |tx| {
                owned(tx, owner, &notebook)?;
                note(tx, &notebook, &id)?;
                enqueue_cleanup(tx, owner, &id)?;
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
                )?;
                tx.write(
                    Q::delete_from("notebook_notes")
                        .and_where(C::equal("note_id", id))
                        .build(),
                )?;
                Ok(())
            })
            .await
    }
}
pub(super) fn relocate_folder(tx: &SqliteTransaction<'_>, id: &str) -> Result<()> {
    tx.write(
        Q::update("notebook_notes")
            .set("folder_id", None::<String>)
            .set("updated_at", now())
            .increment("revision")
            .and_where(C::equal("folder_id", id))
            .build(),
    )?;
    Ok(())
}
fn enqueue_cleanup(tx: &SqliteTransaction<'_>, owner: i64, note: &str) -> Result<()> {
    tx.write(
        Q::insert_into("notebook_cleanup")
            .value("note_id", note)
            .value("user_id", owner)
            .value("created_at", now())
            .on_conflict_do_nothing(["note_id"])
            .build(),
    )?;
    Ok(())
}
fn notebook_row(r: &Row<'_>) -> rusqlite::Result<Notebook> {
    Ok(Notebook {
        notebook_id: r.get("notebook_id")?,
        title: r.get("title")?,
        deal_id: r.get("deal_id")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}
fn folder_row(r: &Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        folder_id: r.get("folder_id")?,
        notebook_id: r.get("notebook_id")?,
        name: r.get("name")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}
fn summary_row(r: &Row<'_>) -> rusqlite::Result<NoteSummary> {
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
fn note_row(r: &Row<'_>) -> rusqlite::Result<Note> {
    let json: String = r.get("blocks_json")?;
    let blocks: Value = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(Note {
        summary: summary_row(r)?,
        blocks,
    })
}
#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/repository_tests.rs"]
mod tests;

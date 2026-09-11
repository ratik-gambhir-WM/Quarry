use serde::Serialize;

use crate::{
    adapters::sqlite::{
        client::{SqlRow, SqliteClient, SqliteClientError, SqliteTransaction},
        query::{Condition, ConflictUpdate, SortDirection, SqlBuilder, SqlQuery, SqlValue},
    },
    domains::documents::store::model::{
        ExistingFileVersion, FilePersistenceInput, PersistedFileIdentity,
    },
    shared::error::RepositoryError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealDocumentSummary {
    pub file_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDocumentBlob {
    pub file_id: String,
    pub display_name: String,
    pub mime_type: String,
    pub file_bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct DocumentStore {
    sqlite: SqliteClient,
}

impl DocumentStore {
    pub fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }

    pub async fn list_for_deal(
        &self,
        deal_id: &str,
    ) -> Result<Vec<DealDocumentSummary>, RepositoryError> {
        list_current_deal_documents(&self.sqlite, deal_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn current_blob(
        &self,
        deal_id: &str,
        file_id: &str,
    ) -> Result<Option<StoredDocumentBlob>, RepositoryError> {
        get_current_deal_document_blob(&self.sqlite, deal_id, file_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn find_current_by_hash(
        &self,
        deal_id: &str,
        workspace_id: &str,
        content_sha256: &str,
    ) -> Result<Option<PersistedFileIdentity>, RepositoryError> {
        find_current_sqlite_file_by_content_hash(
            &self.sqlite,
            deal_id,
            workspace_id,
            content_sha256,
        )
        .await
        .map_err(RepositoryError::storage)
    }

    pub(crate) async fn persist(
        &self,
        input: FilePersistenceInput,
    ) -> Result<PersistedFileIdentity, RepositoryError> {
        persist_file_blob(&self.sqlite, input)
            .await
            .map_err(RepositoryError::storage)
    }
}

async fn list_current_deal_documents(
    sqlite: &SqliteClient,
    deal_id: &str,
) -> Result<Vec<DealDocumentSummary>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns(["quarry_files.file_id", "quarry_files.display_name"])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        .and_where(Condition::equal("quarry_files.deal_id", deal_id))
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .order_by("quarry_files.display_name", SortDirection::Ascending)
        .order_by("quarry_files.file_id", SortDirection::Ascending)
        .build()
        .map_err(|error| format!("failed to build deal document list query: {error}"))?;
    let rows = sqlite
        .read_async(query)
        .await
        .map_err(|error| format!("failed to list deal documents: {error}"))?;

    rows.into_iter()
        .map(|row| {
            Ok(DealDocumentSummary {
                file_id: required_text(&row, "file_id")?,
                display_name: required_text(&row, "display_name")?,
            })
        })
        .collect::<Result<Vec<_>, SqliteClientError>>()
        .map_err(|error| format!("failed to decode deal document list: {error}"))
}

async fn get_current_deal_document_blob(
    sqlite: &SqliteClient,
    deal_id: &str,
    file_id: &str,
) -> Result<Option<StoredDocumentBlob>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.display_name",
            "quarry_file_versions.mime_type",
            "quarry_file_blobs.file_bytes",
        ])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        .inner_join(
            "quarry_file_blobs",
            "quarry_file_versions.version_id",
            "quarry_file_blobs.version_id",
        )
        .and_where(Condition::equal("quarry_files.deal_id", deal_id))
        .and_where(Condition::equal("quarry_files.file_id", file_id))
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .build()
        .map_err(|error| format!("failed to build document blob query: {error}"))?;
    let row = sqlite
        .read_one_async(query)
        .await
        .map_err(|error| format!("failed to load document blob: {error}"))?;

    row.map(|row| {
        Ok(StoredDocumentBlob {
            file_id: required_text(&row, "file_id")?,
            display_name: required_text(&row, "display_name")?,
            mime_type: required_text(&row, "mime_type")?,
            file_bytes: required_blob(&row, "file_bytes")?,
        })
    })
    .transpose()
    .map_err(|error: SqliteClientError| format!("failed to decode document blob: {error}"))
}

async fn find_current_sqlite_file_by_content_hash(
    sqlite: &SqliteClient,
    deal_id: &str,
    workspace_id: &str,
    content_sha256: &str,
) -> Result<Option<PersistedFileIdentity>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.workspace_id",
            "quarry_files.display_name",
            "quarry_file_versions.version_id",
        ])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        .inner_join(
            "quarry_file_blobs",
            "quarry_file_versions.version_id",
            "quarry_file_blobs.version_id",
        )
        .and_where(Condition::equal("quarry_files.deal_id", deal_id))
        .and_where(Condition::equal("quarry_files.workspace_id", workspace_id))
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .and_where(Condition::equal(
            "quarry_file_versions.content_sha256",
            content_sha256,
        ))
        .build()
        .map_err(|error| format!("failed to build deal attachment lookup: {error}"))?;
    let rows = sqlite
        .read_async(query)
        .await
        .map_err(|error| format!("failed to find deal attachment: {error}"))?;
    if rows.len() > 1 {
        return Err(format!(
            "deal `{deal_id}` contains multiple current attachments for content hash `{content_sha256}`"
        ));
    }

    rows.into_iter()
        .next()
        .map(|row| {
            Ok(PersistedFileIdentity {
                file_id: required_text(&row, "file_id")?,
                workspace_id: required_text(&row, "workspace_id")?,
                display_name: required_text(&row, "display_name")?,
                version_id: required_text(&row, "version_id")?,
            })
        })
        .transpose()
        .map_err(|error: SqliteClientError| format!("failed to decode deal attachment: {error}"))
}

async fn persist_file_blob(
    sqlite: &SqliteClient,
    file_persistence: FilePersistenceInput,
) -> Result<PersistedFileIdentity, String> {
    sqlite
        .transaction_async(move |transaction| {
            validate_deal_ownership(transaction, &file_persistence)?;

            // The aggregate is always mutated parent-to-child.
            validate_existing_quarry_file(transaction, &file_persistence)?;
            upsert_quarry_file(transaction, &file_persistence)?;

            if let Some(existing) = find_existing_file_version(transaction, &file_persistence)? {
                verify_idempotent_version_and_blob(transaction, &file_persistence, &existing)?;
                make_existing_version_current(transaction, &file_persistence, &existing)?;
            } else {
                let version_number = next_file_version_number(transaction, &file_persistence)?;

                clear_current_file_version(transaction, &file_persistence)?;
                insert_quarry_file_version(transaction, &file_persistence, version_number)?;
                insert_quarry_file_blob(transaction, &file_persistence)?;
            }

            read_back_persisted_file_identity(transaction, &file_persistence)
        })
        .await
        .map_err(|error| format!("failed to persist file transaction: {error}"))
}

fn normalize_stored_owner(value: &str) -> String {
    value.trim().to_lowercase()
}

fn validate_deal_ownership(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("deals")
            .columns(["deals.status", "users.email"])
            .inner_join("users", "deals.user_id", "users.id")
            .and_where(Condition::equal("deals.deal_id", &input.deal_id))
            .build(),
        "deal ownership select",
    )?;
    let row = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read deal ownership"))?
        .ok_or_else(|| {
            SqliteClientError::transaction_aborted(format!(
                "deal `{}` does not exist",
                input.deal_id
            ))
        })?;
    let status = required_text(&row, "status")?;
    if status.trim().eq_ignore_ascii_case("archived") {
        return Err(SqliteClientError::transaction_aborted(format!(
            "deal `{}` is archived",
            input.deal_id
        )));
    }
    let owner = normalize_stored_owner(&required_text(&row, "email")?);
    if owner != input.workspace_id {
        return Err(SqliteClientError::transaction_aborted(format!(
            "deal `{}` is not owned by workspace `{}`",
            input.deal_id, input.workspace_id
        )));
    }
    Ok(())
}

fn validate_existing_quarry_file(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_files")
            .columns(["deal_id", "workspace_id", "deleted_at"])
            .and_where(Condition::equal("file_id", &input.file_id))
            .build(),
        "existing quarry file select",
    )?;
    let Some(row) = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read existing quarry file"))?
    else {
        return Ok(());
    };

    if required_text(&row, "deal_id")? != input.deal_id {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` already belongs to another deal",
            input.file_id
        )));
    }
    if required_text(&row, "workspace_id")? != input.workspace_id {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` already belongs to another workspace",
            input.file_id
        )));
    }
    if !matches!(row.get("deleted_at"), Some(SqlValue::Null)) {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` is deleted and cannot be restored by persistence",
            input.file_id
        )));
    }
    Ok(())
}

fn upsert_quarry_file(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::insert_into("quarry_files")
            .value("file_id", &input.file_id)
            .value("deal_id", &input.deal_id)
            .value("workspace_id", &input.workspace_id)
            .value("display_name", &input.display_name)
            .value("source_uri", input.source_uri.clone())
            .value("metadata_json", &input.metadata_json)
            .value("created_at", &input.timestamp)
            .value("updated_at", &input.timestamp)
            .value("deleted_at", Option::<String>::None)
            .on_conflict_update(
                ConflictUpdate::new(["file_id"])
                    .set_excluded("display_name")
                    .set_excluded("source_uri")
                    .set_excluded("metadata_json")
                    .set_excluded("updated_at"),
            )
            .build(),
        "quarry_files upsert",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to upsert quarry file"))?
            .rows_affected,
        "quarry_files upsert",
    )
}

fn find_existing_file_version(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<Option<ExistingFileVersion>, SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_file_versions")
            .columns(["version_id", "byte_size", "is_current"])
            .and_where(Condition::equal("file_id", &input.file_id))
            .and_where(Condition::equal("content_sha256", &input.content_sha256))
            .build(),
        "existing file version select",
    )?;
    transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read existing file version"))?
        .map(|row| {
            let is_current = required_integer(&row, "is_current")?;
            if !matches!(is_current, 0 | 1) {
                return Err(SqliteClientError::transaction_aborted(
                    "stored file version has an invalid is_current value",
                ));
            }
            Ok(ExistingFileVersion {
                version_id: required_text(&row, "version_id")?,
                byte_size: required_integer(&row, "byte_size")?,
                is_current: is_current == 1,
            })
        })
        .transpose()
}

fn verify_idempotent_version_and_blob(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
    existing: &ExistingFileVersion,
) -> Result<(), SqliteClientError> {
    if existing.version_id != input.version_id || existing.byte_size != input.byte_size {
        return Err(SqliteClientError::transaction_aborted(format!(
            "stored version for file `{}` does not match its derived identity or byte size",
            input.file_id
        )));
    }
    let query = build_transaction_query(
        SqlBuilder::select("quarry_file_blobs")
            .column("file_bytes")
            .and_where(Condition::equal("version_id", &existing.version_id))
            .build(),
        "existing file blob select",
    )?;
    let row = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read existing file blob"))?
        .ok_or_else(|| {
            SqliteClientError::transaction_aborted(format!(
                "stored version `{}` has no blob",
                existing.version_id
            ))
        })?;
    if required_blob(&row, "file_bytes")? != input.file_bytes {
        return Err(SqliteClientError::transaction_aborted(format!(
            "stored blob for version `{}` is corrupt or collided",
            existing.version_id
        )));
    }
    Ok(())
}

fn make_existing_version_current(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
    existing: &ExistingFileVersion,
) -> Result<(), SqliteClientError> {
    if existing.is_current {
        return Ok(());
    }
    clear_current_file_version(transaction, input)?;
    let query = build_transaction_query(
        SqlBuilder::update("quarry_file_versions")
            .set("is_current", true)
            .and_where(Condition::equal("version_id", &existing.version_id))
            .and_where(Condition::equal("file_id", &input.file_id))
            .build(),
        "existing file version current update",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to make existing file version current"))?
            .rows_affected,
        "existing file version current update",
    )
}

fn next_file_version_number(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<i64, SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_file_versions")
            .column("version_number")
            .and_where(Condition::equal("file_id", &input.file_id))
            .order_by("version_number", SortDirection::Descending)
            .limit(1)
            .build(),
        "latest file version select",
    )?;
    let latest = transaction
        .read_one(&query)
        .map_err(|error| error.context("failed to read latest file version"))?
        .map(|row| required_integer(&row, "version_number"))
        .transpose()?;
    match latest {
        Some(version) if version > 0 => version.checked_add(1).ok_or_else(|| {
            SqliteClientError::transaction_aborted(format!(
                "version number overflow for file `{}`",
                input.file_id
            ))
        }),
        Some(_) => Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` has an invalid stored version number",
            input.file_id
        ))),
        None => Ok(1),
    }
}

fn clear_current_file_version(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::update("quarry_file_versions")
            .set("is_current", false)
            .and_where(Condition::equal("file_id", &input.file_id))
            .and_where(Condition::equal("is_current", true))
            .build(),
        "current file version clear",
    )?;
    let rows_affected = transaction
        .write(&query)
        .map_err(|error| error.context("failed to clear current file version"))?
        .rows_affected;
    if rows_affected > 1 {
        return Err(SqliteClientError::transaction_aborted(format!(
            "file `{}` had more than one current version",
            input.file_id
        )));
    }
    Ok(())
}

fn insert_quarry_file_version(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
    version_number: i64,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::insert_into("quarry_file_versions")
            .value("version_id", &input.version_id)
            .value("file_id", &input.file_id)
            .value("version_number", version_number)
            .value("original_filename", &input.display_name)
            .value("mime_type", &input.mime_type)
            .value("content_sha256", &input.content_sha256)
            .value("byte_size", input.byte_size)
            .value("is_current", true)
            .value("created_at", &input.timestamp)
            .build(),
        "quarry_file_versions insert",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to insert quarry file version"))?
            .rows_affected,
        "quarry_file_versions insert",
    )
}

fn insert_quarry_file_blob(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<(), SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::insert_into("quarry_file_blobs")
            .value("version_id", &input.version_id)
            .value("file_bytes", input.file_bytes.clone())
            .build(),
        "quarry_file_blobs insert",
    )?;
    require_one_row(
        transaction
            .write(&query)
            .map_err(|error| error.context("failed to insert quarry file blob"))?
            .rows_affected,
        "quarry_file_blobs insert",
    )
}

fn read_back_persisted_file_identity(
    transaction: &SqliteTransaction<'_>,
    input: &FilePersistenceInput,
) -> Result<PersistedFileIdentity, SqliteClientError> {
    let query = build_transaction_query(
        SqlBuilder::select("quarry_files")
            .columns([
                "quarry_files.file_id",
                "quarry_files.workspace_id",
                "quarry_files.display_name",
                "quarry_file_versions.version_id",
            ])
            .inner_join(
                "quarry_file_versions",
                "quarry_files.file_id",
                "quarry_file_versions.file_id",
            )
            .and_where(Condition::equal("quarry_files.file_id", &input.file_id))
            .and_where(Condition::equal(
                "quarry_file_versions.version_id",
                &input.version_id,
            ))
            .build(),
        "persisted file identity read-back",
    )?;
    let rows = transaction
        .read(&query)
        .map_err(|error| error.context("failed to read back persisted file identity"))?;
    if rows.len() != 1 {
        return Err(SqliteClientError::transaction_aborted(format!(
            "persisted file identity read-back returned {} rows instead of one",
            rows.len()
        )));
    }
    let row = &rows[0];
    let identity = PersistedFileIdentity {
        file_id: required_text(row, "file_id")?,
        workspace_id: required_text(row, "workspace_id")?,
        display_name: required_text(row, "display_name")?,
        version_id: required_text(row, "version_id")?,
    };
    if identity.file_id != input.file_id
        || identity.workspace_id != input.workspace_id
        || identity.display_name != input.display_name
        || identity.version_id != input.version_id
    {
        return Err(SqliteClientError::transaction_aborted(
            "persisted file identity read-back did not match the validated input",
        ));
    }
    Ok(identity)
}

fn build_transaction_query(
    query: Result<SqlQuery, crate::adapters::sqlite::query::SqlBuilderError>,
    operation: &str,
) -> Result<SqlQuery, SqliteClientError> {
    query.map_err(|error| {
        SqliteClientError::transaction_aborted(format!("failed to build {operation}: {error}"))
    })
}

fn require_one_row(rows_affected: usize, operation: &str) -> Result<(), SqliteClientError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(SqliteClientError::transaction_aborted(format!(
            "{operation} affected {rows_affected} rows instead of one"
        )))
    }
}

fn required_text(row: &SqlRow, column: &str) -> Result<String, SqliteClientError> {
    match row.get(column) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(invalid_row_value(column, "TEXT")),
    }
}

fn required_integer(row: &SqlRow, column: &str) -> Result<i64, SqliteClientError> {
    match row.get(column) {
        Some(SqlValue::Integer(value)) => Ok(*value),
        _ => Err(invalid_row_value(column, "INTEGER")),
    }
}

fn required_blob(row: &SqlRow, column: &str) -> Result<Vec<u8>, SqliteClientError> {
    match row.get(column) {
        Some(SqlValue::Blob(value)) => Ok(value.clone()),
        _ => Err(invalid_row_value(column, "BLOB")),
    }
}

fn invalid_row_value(column: &str, expected: &str) -> SqliteClientError {
    SqliteClientError::transaction_aborted(format!(
        "transaction row column `{column}` was missing or was not {expected}"
    ))
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/store/sqlite_tests.rs"]
mod tests;

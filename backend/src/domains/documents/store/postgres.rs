use serde::Serialize;

use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, PostgresTransaction, SqlRow},
        query::{Condition, ConflictUpdate, SortDirection, SqlBuilder, SqlValue},
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
    pub uploaded_by_user_id: Option<i64>,
    pub uploaded_by_name: Option<String>,
    pub attribution_origin: String,
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
    postgres: PostgresClient,
}

impl DocumentStore {
    pub fn new(postgres: PostgresClient) -> Self {
        Self { postgres }
    }

    pub async fn manifest(
        &self,
        version: String,
    ) -> Result<Option<super::identity::IdentityManifest>, RepositoryError> {
        super::identity::manifest(&self.postgres, version).await
    }
    pub(crate) async fn image_assembly(
        &self,
        version: &str,
    ) -> Result<Option<crate::domains::documents::formats::image::ImageAssembly>, RepositoryError>
    {
        let json = self
            .postgres
            .read_one_with_async(
                SqlBuilder::select("document_image_recovery")
                    .columns(["assembly_json"])
                    .and_where(Condition::equal("version_id", version))
                    .build(),
                |r| r.get::<_, String>(0),
            )
            .await?;
        json.map(|json| {
            serde_json::from_str(&json).map_err(|e| RepositoryError::storage(e.to_string()))
        })
        .transpose()
    }
    pub async fn authorize_read(&self, deal: &str, actor: i64) -> Result<(), RepositoryError> {
        let deal = deal.to_string();
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                crate::domains::memberships::require_deal(
                    tx,
                    &deal,
                    actor,
                    crate::domains::memberships::model::Capability::Read,
                )
                .await
                .map(|_| ())
            })
            .await
    }
    pub async fn authorize_upload(&self, deal: &str, actor: i64) -> Result<(), RepositoryError> {
        let deal = deal.to_string();
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                crate::domains::memberships::require_deal(
                    tx,
                    &deal,
                    actor,
                    crate::domains::memberships::model::Capability::Upload,
                )
                .await
                .map(|_| ())
            })
            .await
    }
    pub async fn list_for_deal(
        &self,
        deal_id: &str,
    ) -> Result<Vec<DealDocumentSummary>, RepositoryError> {
        list_current_deal_documents(&self.postgres, deal_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn current_blob(
        &self,
        deal_id: &str,
        file_id: &str,
    ) -> Result<Option<StoredDocumentBlob>, RepositoryError> {
        get_current_deal_document_blob(&self.postgres, deal_id, file_id)
            .await
            .map_err(RepositoryError::storage)
    }

    pub async fn find_current_by_hash(
        &self,
        deal_id: &str,
        content_sha256: &str,
    ) -> Result<Option<PersistedFileIdentity>, RepositoryError> {
        find_current_postgres_file_by_content_hash(&self.postgres, deal_id, content_sha256)
            .await
            .map_err(RepositoryError::storage)
    }

    pub(crate) async fn persist(
        &self,
        input: FilePersistenceInput,
    ) -> Result<PersistedFileIdentity, RepositoryError> {
        persist_file_blob(&self.postgres, input).await
    }
}

async fn list_current_deal_documents(
    postgres: &PostgresClient,
    deal_id: &str,
) -> Result<Vec<DealDocumentSummary>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.display_name",
            "quarry_file_versions.uploaded_by_user_id",
            "users.first_name",
            "users.last_name",
        ])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        .left_join(
            "users",
            "quarry_file_versions.uploaded_by_user_id",
            "users.id",
        )
        .and_where(Condition::equal("quarry_files.deal_id", deal_id))
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .order_by("quarry_files.display_name", SortDirection::Ascending)
        .order_by("quarry_files.file_id", SortDirection::Ascending)
        .build();
    let rows = postgres
        .read_async(query)
        .await
        .map_err(|error| format!("failed to list deal documents: {error}"))?;

    rows.into_iter()
        .map(|row| {
            Ok(DealDocumentSummary {
                file_id: required_text(&row, "file_id")?,
                display_name: required_text(&row, "display_name")?,
                uploaded_by_user_id: match row.get("uploaded_by_user_id") {
                    Some(SqlValue::Integer(id)) => Some(*id),
                    _ => None,
                },
                uploaded_by_name: match (row.get("first_name"), row.get("last_name")) {
                    (Some(SqlValue::Text(a)), Some(SqlValue::Text(b))) => Some(format!("{a} {b}")),
                    _ => None,
                },
                attribution_origin: if matches!(
                    row.get("uploaded_by_user_id"),
                    Some(SqlValue::Integer(_))
                ) {
                    "authenticated"
                } else {
                    "legacy_unknown"
                }
                .into(),
            })
        })
        .collect::<Result<Vec<_>, PostgresError>>()
        .map_err(|error| format!("failed to decode deal document list: {error}"))
}

async fn get_current_deal_document_blob(
    postgres: &PostgresClient,
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
        .build();
    let row = postgres
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
    .map_err(|error: PostgresError| format!("failed to decode document blob: {error}"))
}

async fn find_current_postgres_file_by_content_hash(
    postgres: &PostgresClient,
    deal_id: &str,
    content_sha256: &str,
) -> Result<Option<PersistedFileIdentity>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.deal_id",
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
        .and_where(Condition::is_null("quarry_files.deleted_at"))
        .and_where(Condition::equal("quarry_file_versions.is_current", true))
        .and_where(Condition::equal(
            "quarry_file_versions.content_sha256",
            content_sha256,
        ))
        .build();
    let rows = postgres
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
                deal_id: required_text(&row, "deal_id")?,
                display_name: required_text(&row, "display_name")?,
                version_id: required_text(&row, "version_id")?,
            })
        })
        .transpose()
        .map_err(|error: PostgresError| format!("failed to decode deal attachment: {error}"))
}

async fn persist_file_blob(
    postgres: &PostgresClient,
    file_persistence: FilePersistenceInput,
) -> Result<PersistedFileIdentity, RepositoryError> {
    postgres
        .transaction_result_async(move |transaction| async move {
            let transaction = &transaction;
            crate::domains::deals::coordination::lock_authority(
                transaction,
                &file_persistence.deal_id,
                crate::adapters::postgres::query::RowLock::Share,
            )
            .await?;
            crate::domains::memberships::require_deal(
                transaction,
                &file_persistence.deal_id,
                file_persistence.actor_id,
                crate::domains::memberships::model::Capability::Upload,
            )
            .await?;

            if let Some(claim) = transaction
                .read_one_with(
                    SqlBuilder::select("quarry_current_content")
                        .columns(["file_id"])
                        .and_where(Condition::equal("deal_id", &file_persistence.deal_id))
                        .and_where(Condition::equal(
                            "content_sha256",
                            &file_persistence.content_sha256,
                        ))
                        .build(),
                    |r| r.get::<_, String>(0),
                )
                .await?
            {
                if claim != file_persistence.file_id {
                    return Err(RepositoryError::Conflict(
                        "identical content was accepted concurrently; retry the same file".into(),
                    ));
                }
            }
            // The aggregate is always mutated parent-to-child.
            validate_existing_quarry_file(transaction, &file_persistence).await?;
            upsert_quarry_file(transaction, &file_persistence).await?;

            if let Some(existing) =
                find_existing_file_version(transaction, &file_persistence).await?
            {
                verify_idempotent_version_and_blob(transaction, &file_persistence, &existing)
                    .await?;
                make_existing_version_current(transaction, &file_persistence, &existing).await?;
            } else {
                let version_number =
                    next_file_version_number(transaction, &file_persistence).await?;

                clear_current_file_version(transaction, &file_persistence).await?;
                insert_quarry_file_version(transaction, &file_persistence, version_number).await?;
                insert_quarry_file_blob(transaction, &file_persistence).await?;
                crate::domains::memberships::record_activity(
                    transaction,
                    &file_persistence.deal_id,
                    Some(file_persistence.actor_id),
                    "document_uploaded",
                    Some(&file_persistence.version_id),
                )
                .await?;
            }

            transaction
                .write(
                    SqlBuilder::delete_from("quarry_current_content")
                        .and_where(Condition::equal("file_id", &file_persistence.file_id))
                        .build(),
                )
                .await?;
            transaction
                .write(
                    SqlBuilder::insert_into("quarry_current_content")
                        .value("deal_id", &file_persistence.deal_id)
                        .value("content_sha256", &file_persistence.content_sha256)
                        .value("file_id", &file_persistence.file_id)
                        .value("version_id", &file_persistence.version_id)
                        .build(),
                )
                .await?;
            let manifest = file_persistence
                .manifest
                .as_ref()
                .ok_or_else(|| PostgresError::transaction_aborted("identity manifest required"))?;
            super::identity::save(transaction, &file_persistence.version_id, manifest, true)
                .await
                .map_err(|e| PostgresError::transaction_aborted(e.to_string()))?;
            if let Some(assembly) = &file_persistence.image_assembly_json {
                let existing = transaction
                    .read_one_with(
                        SqlBuilder::select("document_image_recovery")
                            .columns(["assembly_json"])
                            .and_where(Condition::equal("version_id", &file_persistence.version_id))
                            .build(),
                        |r| r.get::<_, String>(0),
                    )
                    .await?;
                match existing {
                    Some(existing) if existing != *assembly => {
                        return Err(RepositoryError::Conflict(
                            "image recovery text differs from immutable version".into(),
                        ))
                    }
                    Some(_) => {}
                    None => {
                        transaction
                            .write(
                                SqlBuilder::insert_into("document_image_recovery")
                                    .value("version_id", &file_persistence.version_id)
                                    .value("assembly_json", assembly)
                                    .build(),
                            )
                            .await?;
                    }
                }
            }
            Ok(read_back_persisted_file_identity(transaction, &file_persistence).await?)
        })
        .await
}

async fn validate_existing_quarry_file(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
) -> Result<(), PostgresError> {
    let query = SqlBuilder::select("quarry_files")
        .columns(["deal_id", "deleted_at"])
        .and_where(Condition::equal("file_id", &input.file_id))
        .build();
    let Some(row) = transaction
        .read_one(query)
        .await
        .map_err(|error| error.context("failed to read existing quarry file"))?
    else {
        return Ok(());
    };

    if required_text(&row, "deal_id")? != input.deal_id {
        return Err(PostgresError::transaction_aborted(format!(
            "file `{}` already belongs to another deal",
            input.file_id
        )));
    }
    if !matches!(row.get("deleted_at"), Some(SqlValue::Null)) {
        return Err(PostgresError::transaction_aborted(format!(
            "file `{}` is deleted and cannot be restored by persistence",
            input.file_id
        )));
    }
    Ok(())
}

async fn upsert_quarry_file(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
) -> Result<(), PostgresError> {
    let query = SqlBuilder::insert_into("quarry_files")
        .value("file_id", &input.file_id)
        .value("deal_id", &input.deal_id)
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
        .build();
    require_one_row(
        transaction
            .write(query)
            .await
            .map_err(|error| error.context("failed to upsert quarry file"))?
            .rows_affected,
        "quarry_files upsert",
    )
}

async fn find_existing_file_version(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
) -> Result<Option<ExistingFileVersion>, PostgresError> {
    let query = SqlBuilder::select("quarry_file_versions")
        .columns(["version_id", "byte_size", "is_current"])
        .and_where(Condition::equal("file_id", &input.file_id))
        .and_where(Condition::equal("content_sha256", &input.content_sha256))
        .build();
    transaction
        .read_one(query)
        .await
        .map_err(|error| error.context("failed to read existing file version"))?
        .map(|row| {
            let is_current = required_integer(&row, "is_current")?;
            if !matches!(is_current, 0 | 1) {
                return Err(PostgresError::transaction_aborted(
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

async fn verify_idempotent_version_and_blob(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
    existing: &ExistingFileVersion,
) -> Result<(), PostgresError> {
    if existing.version_id != input.version_id || existing.byte_size != input.byte_size {
        return Err(PostgresError::transaction_aborted(format!(
            "stored version for file `{}` does not match its derived identity or byte size",
            input.file_id
        )));
    }
    let query = SqlBuilder::select("quarry_file_blobs")
        .column("file_bytes")
        .and_where(Condition::equal("version_id", &existing.version_id))
        .build();
    let row = transaction
        .read_one(query)
        .await
        .map_err(|error| error.context("failed to read existing file blob"))?
        .ok_or_else(|| {
            PostgresError::transaction_aborted(format!(
                "stored version `{}` has no blob",
                existing.version_id
            ))
        })?;
    if required_blob(&row, "file_bytes")? != input.file_bytes {
        return Err(PostgresError::transaction_aborted(format!(
            "stored blob for version `{}` is corrupt or collided",
            existing.version_id
        )));
    }
    Ok(())
}

async fn make_existing_version_current(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
    existing: &ExistingFileVersion,
) -> Result<(), PostgresError> {
    if existing.is_current {
        return Ok(());
    }
    clear_current_file_version(transaction, input).await?;
    let query = SqlBuilder::update("quarry_file_versions")
        .set("is_current", true)
        .and_where(Condition::equal("version_id", &existing.version_id))
        .and_where(Condition::equal("file_id", &input.file_id))
        .build();
    require_one_row(
        transaction
            .write(query)
            .await
            .map_err(|error| error.context("failed to make existing file version current"))?
            .rows_affected,
        "existing file version current update",
    )
}

async fn next_file_version_number(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
) -> Result<i64, PostgresError> {
    let query = SqlBuilder::select("quarry_file_versions")
        .column("version_number")
        .and_where(Condition::equal("file_id", &input.file_id))
        .order_by("version_number", SortDirection::Descending)
        .limit(1)
        .build();
    let latest = transaction
        .read_one(query)
        .await
        .map_err(|error| error.context("failed to read latest file version"))?
        .map(|row| required_integer(&row, "version_number"))
        .transpose()?;
    match latest {
        Some(version) if version > 0 => version.checked_add(1).ok_or_else(|| {
            PostgresError::transaction_aborted(format!(
                "version number overflow for file `{}`",
                input.file_id
            ))
        }),
        Some(_) => Err(PostgresError::transaction_aborted(format!(
            "file `{}` has an invalid stored version number",
            input.file_id
        ))),
        None => Ok(1),
    }
}

async fn clear_current_file_version(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
) -> Result<(), PostgresError> {
    let query = SqlBuilder::update("quarry_file_versions")
        .set("is_current", false)
        .and_where(Condition::equal("file_id", &input.file_id))
        .and_where(Condition::equal("is_current", true))
        .build();
    let rows_affected = transaction
        .write(query)
        .await
        .map_err(|error| error.context("failed to clear current file version"))?
        .rows_affected;
    if rows_affected > 1 {
        return Err(PostgresError::transaction_aborted(format!(
            "file `{}` had more than one current version",
            input.file_id
        )));
    }
    Ok(())
}

async fn insert_quarry_file_version(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
    version_number: i64,
) -> Result<(), PostgresError> {
    let query = SqlBuilder::insert_into("quarry_file_versions")
        .value("version_id", &input.version_id)
        .value("uploaded_by_user_id", input.actor_id)
        .value("file_id", &input.file_id)
        .value("version_number", version_number)
        .value("original_filename", &input.display_name)
        .value("mime_type", &input.mime_type)
        .value("content_sha256", &input.content_sha256)
        .value("byte_size", input.byte_size)
        .value("is_current", true)
        .value("created_at", &input.timestamp)
        .build();
    require_one_row(
        transaction
            .write(query)
            .await
            .map_err(|error| error.context("failed to insert quarry file version"))?
            .rows_affected,
        "quarry_file_versions insert",
    )
}

async fn insert_quarry_file_blob(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
) -> Result<(), PostgresError> {
    let query = SqlBuilder::insert_into("quarry_file_blobs")
        .value("version_id", &input.version_id)
        .value("file_bytes", input.file_bytes.clone())
        .build();
    require_one_row(
        transaction
            .write(query)
            .await
            .map_err(|error| error.context("failed to insert quarry file blob"))?
            .rows_affected,
        "quarry_file_blobs insert",
    )
}

async fn read_back_persisted_file_identity(
    transaction: &PostgresTransaction,
    input: &FilePersistenceInput,
) -> Result<PersistedFileIdentity, PostgresError> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.deal_id",
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
        .build();
    let rows = transaction
        .read(query)
        .await
        .map_err(|error| error.context("failed to read back persisted file identity"))?;
    if rows.len() != 1 {
        return Err(PostgresError::transaction_aborted(format!(
            "persisted file identity read-back returned {} rows instead of one",
            rows.len()
        )));
    }
    let row = &rows[0];
    let identity = PersistedFileIdentity {
        file_id: required_text(row, "file_id")?,
        deal_id: required_text(row, "deal_id")?,
        display_name: required_text(row, "display_name")?,
        version_id: required_text(row, "version_id")?,
    };
    if identity.file_id != input.file_id
        || identity.deal_id != input.deal_id
        || identity.display_name != input.display_name
        || identity.version_id != input.version_id
    {
        return Err(PostgresError::transaction_aborted(
            "persisted file identity read-back did not match the validated input",
        ));
    }
    Ok(identity)
}

fn require_one_row(rows_affected: usize, operation: &str) -> Result<(), PostgresError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(PostgresError::transaction_aborted(format!(
            "{operation} affected {rows_affected} rows instead of one"
        )))
    }
}

fn required_text(row: &SqlRow, column: &str) -> Result<String, PostgresError> {
    match row.get(column) {
        Some(SqlValue::Text(value)) => Ok(value.clone()),
        _ => Err(invalid_row_value(column, "TEXT")),
    }
}

fn required_integer(row: &SqlRow, column: &str) -> Result<i64, PostgresError> {
    match row.get(column) {
        Some(SqlValue::Integer(value)) => Ok(*value),
        _ => Err(invalid_row_value(column, "INTEGER")),
    }
}

fn required_blob(row: &SqlRow, column: &str) -> Result<Vec<u8>, PostgresError> {
    match row.get(column) {
        Some(SqlValue::Blob(value)) => Ok(value.clone()),
        _ => Err(invalid_row_value(column, "BLOB")),
    }
}

fn invalid_row_value(column: &str, expected: &str) -> PostgresError {
    PostgresError::transaction_aborted(format!(
        "transaction row column `{column}` was missing or was not {expected}"
    ))
}

#[cfg(test)]
#[path = "../../../../tests/integration/domains/documents/store/postgres_tests.rs"]
mod tests;

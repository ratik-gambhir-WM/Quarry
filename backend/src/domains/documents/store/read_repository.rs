//! Current-document queries and row decoding for the DocumentStore facade.
use super::model::PersistedFileIdentity;
use super::repository::{required_integer, required_text, DealDocumentSummary, StoredDocumentBlob};
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, SqlRow},
        query::{Condition, JoinClause, JoinCondition, SortDirection, SqlBuilder, SqlValue},
    },
    domains::documents::classification::model::DocumentClassification,
};

pub(super) async fn list_current_deal_documents(
    postgres: &PostgresClient,
    deal_id: &str,
) -> Result<Vec<DealDocumentSummary>, String> {
    let query = SqlBuilder::select("quarry_files")
        .columns([
            "quarry_files.file_id",
            "quarry_files.display_name",
            "quarry_files.created_at",
            "quarry_file_versions.content_sha256",
            "quarry_file_versions.version_id",
            "quarry_file_versions.byte_size",
            "quarry_file_versions.uploaded_by_user_id",
            "users.first_name",
            "users.last_name",
            "document_classifications.run_status",
            "document_classifications.generation",
            "document_classifications.last_error_code",
            "document_classifications.result_json",
            "document_classifications.completed_at",
        ])
        .inner_join(
            "quarry_file_versions",
            "quarry_files.file_id",
            "quarry_file_versions.file_id",
        )
        // One snapshot read: enrichment joins the exact current content identity only.
        .left_join(
            "document_input_identities",
            "quarry_files.file_id",
            "document_input_identities.file_id",
        )
        .join(
            JoinClause::left("document_classifications")
                .on(JoinCondition::equal(
                    "document_classifications.deal_id",
                    "document_input_identities.deal_id",
                ))
                .and_on(JoinCondition::equal(
                    "document_classifications.input_hash",
                    "quarry_file_versions.content_sha256",
                )),
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
                added_at: required_text(&row, "created_at")?,
                file_id: required_text(&row, "file_id")?,
                content_sha256: required_text(&row, "content_sha256")?,
                display_name: required_text(&row, "display_name")?,
                size_bytes: required_integer(&row, "byte_size")?,
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
                classification: classification_view(&row)?,
            })
        })
        .collect::<Result<Vec<_>, PostgresError>>()
        .map_err(|error| format!("failed to decode deal document list: {error}"))
}

pub(super) async fn get_current_deal_document_blob(
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
    postgres
        .read_one_with_async(query, |row| {
            Ok(StoredDocumentBlob {
                file_id: row.get("file_id")?,
                display_name: row.get("display_name")?,
                mime_type: row.get("mime_type")?,
                file_bytes: row.get("file_bytes")?,
            })
        })
        .await
        .map_err(|error| format!("failed to load document blob: {error}"))
}

pub(super) async fn find_current_postgres_file_by_content_hash(
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

fn optional_text(row: &SqlRow, column: &str) -> Option<String> {
    match row.get(column) {
        Some(SqlValue::Text(value)) => Some(value.clone()),
        _ => None,
    }
}

fn optional_integer(row: &SqlRow, column: &str) -> Option<i64> {
    match row.get(column) {
        Some(SqlValue::Integer(value)) => Some(*value),
        _ => None,
    }
}

/// Public classification metadata; raw samples and claim tokens are never projected.
fn classification_view(row: &SqlRow) -> Result<DocumentClassification, PostgresError> {
    let result_json = optional_text(row, "result_json");
    Ok(super::super::classification::view::from_stored(
        required_text(row, "version_id")?,
        optional_text(row, "run_status"),
        optional_integer(row, "generation"),
        optional_text(row, "last_error_code"),
        result_json.as_deref(),
        optional_integer(row, "completed_at"),
    ))
}

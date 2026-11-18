//! One user-scoped PostgreSQL repository with focused persistence capabilities.
//!
//! `contracts` defines the consumer interfaces. Catalog, processing and retrieval
//! implement them on the same repository. Foreground operations use an immutable
//! authenticated owner scope; the unscoped instance is reserved for global worker
//! recovery. Publication helpers retain the caller's transaction and ownership guards.
mod catalog_repository;
mod contracts;
mod processing_repository;
mod retrieval_repository;
use super::{compact_document, models::*, normalized_document, retrieval::*};
use crate::adapters::postgres::client::RowResult;
use crate::adapters::postgres::{
    client::{PostgresClient, PostgresTransaction},
    query::{
        Condition, ConflictUpdate, JoinClause, JoinCondition, LockWait, RowLock, SortDirection,
        SqlBuilder,
    },
};
use crate::shared::error::RepositoryError;
use anyhow::{ensure, Context};
use std::sync::Arc;
pub type RepositoryResult<T> = Result<T, RepositoryError>;
const MAX_STORED_PREVIEW_BYTES: usize = 16 * 1024 * 1024;
const MAX_STORED_PREVIEW_DIMENSION: u32 = 10_000;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
pub use contracts::{
    TemplateAssetRepository, TemplateCatalogRepository, TemplateProcessingRepository,
    TemplateRepository, TemplateRetrievalRepository,
};

#[derive(Clone)]
pub struct PostgresTemplateRepository {
    client: PostgresClient,
    owner_user_id: Option<i64>,
}
impl PostgresTemplateRepository {
    pub(crate) fn user_scope(&self, user_id: i64) -> RepositoryResult<Self> {
        if user_id <= 0 || self.owner_user_id.is_some_and(|owner| owner != user_id) {
            return Err(RepositoryError::Forbidden);
        }
        Ok(Self {
            client: self.client.clone(),
            owner_user_id: Some(user_id),
        })
    }

    pub fn new(client: PostgresClient) -> Self {
        Self {
            client,
            owner_user_id: None,
        }
    }
}
fn validate_records(
    template: &StoredTemplate,
    assets: &[TemplateAsset],
    metadata: &StoredTemplateMetadata,
    preview: Option<&StoredTemplatePreview>,
) -> anyhow::Result<()> {
    ensure!(
        metadata.template_id == template.template_id
            && preview.is_none_or(|preview| preview.template_id == template.template_id),
        "template records must use the same template ID"
    );
    ensure!(
        assets
            .iter()
            .all(|asset| asset.template_id == template.template_id),
        "a template asset cannot be stored under a different template"
    );
    if let Some(preview) = preview {
        ensure!(
            preview.content_type == "image/png"
                && preview.bytes.len() >= 24
                && preview.bytes.len() <= MAX_STORED_PREVIEW_BYTES
                && preview.bytes.starts_with(PNG_SIGNATURE),
            "template previews must contain a valid bounded PNG image"
        );
        let dimensions = png_dimensions(&preview.bytes)
            .context("template preview dimensions could not be read")?;
        ensure!(
            dimensions == (preview.width, preview.height)
                && preview.width <= MAX_STORED_PREVIEW_DIMENSION
                && preview.height <= MAX_STORED_PREVIEW_DIMENSION,
            "template preview dimensions do not match the PNG image"
        );
    }
    Ok(())
}

type MetadataRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
);

fn map_metadata(row: MetadataRow) -> anyhow::Result<StoredTemplateMetadata> {
    let kind = match row.1.as_str() {
        "diagram" => TemplateKind::Diagram,
        "commentary" => TemplateKind::Commentary,
        _ => anyhow::bail!("stored template kind is invalid"),
    };
    let source = match row.4.as_str() {
        "builtin" => TemplateSource::Builtin,
        "import" => TemplateSource::Import,
        _ => anyhow::bail!("stored template source is invalid"),
    };
    Ok(StoredTemplateMetadata {
        template_id: row.0,
        kind,
        title: row.2,
        description: row.3,
        source,
        created_at: timestamp_millis(&row.5)?,
        catalog_version: row.6,
        catalog_checksum: row.7,
    })
}

pub(super) fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(PNG_SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

async fn decode_document(json: String) -> anyhow::Result<CanvasDocument> {
    tokio::task::spawn_blocking(move || {
        let value = serde_json::from_str(&json)?;
        Ok(super::normalize::normalize(&value)?.0)
    })
    .await?
}

fn timestamp_iso(millis: i64) -> anyhow::Result<String> {
    Ok(chrono::DateTime::from_timestamp_millis(millis)
        .context("invalid creation time")?
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}
fn timestamp_millis(value: &str) -> anyhow::Result<i64> {
    if let Ok(time) = chrono::DateTime::parse_from_rfc3339(value) {
        return Ok(time.timestamp_millis());
    }
    Ok(
        chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")?
            .and_utc()
            .timestamp_millis(),
    )
}

#[cfg(test)]
#[path = "../../../../../tests/unit/domains/templates/slide_templates/repository/tests.rs"]
mod tests;
impl PostgresTemplateRepository {
    async fn write_new(
        &self,
        tx: &PostgresTransaction,
        template: &StoredTemplate,
        assets: &[TemplateAsset],
        metadata: &StoredTemplateMetadata,
        preview: Option<&StoredTemplatePreview>,
    ) -> anyhow::Result<()> {
        validate_records(template, assets, metadata, preview)?;
        let json = serde_json::to_string(&if metadata.source == TemplateSource::Builtin {
            normalized_document(&template.template_json)
        } else {
            compact_document(&template.template_json)
        })?;
        tx.write(
            SqlBuilder::insert_into("templates")
                .value("template_id", &template.template_id)
                .value("template_json", json)
                .value(
                    "owner_user_id",
                    self.owner_user_id
                        .context("template owner scope is required")?,
                )
                .build(),
        )
        .await?;
        write_metadata(tx, metadata, false).await?;
        insert_assets(tx, assets).await?;
        write_preview(tx, preview).await
    }
}
impl PostgresTemplateRepository {
    fn owner_condition(&self) -> Condition {
        match self.owner_user_id {
            Some(owner) => Condition::equal("templates.owner_user_id", owner),
            None => Condition::is_not_null("templates.owner_user_id"),
        }
    }
    async fn visible(&self, tx: &PostgresTransaction, id: &str) -> anyhow::Result<bool> {
        Ok(tx
            .read_one(
                SqlBuilder::select("templates")
                    .column("template_id")
                    .and_where(Condition::equal("template_id", id))
                    .and_where(self.owner_condition())
                    .build(),
            )
            .await?
            .is_some())
    }
}

async fn write_metadata(
    tx: &PostgresTransaction,
    m: &StoredTemplateMetadata,
    upsert: bool,
) -> anyhow::Result<()> {
    let mut q = SqlBuilder::insert_into("template_metadata")
        .value("template_id", &m.template_id)
        .value("kind", m.kind.as_str())
        .value("description", &m.description)
        .value("source", m.source.as_str())
        .value("checksum", m.catalog_checksum.as_deref())
        .value("created_at", timestamp_iso(m.created_at)?);
    if upsert {
        let mut conflict = ConflictUpdate::new(["template_id"]);
        for col in ["kind", "description", "source", "checksum", "created_at"] {
            conflict = conflict.set_excluded(col);
        }
        q = q.on_conflict_update(conflict);
    }
    tx.write(q.build()).await?;
    Ok(())
}
async fn write_preview(
    tx: &PostgresTransaction,
    p: Option<&StoredTemplatePreview>,
) -> anyhow::Result<()> {
    if let Some(p) = p {
        tx.write(
            SqlBuilder::insert_into("template_previews")
                .value("template_id", &p.template_id)
                .value("content_type", &p.content_type)
                .value("preview_data", p.bytes.as_slice())
                .value("width", p.width)
                .value("height", p.height)
                .build(),
        )
        .await?;
    }
    Ok(())
}
async fn insert_assets(tx: &PostgresTransaction, assets: &[TemplateAsset]) -> anyhow::Result<()> {
    for a in assets {
        tx.write(
            SqlBuilder::insert_into("template_assets")
                .value("asset_id", &a.asset_id)
                .value("template_id", &a.template_id)
                .value("content_type", &a.content_type)
                .value("asset_data", a.bytes.as_slice())
                .build(),
        )
        .await?;
    }
    Ok(())
}
fn map_asset(r: &crate::adapters::postgres::client::Row<'_>) -> RowResult<TemplateAsset> {
    Ok(TemplateAsset {
        asset_id: r.get("asset_id")?,
        template_id: r.get("template_id")?,
        content_type: r.get("content_type")?,
        bytes: r.get("asset_data")?,
    })
}
fn map_preview(r: &crate::adapters::postgres::client::Row<'_>) -> RowResult<StoredTemplatePreview> {
    Ok(StoredTemplatePreview {
        revision: r.get("revision")?,
        template_id: r.get("template_id")?,
        content_type: r.get("content_type")?,
        bytes: r.get("preview_data")?,
        width: r.get("width")?,
        height: r.get("height")?,
    })
}
async fn metadata_in(
    tx: &PostgresTransaction,
    id: &str,
) -> anyhow::Result<Option<StoredTemplateMetadata>> {
    let row = tx
        .read_one_with(
            SqlBuilder::select("template_metadata")
                .and_where(Condition::equal("template_id", id))
                .build(),
            |r| {
                Ok((
                    r.get::<_, String>("kind")?,
                    r.get::<_, String>("description")?,
                    r.get::<_, String>("source")?,
                    r.get::<_, String>("created_at")?,
                    r.get::<_, Option<String>>("checksum")?,
                ))
            },
        )
        .await?;
    let Some((kind, description, source, created, checksum)) = row else {
        return Ok(None);
    };
    let json = tx
        .read_one_with(
            SqlBuilder::select("templates")
                .columns(["template_json"])
                .and_where(Condition::equal("template_id", id))
                .build(),
            |r| r.get::<_, String>(0),
        )
        .await?
        .context("missing template")?;
    let title = serde_json::from_str::<serde_json::Value>(&json)?["presentation"]["title"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    map_metadata((
        id.into(),
        kind,
        title,
        description,
        source,
        created,
        None,
        checksum,
    ))
    .map(Some)
}

#[derive(Debug, thiserror::Error)]
#[error("template publication was cancelled")]
pub(super) struct PublicationCancelled;
fn check_publication(c: Option<&super::RequestCancellation>) -> anyhow::Result<()> {
    if c.is_some_and(super::RequestCancellation::is_cancelled) {
        return Err(PublicationCancelled.into());
    }
    Ok(())
}

async fn repository_boundary<T>(
    operation: impl std::future::Future<Output = anyhow::Result<T>>,
) -> RepositoryResult<T> {
    operation
        .await
        .map_err(|error| match error.downcast::<RepositoryError>() {
            Ok(error) => error,
            Err(error) => RepositoryError::from(error),
        })
}

#[async_trait::async_trait]
impl TemplateRepository for PostgresTemplateRepository {
    fn for_user(&self, user_id: i64) -> RepositoryResult<Arc<dyn TemplateRepository>> {
        Ok(Arc::new(self.user_scope(user_id)?))
    }
}

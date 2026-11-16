//! Canonical immutable identity persistence.
use super::identity::{ChunkIdentity, IdentityManifest};
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, PostgresTransaction},
        query::{Condition, SqlBuilder},
    },
    shared::error::RepositoryError,
};
pub(crate) async fn read(
    tx: &PostgresTransaction,
    version: &str,
) -> Result<Option<IdentityManifest>, PostgresError> {
    let head = tx
        .read_one_with(
            SqlBuilder::select("document_identity_manifests")
                .and_where(Condition::equal("version_id", version))
                .build(),
            |r| {
                Ok((
                    r.get("derivation_version")?,
                    r.get::<_, String>("document_id")?,
                    r.get::<_, i64>("chunk_count")?,
                    r.get::<_, String>("active_generation")?,
                ))
            },
        )
        .await?;
    let Some((derivation_version, document_id, count, active_generation)) = head else {
        return Ok(None);
    };
    let chunks = tx
        .read_with(
            SqlBuilder::select("document_chunk_identities")
                .and_where(Condition::equal("version_id", version))
                .order_by(
                    "index_generation",
                    crate::adapters::postgres::query::SortDirection::Ascending,
                )
                .order_by(
                    "chunk_index",
                    crate::adapters::postgres::query::SortDirection::Ascending,
                )
                .build(),
            |r| {
                Ok(ChunkIdentity {
                    chunk_id: r.get("chunk_id")?,
                    index_generation: r.get("index_generation")?,
                    chunk_index: r.get("chunk_index")?,
                    chunk_sha256: r.get("chunk_sha256")?,
                })
            },
        )
        .await?;
    if i64::try_from(chunks.len()).ok() != Some(count) {
        return Err(PostgresError::transaction_aborted(
            "incomplete identity manifest",
        ));
    }
    Ok(Some(IdentityManifest {
        derivation_version,
        document_id,
        active_generation,
        chunks,
    }))
}
pub(crate) async fn save(
    tx: &PostgresTransaction,
    version: &str,
    manifest: &IdentityManifest,
    migrated: bool,
) -> Result<(), RepositoryError> {
    manifest.validate()?;
    if let Some(existing) = read(tx, version).await? {
        let mut incoming = manifest.clone();
        incoming.chunks.sort_by(|a, b| {
            (&a.index_generation, a.chunk_index).cmp(&(&b.index_generation, b.chunk_index))
        });
        if existing != incoming {
            return Err(RepositoryError::Conflict(
                "identity manifest differs from immutable version".into(),
            ));
        }
        return Ok(());
    }
    tx.write(
        SqlBuilder::insert_into("document_identity_manifests")
            .value("version_id", version)
            .value("derivation_version", manifest.derivation_version)
            .value("document_id", &manifest.document_id)
            .value("active_generation", &manifest.active_generation)
            .value(
                "chunk_count",
                i64::try_from(manifest.chunks.len())
                    .map_err(|_| RepositoryError::invalid("too many chunk identities"))?,
            )
            .value("search_ready", migrated)
            .build(),
    )
    .await?;
    for c in &manifest.chunks {
        tx.write(
            SqlBuilder::insert_into("document_chunk_identities")
                .value("chunk_id", &c.chunk_id)
                .value("version_id", version)
                .value("index_generation", &c.index_generation)
                .value("chunk_index", c.chunk_index)
                .value("chunk_sha256", &c.chunk_sha256)
                .build(),
        )
        .await?;
    }
    Ok(())
}
pub async fn manifest(
    postgres: &PostgresClient,
    version: String,
) -> Result<Option<IdentityManifest>, RepositoryError> {
    Ok(postgres
        .read_transaction_async(move |tx| async move {
            let tx = &tx;
            read(tx, &version).await
        })
        .await?)
}
pub async fn ready(postgres: &PostgresClient) -> Result<bool, RepositoryError> {
    Ok(postgres
        .read_one_with_async(
            SqlBuilder::select("document_search_integrity")
                .columns(["version_id"])
                .limit(1)
                .build(),
            |_| Ok(()),
        )
        .await?
        .is_none())
}

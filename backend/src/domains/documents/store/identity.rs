//! Immutable chunk identity, retained independently of the recoverable graph projection.
use crate::{
    adapters::sqlite::{
        client::{SqliteClient, SqliteClientError, SqliteTransaction},
        query::{Condition, SqlBuilder},
    },
    shared::error::RepositoryError,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChunkIdentity {
    pub chunk_id: String,
    pub index_generation: String,
    pub chunk_index: i64,
    pub chunk_sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityManifest {
    pub derivation_version: i64,
    pub document_id: String,
    pub active_generation: String,
    pub chunks: Vec<ChunkIdentity>,
}
impl IdentityManifest {
    pub fn validate(&self) -> Result<(), RepositoryError> {
        if ![1, 2].contains(&self.derivation_version)
            || self.document_id.is_empty()
            || self.active_generation.is_empty()
        {
            return Err(RepositoryError::invalid("invalid identity manifest"));
        }
        let mut ids = std::collections::HashSet::new();
        let mut orders = std::collections::HashSet::new();
        for c in &self.chunks {
            if c.chunk_id.is_empty()
                || c.index_generation.is_empty()
                || c.chunk_sha256.len() != 64
                || c.chunk_index < 0
                || !ids.insert(&c.chunk_id)
                || !orders.insert((&c.index_generation, c.chunk_index))
            {
                return Err(RepositoryError::invalid(
                    "ambiguous chunk identity manifest",
                ));
            }
        }
        Ok(())
    }
}
pub(crate) fn read(
    tx: &SqliteTransaction<'_>,
    version: &str,
) -> Result<Option<IdentityManifest>, SqliteClientError> {
    let head = tx.read_one_with(
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
    )?;
    let Some((derivation_version, document_id, count, active_generation)) = head else {
        return Ok(None);
    };
    let chunks = tx.read_with(
        SqlBuilder::select("document_chunk_identities")
            .and_where(Condition::equal("version_id", version))
            .order_by(
                "index_generation",
                crate::adapters::sqlite::query::SortDirection::Ascending,
            )
            .order_by(
                "chunk_index",
                crate::adapters::sqlite::query::SortDirection::Ascending,
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
    )?;
    if i64::try_from(chunks.len()).ok() != Some(count) {
        return Err(SqliteClientError::transaction_aborted(
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
pub(crate) fn save(
    tx: &SqliteTransaction<'_>,
    version: &str,
    manifest: &IdentityManifest,
    migrated: bool,
) -> Result<(), RepositoryError> {
    manifest.validate()?;
    if let Some(existing) = read(tx, version)? {
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
            .value("graph_migrated", migrated)
            .build(),
    )?;
    for c in &manifest.chunks {
        tx.write(
            SqlBuilder::insert_into("document_chunk_identities")
                .value("chunk_id", &c.chunk_id)
                .value("version_id", version)
                .value("index_generation", &c.index_generation)
                .value("chunk_index", c.chunk_index)
                .value("chunk_sha256", &c.chunk_sha256)
                .build(),
        )?;
    }
    Ok(())
}
pub async fn manifest(
    sqlite: &SqliteClient,
    version: String,
) -> Result<Option<IdentityManifest>, RepositoryError> {
    Ok(sqlite
        .transaction_async(move |tx| read(tx, &version))
        .await?)
}
pub async fn ready(sqlite: &SqliteClient) -> Result<bool, RepositoryError> {
    let missing = sqlite
        .read_one_with_async(
            SqlBuilder::select("quarry_file_versions")
                .columns(["quarry_file_versions.version_id"])
                .left_join(
                    "document_identity_manifests",
                    "quarry_file_versions.version_id",
                    "document_identity_manifests.version_id",
                )
                .and_where(Condition::is_null("document_identity_manifests.version_id"))
                .limit(1)
                .build(),
            |_| Ok(()),
        )
        .await?;
    let pending = sqlite
        .read_one_with_async(
            SqlBuilder::select("document_identity_manifests")
                .and_where(Condition::equal("graph_migrated", false))
                .limit(1)
                .build(),
            |_| Ok(()),
        )
        .await?;
    let legacy = sqlite
        .read_one_with_async(
            SqlBuilder::select("document_identity_manifests")
                .and_where(Condition::equal("derivation_version", 1))
                .limit(1)
                .build(),
            |_| Ok(()),
        )
        .await?;
    let indexes = sqlite
        .read_one_with_async(
            SqlBuilder::select("app_metadata")
                .and_where(Condition::equal("key", "document_deal_scope_indexes"))
                .columns(["value"])
                .build(),
            |row| row.get::<_, String>(0),
        )
        .await?;
    Ok(missing.is_none()
        && pending.is_none()
        && indexes.as_deref() != Some("migrating")
        && (legacy.is_none() || indexes.as_deref() == Some("ready")))
}

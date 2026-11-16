use crate::shared::search_text::lexical_parts;
// Complete, dimension/profile-pinned document batches and SQL-scoped retrieval.
use super::chunk::*;
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, PostgresTransaction, Row},
        query::{
            Condition as C, SelectExpression as E, SortDirection as D, SqlBuilder as Q, SqlQuery,
            SqlValue,
        },
        vector::CosineVector,
    },
    shared::{error::RepositoryError, ids::sha256_hex},
};

#[derive(Clone, Debug)]
pub(crate) struct EmbeddingProfile {
    pub id: String,
    pub model: String,
    pub dimensions: usize,
}
impl EmbeddingProfile {
    pub fn new(model: String, dimensions: usize) -> Self {
        Self {
            id: sha256_hex(format!("openai\0{model}\0{dimensions}\0cosine\01").as_bytes()),
            model,
            dimensions,
        }
    }
    pub async fn ensure(&self, tx: &PostgresTransaction) -> Result<(), RepositoryError> {
        tx.write(
            Q::insert_into("document_embedding_profiles")
                .value("profile_id", &self.id)
                .value("provider", "openai")
                .value("model", &self.model)
                .value(
                    "dimensions",
                    i64::try_from(self.dimensions).map_err(PostgresError::decode)?,
                )
                .value("metric", "cosine")
                .value("preparation_version", 1)
                .on_conflict_do_nothing(["profile_id"])
                .build(),
        )
        .await?;
        Ok(())
    }
}

pub(crate) struct PreparedChunks {
    pub profile: EmbeddingProfile,
    pub rows: Vec<(FileChunkResult, CosineVector, String)>,
}
impl PreparedChunks {
    pub fn new(
        profile: EmbeddingProfile,
        rows: Vec<(FileChunkResult, Vec<f32>)>,
    ) -> Result<Self, RepositoryError> {
        if rows.len() > 10_000 {
            return Err(RepositoryError::invalid("document exceeds 10000 chunks"));
        }
        let mut bytes = 0usize;
        let mut result = Vec::with_capacity(rows.len());
        for (index, (chunk, embedding)) in rows.into_iter().enumerate() {
            bytes = bytes
                .checked_add(chunk.text.len())
                .and_then(|n| {
                    embedding
                        .len()
                        .checked_mul(4)
                        .and_then(|v| n.checked_add(v))
                })
                .ok_or_else(|| RepositoryError::invalid("document batch overflow"))?;
            if chunk.chunk_index != i64::try_from(index + 1).map_err(PostgresError::decode)?
                || chunk.chunk_sha256 != sha256_hex(chunk.text.as_bytes())
                || chunk.text.is_empty()
                || chunk.text.len() > 131_072
                || bytes > 128 * 1024 * 1024
            {
                return Err(RepositoryError::invalid(
                    "invalid or oversized document batch",
                ));
            }
            let vector = CosineVector::new(embedding, profile.dimensions)
                .map_err(|_| RepositoryError::invalid("invalid document embedding"))?;
            let (_, technical) = lexical_parts(&chunk.text);
            bytes = bytes
                .checked_add(technical.len())
                .and_then(|n| n.checked_add(chunk.section_path.len()))
                .ok_or_else(|| RepositoryError::invalid("document batch overflow"))?;
            if bytes > 128 * 1024 * 1024 || chunk.section_path.len() > 8192 {
                return Err(RepositoryError::invalid(
                    "normalized document batch exceeds search bounds",
                ));
            }
            if technical.len() > 131072 {
                return Err(RepositoryError::invalid(
                    "normalized document text exceeds search bounds",
                ));
            }
            result.push((chunk, vector, technical));
        }
        Ok(Self {
            profile,
            rows: result,
        })
    }
    pub async fn insert(self, tx: &PostgresTransaction) -> Result<(), RepositoryError> {
        self.profile.ensure(tx).await?;
        // A replay must already have the complete, immutable set. Never skip a
        // conflicting chunk INSERT or retain earlier successful batches on error.
        let mut chunks = self.rows.into_iter();
        loop {
            let rows: Vec<_> = chunks
                .by_ref()
                .take(128)
                .map(|(chunk, embedding, technical)| {
                    vec![
                        chunk.chunk_id.into(),
                        self.profile.id.clone().into(),
                        chunk.text.into(),
                        technical.into(),
                        embedding.into(),
                        chunk.token_count.into(),
                        chunk.page_start.into(),
                        chunk.page_end.into(),
                        chunk.char_start.into(),
                        chunk.char_end.into(),
                        chunk.section_path.into(),
                        chunk.created_at.into(),
                    ]
                })
                .collect();
            let expected = rows.len();
            if expected == 0 {
                break;
            }
            let affected = tx.write(insert_chunks(rows)).await?.rows_affected;
            if affected != expected {
                return Err(RepositoryError::invalid(
                    "document chunk insert count differs",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct DocumentSearchRepository {
    pub postgres: PostgresClient,
    pub profile: EmbeddingProfile,
}
impl DocumentSearchRepository {
    pub fn new(postgres: PostgresClient, profile: EmbeddingProfile) -> Self {
        Self { postgres, profile }
    }
    pub async fn branches(
        &self,
        actor: i64,
        deal: &str,
        query: Option<&str>,
        vector: Option<Vec<f32>>,
        limit: usize,
    ) -> Result<(Vec<KeywordFileChunkHit>, Vec<VectorFileChunkHit>), RepositoryError> {
        let vector = vector
            .map(|v| {
                CosineVector::new(v, self.profile.dimensions).map_err(|_| {
                    RepositoryError::invalid("query does not match document embedding profile")
                })
            })
            .transpose()?;
        let deal = deal.to_owned();
        let profile = self.profile.id.clone();
        let query = query.map(str::to_owned);
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                crate::domains::memberships::require_deal(
                    &tx,
                    &deal,
                    actor,
                    crate::domains::memberships::model::Capability::Read,
                )
                .await?;
                let keyword = if let Some(query) = query {
                    tx.read_with(
                        ranked(actor, &deal, &profile, Ranking::Keyword(&query), limit),
                        |r| {
                            Ok(KeywordFileChunkHit {
                                chunk: decode(r)?,
                                score: r.get("score")?,
                            })
                        },
                    )
                    .await?
                } else {
                    Vec::new()
                };
                let vector = if let Some(vector) = vector {
                    tx.read_with(
                        ranked(actor, &deal, &profile, Ranking::Cosine(vector), limit),
                        |r| {
                            Ok(VectorFileChunkHit {
                                chunk: decode(r)?,
                                distance: r.get::<_, f64>("score")? / 2.0,
                            })
                        },
                    )
                    .await?
                } else {
                    Vec::new()
                };
                Ok((keyword, vector))
            })
            .await
    }
}
fn decode(r: &Row<'_>) -> Result<FileChunkResult, PostgresError> {
    Ok(FileChunkResult {
        chunk_id: r.get("chunk_id")?,
        deal_id: r.get("deal_id")?,
        file_id: r.get("file_id")?,
        version_id: r.get("version_id")?,
        index_generation: r.get("index_generation")?,
        chunk_index: r.get("chunk_index")?,
        text: r.get("text")?,
        chunk_sha256: r.get("chunk_sha256")?,
        token_count: r.get("token_count")?,
        page_start: r.get("page_start")?,
        page_end: r.get("page_end")?,
        char_start: r.get("char_start")?,
        char_end: r.get("char_end")?,
        section_path: r.get("section_path")?,
        created_at: r.get("created_at")?,
    })
}

pub(crate) enum Ranking<'a> {
    Keyword(&'a str),
    Cosine(CosineVector),
}

pub(crate) fn ranked(
    actor: i64,
    deal: &str,
    profile: &str,
    ranking: Ranking<'_>,
    limit: usize,
) -> Result<SqlQuery, PostgresError> {
    if limit == 0 || limit > 100 {
        return Err(PostgresError::transaction_aborted(
            "invalid document candidate limit",
        ));
    }
    let mut query = Q::select("document_chunks")
        .columns([
            "document_chunk_identities.chunk_id",
            "quarry_files.deal_id",
            "quarry_file_versions.file_id",
            "document_chunk_identities.version_id",
            "document_chunk_identities.index_generation",
            "document_chunk_identities.chunk_index",
            "document_chunks.text",
            "document_chunk_identities.chunk_sha256",
            "document_chunks.token_count",
            "document_chunks.page_start",
            "document_chunks.page_end",
            "document_chunks.char_start",
            "document_chunks.char_end",
            "document_chunks.section_path",
            "document_chunks.created_at",
        ])
        .inner_join(
            "document_chunk_identities",
            "document_chunks.chunk_id",
            "document_chunk_identities.chunk_id",
        )
        .inner_join(
            "document_identity_manifests",
            "document_chunk_identities.version_id",
            "document_identity_manifests.version_id",
        )
        .inner_join(
            "quarry_file_versions",
            "document_chunk_identities.version_id",
            "quarry_file_versions.version_id",
        )
        .inner_join(
            "quarry_files",
            "quarry_file_versions.file_id",
            "quarry_files.file_id",
        )
        .inner_join(
            "deal_memberships",
            "quarry_files.deal_id",
            "deal_memberships.deal_id",
        )
        .left_join(
            "deal_deletions",
            "quarry_files.deal_id",
            "deal_deletions.deal_id",
        )
        .and_where(C::equal("deal_memberships.user_id", actor))
        .and_where(C::equal("document_chunks.profile_id", profile))
        .and_where(C::is_null("quarry_files.deleted_at"))
        .and_where(C::equal("quarry_file_versions.is_current", 1))
        .and_where(C::equal("document_identity_manifests.search_ready", 1))
        .and_where(C::columns_equal(
            "document_chunk_identities.index_generation",
            "document_identity_manifests.active_generation",
        ))
        .and_where(C::is_null("deal_deletions.deal_id"));
    if !deal.is_empty() {
        query = query.and_where(C::equal("quarry_files.deal_id", deal));
    }
    let direction = match ranking {
        Ranking::Keyword(text) => {
            let phrase = text.starts_with('"') && text.ends_with('"') && text.len() > 1;
            let (body, technical) = lexical_parts(text);
            let prefix =
                text.ends_with('*') && body.split_whitespace().count() == 1 && technical.is_empty();
            if prefix {
                query = query
                    .expression(
                        E::TextPrefixRank {
                            column: "document_chunks.search_vector".into(),
                            token: body.clone(),
                        },
                        "score",
                    )
                    .and_where(C::text_prefix("document_chunks.search_vector", body));
            } else {
                query = query.expression(
                    E::TextRank {
                        column: "document_chunks.search_vector".into(),
                        text: format!("{body} {technical}"),
                    },
                    "score",
                );
                if !body.is_empty() {
                    query = query.and_where(C::english_text_matches(
                        "document_chunks.search_vector",
                        body,
                        phrase,
                    ));
                }
            }
            if !technical.is_empty() {
                query =
                    query.and_where(C::text_matches("document_chunks.search_vector", technical));
            }
            if text.trim().is_empty() || lexical_parts(text) == (String::new(), String::new()) {
                query = query.and_where(C::text_matches("document_chunks.search_vector", ""));
            }
            D::Descending
        }
        Ranking::Cosine(vector) => {
            query = query.expression(
                E::CosineDistance {
                    column: "document_chunks.embedding".into(),
                    vector,
                },
                "score",
            );
            D::Ascending
        }
    };
    query
        .order_by("score", direction)
        .order_by("quarry_files.deal_id", D::Ascending)
        .order_by("quarry_file_versions.file_id", D::Ascending)
        .order_by("document_chunk_identities.version_id", D::Ascending)
        .order_by("document_chunk_identities.chunk_id", D::Ascending)
        .order_by("document_chunk_identities.index_generation", D::Ascending)
        .order_by("document_chunk_identities.chunk_sha256", D::Ascending)
        .order_by("document_chunk_identities.chunk_index", D::Ascending)
        .limit(limit)
        .build()
}
pub(crate) fn insert_chunks(rows: Vec<Vec<SqlValue>>) -> Result<SqlQuery, PostgresError> {
    Q::insert_batch(
        "document_chunks",
        [
            "chunk_id",
            "profile_id",
            "text",
            "technical_text",
            "embedding",
            "token_count",
            "page_start",
            "page_end",
            "char_start",
            "char_end",
            "section_path",
            "created_at",
        ],
        rows,
    )
    .build()
}

#[cfg(test)]
#[path = "../../../../tests/unit/domains/documents/store/search_repository_tests.rs"]
mod tests;

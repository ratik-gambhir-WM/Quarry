//! Immutable prepared artifacts and bounded SQL publication on the native save transaction.
use super::{
    model::Note,
    repository::{NotebookRepository, Result},
};
use crate::{
    adapters::postgres::{
        client::PostgresTransaction,
        query::{Condition as C, SqlBuilder as Q},
        vector::CosineVector,
    },
    shared::{error::RepositoryError as Error, ids::sha256_hex},
};

pub(super) fn source_hash(note: &Note, pipeline: &str) -> Result<String> {
    let bytes = serde_json::to_vec(&(pipeline, &note.summary.title, &note.blocks))
        .map_err(|_| Error::storage("invalid notebook artifact input"))?;
    Ok(sha256_hex(&bytes))
}
pub(super) struct PreparedNote {
    pub pipeline: String,
    pub dimensions: usize,
    pub source_hash: String,
    pub markdown: String,
    pub rows: Vec<(String, CosineVector, String)>,
}
impl PreparedNote {
    pub fn new(
        note: &Note,
        pipeline: String,
        dimensions: usize,
        markdown: String,
        chunks: Vec<String>,
        vectors: Vec<CosineVector>,
    ) -> Result<Self> {
        if chunks.len() != vectors.len()
            || chunks.len() > 1024
            || markdown.len() > 2 * 1024 * 1024
            || !(1..=16000).contains(&dimensions)
        {
            return Err(Error::invalid("invalid notebook artifact batch"));
        }
        let mut bytes = markdown.len();
        let mut rows = Vec::with_capacity(chunks.len());
        for (text, vector) in chunks.into_iter().zip(vectors) {
            if vector.as_slice().len() != dimensions {
                return Err(Error::invalid("incompatible notebook embedding"));
            }
            let (_, technical) =
                crate::shared::search_text::lexical_parts(&super::markdown::unescape(&text));
            bytes = bytes
                .checked_add(text.len() + technical.len() + dimensions * 4)
                .ok_or_else(|| Error::invalid("notebook artifact overflow"))?;
            if text.is_empty() || text.len() > 2561 || bytes > 128 * 1024 * 1024 {
                return Err(Error::invalid("notebook artifact exceeds bounds"));
            }
            rows.push((text, vector, technical));
        }
        Ok(Self {
            source_hash: source_hash(note, &pipeline)?,
            pipeline,
            dimensions,
            markdown,
            rows,
        })
    }
    pub async fn insert(self, tx: &PostgresTransaction, note: &Note, set: &str) -> Result<()> {
        let profile = sha256_hex(self.pipeline.as_bytes());
        tx.write(
            Q::insert_into("notebook_embedding_profiles")
                .value("profile_id", &profile)
                .value("pipeline", self.pipeline)
                .value(
                    "dimensions",
                    i64::try_from(self.dimensions)
                        .map_err(|_| Error::invalid("notebook dimensions"))?,
                )
                .on_conflict_do_nothing(["profile_id"])
                .build(),
        )
        .await?;
        tx.write(
            Q::insert_into("notebook_search_sets")
                .value("set_id", set)
                .value("note_id", &note.summary.note_id)
                .value("profile_id", &profile)
                .value(
                    "dimensions",
                    i64::try_from(self.dimensions)
                        .map_err(|_| Error::invalid("notebook dimensions"))?,
                )
                .value("source_hash", self.source_hash)
                .value("markdown", self.markdown)
                .value(
                    "chunk_count",
                    i64::try_from(self.rows.len())
                        .map_err(|_| Error::invalid("notebook chunk count"))?,
                )
                .build(),
        )
        .await?;
        let mut rows = self.rows.into_iter().enumerate();
        loop {
            let batch: Vec<Vec<crate::adapters::postgres::query::SqlValue>> = rows
                .by_ref()
                .take(64)
                .map(|(ordinal, (text, vector, technical))| {
                    Ok(vec![
                        set.into(),
                        i64::try_from(ordinal)
                            .map_err(|_| Error::invalid("notebook ordinal"))?
                            .into(),
                        profile.clone().into(),
                        sha256_hex(text.as_bytes()).into(),
                        text.into(),
                        note.summary.title.clone().into(),
                        technical.into(),
                        vector.into(),
                    ])
                })
                .collect::<Result<_>>()?;
            let count = batch.len();
            if count == 0 {
                break;
            }
            if tx
                .write(
                    Q::insert_batch(
                        "notebook_chunks",
                        [
                            "set_id",
                            "ordinal",
                            "profile_id",
                            "chunk_hash",
                            "text",
                            "title",
                            "technical_text",
                            "embedding",
                        ],
                        batch,
                    )
                    .build(),
                )
                .await?
                .rows_affected
                != count
            {
                return Err(Error::storage("incomplete notebook chunk insertion"));
            }
        }
        Ok(())
    }
}
impl NotebookRepository {
    /// A hint only: final publication rechecks both the revision and this immutable identity.
    pub(super) async fn reusable(&self, note: &Note, pipeline: &str) -> Result<Option<String>> {
        let hash = source_hash(note, pipeline)?;
        Ok(self
            .postgres
            .read_one_with_async(
                Q::select("notebook_search_sets")
                    .inner_join(
                        "notebook_notes",
                        "notebook_notes.active_set_id",
                        "notebook_search_sets.set_id",
                    )
                    .column("notebook_search_sets.set_id")
                    .and_where(C::equal("notebook_notes.note_id", &note.summary.note_id))
                    .and_where(C::equal("source_hash", hash))
                    .and_where(C::equal("profile_id", sha256_hex(pipeline.as_bytes())))
                    .build(),
                |r| r.get(0),
            )
            .await?)
    }
}

use super::model::{SearchHit, SearchInput, SearchMode, SearchResults};
use crate::adapters::postgres::{
    client::{PostgresError, Row},
    query::{SelectExpression as E, SortDirection as D},
};
use std::collections::BTreeMap;

fn ranked(
    owner: i64,
    notebook: &str,
    profile: &str,
) -> crate::adapters::postgres::query::SelectBuilder {
    Q::select("notebook_chunks")
        .columns([
            "notebook_notes.notebook_id",
            "notebook_notes.note_id",
            "notebook_notes.revision",
            "notebook_notes.title",
            "notebook_chunks.ordinal",
            "notebook_chunks.text",
            "notebook_chunks.set_id",
        ])
        .inner_join(
            "notebook_search_sets",
            "notebook_chunks.set_id",
            "notebook_search_sets.set_id",
        )
        .inner_join(
            "notebook_notes",
            "notebook_search_sets.note_id",
            "notebook_notes.note_id",
        )
        .inner_join(
            "notebooks",
            "notebook_notes.notebook_id",
            "notebooks.notebook_id",
        )
        .and_where(C::equal("notebooks.user_id", owner))
        .and_where(C::equal("notebooks.notebook_id", notebook))
        .and_where(C::columns_equal(
            "notebook_notes.active_set_id",
            "notebook_chunks.set_id",
        ))
        .and_where(C::equal("notebook_chunks.profile_id", profile))
}
fn hit(r: &Row<'_>) -> std::result::Result<SearchHit, PostgresError> {
    let text: String = r.get("text")?;
    Ok(SearchHit {
        notebook_id: r.get("notebook_id")?,
        note_id: r.get("note_id")?,
        revision: r.get("revision")?,
        title: r.get("title")?,
        chunk_ordinal: r.get("ordinal")?,
        excerpt: text.chars().take(600).collect(),
        match_sources: Vec::new(),
        set_id: r.get("set_id")?,
    })
}
impl NotebookRepository {
    pub(super) async fn search(
        &self,
        owner: i64,
        notebook: String,
        pipeline: String,
        input: SearchInput,
        vector: Option<CosineVector>,
    ) -> Result<SearchResults> {
        let profile = sha256_hex(pipeline.as_bytes());
        let check_book = notebook.clone();
        let mut hits = self
            .postgres
            .read_transaction_result_async(move |tx| async move {
                super::repository::owned(&tx, owner, &notebook).await?;
                let mut branches = Vec::new();
                if input.mode != SearchMode::Semantic {
                    let (body, technical) =
                        crate::shared::search_text::lexical_parts(&input.query_text);
                    let phrase =
                        input.query_text.starts_with('"') && input.query_text.ends_with('"');
                    let prefix = input.query_text.ends_with('*')
                        && body.split_whitespace().count() == 1
                        && technical.is_empty();
                    let mut q = ranked(owner, &notebook, &profile);
                    if prefix {
                        q = q
                            .expression(
                                E::TextPrefixRank {
                                    column: "notebook_chunks.search_vector".into(),
                                    token: body.clone(),
                                },
                                "score",
                            )
                            .and_where(C::text_prefix("notebook_chunks.search_vector", body));
                    } else {
                        q = q.expression(
                            E::TextRank {
                                column: "notebook_chunks.search_vector".into(),
                                text: format!("{body} {technical}"),
                            },
                            "score",
                        );
                        if !body.is_empty() {
                            q = q.and_where(C::english_text_matches(
                                "notebook_chunks.search_vector",
                                body,
                                phrase,
                            ));
                        }
                        if !technical.is_empty() {
                            q = q.and_where(C::text_matches(
                                "notebook_chunks.search_vector",
                                technical,
                            ));
                        }
                        if crate::shared::search_text::lexical_parts(&input.query_text)
                            == (String::new(), String::new())
                        {
                            q = q.and_where(C::text_matches("notebook_chunks.search_vector", ""));
                        }
                    }
                    branches.push((
                        "keyword",
                        tx.read_with(
                            q.order_by("score", D::Descending)
                                .order_by("notebook_notes.note_id", D::Ascending)
                                .order_by("notebook_chunks.ordinal", D::Ascending)
                                .limit(100)
                                .build(),
                            hit,
                        )
                        .await?,
                    ));
                }
                if let Some(vector) = vector {
                    let q = ranked(owner, &notebook, &profile)
                        .expression(
                            E::CosineDistance {
                                column: "notebook_chunks.embedding".into(),
                                vector,
                            },
                            "score",
                        )
                        .order_by("score", D::Ascending)
                        .order_by("notebook_notes.note_id", D::Ascending)
                        .order_by("notebook_chunks.ordinal", D::Ascending)
                        .limit(100)
                        .build();
                    branches.push(("semantic", tx.read_with(q, hit).await?));
                }
                let mut fused = BTreeMap::new();
                for (source, rows) in branches {
                    for (rank, mut hit) in rows.into_iter().enumerate() {
                        let score = 1.0
                            / (60.0
                                + f64::from(
                                    u32::try_from(rank + 1)
                                        .map_err(|_| Error::invalid("notebook search rank"))?,
                                ));
                        let entry = fused
                            .entry((hit.note_id.clone(), hit.chunk_ordinal))
                            .or_insert_with(|| {
                                hit.match_sources.push(source);
                                (0.0, hit)
                            });
                        entry.0 += score;
                        if !entry.1.match_sources.contains(&source) {
                            entry.1.match_sources.push(source);
                        }
                    }
                }
                let mut rows: Vec<_> = fused.into_values().collect();
                rows.sort_by(|a, b| {
                    b.0.total_cmp(&a.0)
                        .then_with(|| a.1.note_id.cmp(&b.1.note_id))
                        .then_with(|| a.1.chunk_ordinal.cmp(&b.1.chunk_ordinal))
                });
                Ok::<_, Error>(
                    rows.into_iter()
                        .take(input.limit)
                        .map(|(_, hit)| hit)
                        .collect::<Vec<_>>(),
                )
            })
            .await?;
        // Fresh bounded authorization/identity check after ranking. An old read snapshot must
        // never deliver candidates whose revision or active set has since changed.
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                super::repository::owned(&tx, owner, &check_book).await?;
                let mut current = Vec::with_capacity(hits.len());
                for hit in hits.drain(..) {
                    if tx
                        .read_one(
                            Q::select("notebook_notes")
                                .column("note_id")
                                .and_where(C::equal("notebook_id", &check_book))
                                .and_where(C::equal("note_id", &hit.note_id))
                                .and_where(C::equal("revision", hit.revision))
                                .and_where(C::equal("active_set_id", &hit.set_id))
                                .build(),
                        )
                        .await?
                        .is_some()
                    {
                        current.push(hit);
                    }
                }
                Ok(SearchResults { hits: current })
            })
            .await
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/domains/notebooks/search_repository_tests.rs"]
mod tests;

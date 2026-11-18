//! Owner/app-scoped lexical and vector retrieval queries.
use super::processing_repository::{map_state, STATUS};
use super::*;
use crate::adapters::postgres::{query::SelectExpression, vector::CosineVector};

#[async_trait::async_trait]
impl TemplateRetrievalRepository for PostgresTemplateRepository {
    /// Scoped classified metadata without vectors, newest first.
    async fn list_retrieval_classifications(
        &self,
    ) -> RepositoryResult<Vec<RetrievalClassificationRecord>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let rows = tx
                        .read_result_with(
                            SqlBuilder::select(STATUS)
                                .columns(["template_classification_status.*"])
                                .inner_join(
                                    "templates",
                                    "templates.template_id",
                                    "template_classification_status.template_id",
                                )
                                .and_where(self.owner_condition())
                                .and_where(Condition::is_not_null("metadata_json"))
                                .and_where(Condition::equal("schema_version", 2))
                                .build(),
                            map_state,
                        )
                        .await?;
                    let mut result = Vec::with_capacity(rows.len());
                    for state in rows {
                        let id = state.template_id.clone();
                        let meta = metadata_in(tx, &id).await?.context("missing metadata")?;
                        let preview_available = tx
                            .read_one(
                                SqlBuilder::select("template_previews")
                                    .columns(["template_id"])
                                    .and_where(Condition::equal("template_id", &id))
                                    .build(),
                            )
                            .await?
                            .is_some();
                        result.push(RetrievalClassificationRecord {
                            template_id: id,
                            classification: state,
                            created_at: timestamp_iso(meta.created_at)?,
                            kind: meta.kind,
                            preview_available,
                            title: meta.title,
                        });
                    }
                    result.sort_by(|a, b| {
                        b.created_at
                            .cmp(&a.created_at)
                            .then(a.template_id.cmp(&b.template_id))
                    });
                    Ok(result)
                })
                .await
        })
        .await
    }
    async fn search_slide_classifications(
        &self,
        fts_query: &str,
        limit: usize,
    ) -> RepositoryResult<Vec<String>> {
        repository_boundary(async {
            Ok(crate::adapters::postgres::query::lexical::search(
                &self.client,
                self.owner_user_id,
                fts_query,
                limit,
            )
            .await?)
        })
        .await
    }
    /// Exact SQL cosine ranking for one role over the authorized ready catalog of a profile.
    /// Only identities and similarities leave PostgreSQL. Keep the complete ranking:
    /// metadata filtering, tie-breaking and fusion happen before the service's output limit.
    async fn rank_slide_embeddings(
        &self,
        profile_id: &str,
        role: EmbeddingRole,
        query: &CosineVector,
    ) -> RepositoryResult<Vec<RankedTemplate>> {
        repository_boundary(async {
            let rows = self
                .client
                .read_with_async(
                    SqlBuilder::select("template_embeddings")
                        .columns(["template_embeddings.template_id"])
                        .expression(
                            SelectExpression::CosineDistance {
                                column: "template_embeddings.embedding".into(),
                                vector: query.clone(),
                            },
                            "distance",
                        )
                        .inner_join(
                            STATUS,
                            "template_classification_status.template_id",
                            "template_embeddings.template_id",
                        )
                        .inner_join(
                            "templates",
                            "templates.template_id",
                            "template_embeddings.template_id",
                        )
                        .and_where(self.owner_condition())
                        .and_where(Condition::equal(
                            "template_classification_status.embedding_status",
                            "ready",
                        ))
                        .and_where(Condition::equal(
                            "template_classification_status.embedding_profile_id",
                            profile_id,
                        ))
                        .and_where(Condition::equal(
                            "template_classification_status.schema_version",
                            2,
                        ))
                        .and_where(Condition::is_not_null(
                            "template_classification_status.metadata_json",
                        ))
                        .and_where(Condition::equal("template_embeddings.role", role.as_str()))
                        .and_where(Condition::equal(
                            "template_embeddings.profile_id",
                            profile_id,
                        ))
                        .order_by("distance", SortDirection::Ascending)
                        .order_by("template_embeddings.template_id", SortDirection::Ascending)
                        .build(),
                    |r| {
                        Ok(RankedTemplate {
                            template_id: r.get(0)?,
                            similarity: 1.0 - r.get::<_, f64>(1)?,
                        })
                    },
                )
                .await?;
            Ok(rows)
        })
        .await
    }
    /// Load one authorized, ready source pair for `similar`. Foreign or unready sources are
    /// indistinguishable from missing ones.
    async fn find_slide_embedding_pair(
        &self,
        template_id: &str,
    ) -> RepositoryResult<Option<StoredEmbeddingPair>> {
        repository_boundary(async {
            self.client
                .read_transaction_result_async(move |tx| async move {
                    let tx = &tx;
                    let Some(profile_id) = tx
                        .read_one_with(
                            SqlBuilder::select(STATUS)
                                .columns(["template_classification_status.embedding_profile_id"])
                                .inner_join(
                                    "templates",
                                    "templates.template_id",
                                    "template_classification_status.template_id",
                                )
                                .and_where(self.owner_condition())
                                .and_where(Condition::equal(
                                    "template_classification_status.template_id",
                                    template_id,
                                ))
                                .and_where(Condition::equal(
                                    "template_classification_status.embedding_status",
                                    "ready",
                                ))
                                .and_where(Condition::is_not_null(
                                    "template_classification_status.metadata_json",
                                ))
                                .build(),
                            |r| r.get::<_, String>(0),
                        )
                        .await?
                    else {
                        return Ok(None);
                    };
                    let mut subject = None;
                    let mut capability = None;
                    for (role, vector) in tx
                        .read_with(
                            SqlBuilder::select("template_embeddings")
                                .columns(["role", "embedding"])
                                .and_where(Condition::equal("template_id", template_id))
                                .and_where(Condition::equal("profile_id", &profile_id))
                                .build(),
                            |r| Ok((r.get::<_, String>(0)?, r.get::<_, CosineVector>(1)?)),
                        )
                        .await?
                    {
                        match role.as_str() {
                            "subject" => subject = Some(vector),
                            "capability" => capability = Some(vector),
                            _ => anyhow::bail!("unknown template embedding role"),
                        }
                    }
                    Ok(Some(StoredEmbeddingPair {
                        profile_id,
                        subject: subject.context("missing subject embedding")?,
                        capability: capability.context("missing capability embedding")?,
                    }))
                })
                .await
        })
        .await
    }
    /// Fresh bounded recheck of selected identities before delivery.
    async fn retain_retrievable(
        &self,
        template_ids: &[String],
    ) -> RepositoryResult<std::collections::HashSet<String>> {
        repository_boundary(async {
            if template_ids.is_empty() {
                return Ok(Default::default());
            }
            ensure!(template_ids.len() <= 100, "too many retrieval identities");
            Ok(self
                .client
                .read_with_async(
                    SqlBuilder::select(STATUS)
                        .columns(["template_classification_status.template_id"])
                        .inner_join(
                            "templates",
                            "templates.template_id",
                            "template_classification_status.template_id",
                        )
                        .and_where(self.owner_condition())
                        .and_where(Condition::is_not_null(
                            "template_classification_status.metadata_json",
                        ))
                        .and_where(Condition::in_values(
                            "template_classification_status.template_id",
                            template_ids.iter().map(String::as_str),
                        ))
                        .build(),
                    |r| r.get::<_, String>(0),
                )
                .await?
                .into_iter()
                .collect())
        })
        .await
    }
}

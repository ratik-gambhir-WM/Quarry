//! Deal-scoped remote admission records. Every document mutation checks this record in its
//! own Helix write transaction, including requests prepared before deletion was accepted.
use helix_db::{dsl::prelude::*, BatchEntry};

pub(crate) const CONTROL: &str = "QuarryDealLifecycle";

fn control() -> Traversal<OnNodes> {
    g().n_with_label(CONTROL)
        .where_(Predicate::eq_param("deal_id", "deal_id"))
}

pub(super) fn guard(mut body: WriteBatch) -> WriteBatch {
    let mut batch = write_batch()
        .var_as("deal_control", control())
        .var_as_if(
            "created_control",
            BatchCondition::VarEmpty("deal_control".into()),
            g().add_n(CONTROL, vec![("deal_id", PropertyInput::param("deal_id"))]),
        )
        .var_as(
            "active_deal",
            control().where_(Predicate::is_null("deleted")),
        )
        .var_as(
            "deletion_fence",
            control()
                .where_(Predicate::eq("deleted", true))
                .project(vec![PropertyProjection::new("deal_id")]),
        );
    gate_entries(&mut body.entries);
    batch.entries.extend(body.entries);
    body.returns.push("deletion_fence".into());
    batch.returns = body.returns;
    batch
}

fn gate_entries(entries: &mut Vec<BatchEntry>) {
    let mut gated = Vec::with_capacity(entries.len() * 2);
    for entry in std::mem::take(entries) {
        match entry {
            BatchEntry::Query(mut query) => {
                if let Some(condition) = query.condition.take() {
                    let name = format!("admit_{}", query.name.as_deref().unwrap_or("write"));
                    gated.extend(
                        write_batch()
                            .var_as_if(&name, condition, g().n(NodeRef::var("active_deal")))
                            .entries,
                    );
                    query.condition = Some(BatchCondition::VarNotEmpty(name));
                } else {
                    query.condition = Some(BatchCondition::VarNotEmpty("active_deal".into()));
                }
                gated.push(BatchEntry::Query(query));
            }
            BatchEntry::ForEach { param, mut body } => {
                gate_entries(&mut body);
                gated.push(BatchEntry::ForEach { param, body });
            }
        }
    }
    *entries = gated;
}

#[query]
pub(crate) fn fence(deal_id: String) -> WriteBatch {
    let _ = &deal_id;
    write_batch()
        .var_as("existing", control())
        .var_as_if(
            "created",
            BatchCondition::VarEmpty("existing".into()),
            g().add_n(CONTROL, vec![("deal_id", PropertyInput::param("deal_id"))]),
        )
        .var_as(
            "fence",
            control()
                .set_property("deleted", true)
                .project(vec![PropertyProjection::new("deal_id")]),
        )
        .returning(["fence"])
}

impl super::repository::DocumentGraphWriter {
    pub(crate) async fn fence_deal(
        &self,
        deal: &str,
    ) -> Result<(), crate::shared::error::RepositoryError> {
        use crate::shared::error::RepositoryError as E;
        #[derive(serde::Deserialize)]
        struct Ack {
            fence: Vec<serde_json::Value>,
        }
        let result: Ack = self
            .helix
            .execute_write_query(
                "helix.deletion.fence",
                fence(deal.to_owned()).map_err(|e| E::storage(e.to_string()))?,
            )
            .await
            .map_err(E::storage)?;
        if result.fence.len() != 1
            || result.fence[0]
                .get("deal_id")
                .and_then(serde_json::Value::as_str)
                != Some(deal)
        {
            return Err(E::invalid("deletion fence acknowledgement missing"));
        }
        Ok(())
    }
    /// One bounded sweep. The durable file target is retained until every label is empty.
    pub(crate) async fn purge_deal_file(
        &self,
        deal: &str,
        file: &str,
    ) -> Result<bool, crate::shared::error::RepositoryError> {
        use crate::shared::error::RepositoryError as E;
        #[derive(serde::Deserialize)]
        struct Nodes {
            nodes: Vec<serde_json::Value>,
        }
        let mut empty = true;
        for label in [
            super::writer::FILE_CHUNK_LABEL,
            super::writer::FILE_VERSION_LABEL,
            super::writer::QUARRY_FILE_LABEL,
        ] {
            let scope = || {
                g().n_with_label(label)
                    .where_(Predicate::eq("file_id", file.to_owned()))
            };
            let nodes: Nodes = self
                .helix
                .execute_read_query(
                    "helix.deletion.inspect",
                    QueryRequest::read(
                        read_batch()
                            .var_as(
                                "nodes",
                                scope().limit(128).project(vec![
                                    PropertyProjection::new("deal_id"),
                                    PropertyProjection::new("file_id"),
                                ]),
                            )
                            .returning(["nodes"]),
                    ),
                )
                .await
                .map_err(E::storage)?;
            if nodes.nodes.iter().any(|n| {
                n.get("deal_id").and_then(serde_json::Value::as_str) != Some(deal)
                    || n.get("file_id").and_then(serde_json::Value::as_str) != Some(file)
            }) {
                return Err(E::invalid("deletion graph ownership mismatch"));
            }
            if !nodes.nodes.is_empty() {
                empty = false;
                let _: serde_json::Value = self
                    .helix
                    .execute_write_query(
                        "helix.deletion.purge",
                        QueryRequest::write(
                            write_batch()
                                .var_as(
                                    "removed",
                                    scope()
                                        .where_(Predicate::eq("deal_id", deal.to_owned()))
                                        .limit(128)
                                        .drop(),
                                )
                                .returning(["removed"]),
                        ),
                    )
                    .await
                    .map_err(E::storage)?;
            }
        }
        Ok(empty)
    }
    pub(crate) async fn verify_deal_absent(
        &self,
        deal: &str,
    ) -> Result<(), crate::shared::error::RepositoryError> {
        use crate::shared::error::RepositoryError as E;
        #[derive(serde::Deserialize)]
        struct Nodes {
            nodes: Vec<serde_json::Value>,
        }
        for label in [
            super::writer::FILE_CHUNK_LABEL,
            super::writer::FILE_VERSION_LABEL,
            super::writer::QUARRY_FILE_LABEL,
        ] {
            let nodes: Nodes = self
                .helix
                .execute_read_query(
                    "helix.deletion.verify",
                    QueryRequest::read(
                        read_batch()
                            .var_as(
                                "nodes",
                                g().n_with_label(label)
                                    .where_(Predicate::eq("deal_id", deal.to_owned()))
                                    // A bound of one has scalar/null wire semantics in Helix v3.
                                    .limit(2)
                                    .project(vec![PropertyProjection::new("file_id")]),
                            )
                            .returning(["nodes"]),
                    ),
                )
                .await
                .map_err(E::storage)?;
            if !nodes.nodes.is_empty() {
                return Err(E::invalid("unmatched deal graph records remain"));
            }
        }
        Ok(())
    }
}

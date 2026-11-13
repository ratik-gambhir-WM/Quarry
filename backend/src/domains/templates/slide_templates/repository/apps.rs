//! App memberships are internal persistence scopes, not authentication.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredApp {
    pub app_id: String,
    pub display_name: String,
    pub metadata: BTreeMap<String, String>,
    pub created_at: String,
    pub last_seen_at: String,
}
#[derive(Clone, Debug, Default)]
pub struct AppRegistration {
    pub display_name: Option<String>,
    pub metadata: Option<BTreeMap<String, String>>,
}
impl PostgresTemplateRepository {
    pub async fn ensure_app(
        &self,
        id: &str,
        registration: Option<&AppRegistration>,
    ) -> anyhow::Result<StoredApp> {
        ensure!(
            !id.is_empty()
                && id.len() <= 128
                && id.as_bytes()[0].is_ascii_alphanumeric()
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')),
            "invalid app ID"
        );
        let registration = registration.cloned().unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.client
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                let new = tx
                    .read_one(
                        SqlBuilder::select("apps")
                            .and_where(Condition::equal("app_id", id))
                            .build(),
                    )
                    .await?
                    .is_none();
                let mut conflict = ConflictUpdate::new(["app_id"]).set("last_seen_at", &now);
                if let Some(name) = &registration.display_name {
                    conflict = conflict.set("display_name", name);
                }
                if let Some(metadata) = &registration.metadata {
                    conflict = conflict.set("metadata_json", serde_json::to_string(metadata)?);
                }
                tx.write(
                    SqlBuilder::insert_into("apps")
                        .value("app_id", id)
                        .value(
                            "display_name",
                            registration.display_name.as_deref().unwrap_or(id),
                        )
                        .value(
                            "metadata_json",
                            serde_json::to_string(&registration.metadata.unwrap_or_default())?,
                        )
                        .value("created_at", &now)
                        .value("last_seen_at", &now)
                        .on_conflict_update(conflict)
                        .build(),
                )
                .await?;
                if new {
                    for template in tx
                        .read_with(
                            SqlBuilder::select("template_metadata")
                                .columns(["template_id"])
                                .and_where(Condition::equal("source", "builtin"))
                                .build(),
                            |r| r.get::<_, String>(0),
                        )
                        .await?
                    {
                        membership(tx, id, &template).await?;
                    }
                }
                Ok::<(), anyhow::Error>(())
            })
            .await?;
        self.find_app(id).await?.context("app registration failed")
    }
    pub async fn find_app(&self, id: &str) -> anyhow::Result<Option<StoredApp>> {
        let row = self
            .client
            .read_one_with_async(
                SqlBuilder::select("apps")
                    .and_where(Condition::equal("app_id", id))
                    .build(),
                |r| {
                    Ok((
                        r.get::<_, String>("display_name")?,
                        r.get::<_, String>("metadata_json")?,
                        r.get::<_, String>("created_at")?,
                        r.get::<_, String>("last_seen_at")?,
                    ))
                },
            )
            .await?;
        row.map(|(display_name, json, created_at, last_seen_at)| {
            Ok(StoredApp {
                app_id: id.into(),
                display_name,
                metadata: serde_json::from_str(&json)?,
                created_at,
                last_seen_at,
            })
        })
        .transpose()
    }
}

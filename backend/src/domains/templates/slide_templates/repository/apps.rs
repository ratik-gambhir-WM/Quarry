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
impl SqliteTemplateRepository {
    pub fn ensure_app(
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
        self.client.transaction_result(|tx| -> anyhow::Result<()> {
            let new = tx
                .read_one(
                    SqlBuilder::select("apps")
                        .and_where(Condition::equal("app_id", id))
                        .build(),
                )?
                .is_none();
            let mut conflict = ConflictUpdate::new(["app_id"]).set("last_seen_at", &now);
            if registration.display_name.is_some() || registration.metadata.is_some() {
                conflict = conflict
                    .set(
                        "display_name",
                        registration.display_name.as_deref().unwrap_or(id),
                    )
                    .set(
                        "metadata_json",
                        serde_json::to_string(&registration.metadata.clone().unwrap_or_default())?,
                    );
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
            )?;
            if new {
                for template in tx.read_with(
                    SqlBuilder::select("template_metadata")
                        .columns(["template_id"])
                        .and_where(Condition::equal("source", "builtin"))
                        .build(),
                    |r| r.get::<_, String>(0),
                )? {
                    membership(tx, id, &template)?;
                }
            }
            Ok(())
        })?;
        self.find_app(id)?.context("app registration failed")
    }
    pub fn find_app(&self, id: &str) -> anyhow::Result<Option<StoredApp>> {
        let row = self.client.read_one_with(
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
        )?;
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

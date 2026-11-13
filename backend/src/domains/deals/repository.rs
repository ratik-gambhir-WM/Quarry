pub use super::model::{Deal, DealFields};
use super::model::{DealParty, DiligenceScope, PartyRole};
use crate::adapters::postgres::client::Row;
use crate::adapters::postgres::client::RowResult;
use crate::domains::memberships::{
    model::Capability, record_activity as event, require_deal as require,
};
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError, PostgresTransaction},
        query::{Condition, RowLock, SqlBuilder},
    },
    domains::companies::company_from_row,
    shared::error::RepositoryError,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
const DEAL_COLUMNS: [&str; 15] = [
    "deal_id",
    "content_revision",
    "user_id",
    "deal_name",
    "deal_code",
    "engagement_type",
    "transaction_type",
    "status",
    "start_date",
    "expected_close_date",
    "actual_close_date",
    "engagement_completed_date",
    "archived_at",
    "created_at",
    "updated_at",
];
const METADATA_COLUMNS: [&str; 8] = [
    "deal_id",
    "metadata_revision",
    "key_questions_json",
    "local_path",
    "sharepoint_link",
    "sow_link",
    "fact_sheet_link",
    "rl_link",
];
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealMetadata {
    pub revision: i64,
    pub deal_id: String,
    pub key_questions_json: String,
    pub local_path: Option<String>,
    pub sharepoint_link: Option<String>,
    pub sow_link: Option<String>,
    pub fact_sheet_link: Option<String>,
    pub rl_link: Option<String>,
}
#[derive(Debug, Clone)]
pub struct UpsertDealMetadataRecord {
    pub expected_revision: i64,
    pub deal_id: String,
    pub key_questions_json: String,
    pub local_path: Option<String>,
    pub sharepoint_link: Option<String>,
    pub sow_link: Option<String>,
    pub fact_sheet_link: Option<String>,
    pub rl_link: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealWithMetadata {
    #[serde(flatten)]
    pub deal: Deal,
    pub metadata: Option<DealMetadata>,
}
pub struct CreateDealRecord {
    pub deal_id: String,
    pub user_id: i64,
    pub fields: DealFields,
    pub local_path: Option<String>,
    pub sharepoint_link: Option<String>,
}
#[derive(Clone)]
pub struct DealRepository {
    postgres: PostgresClient,
}
impl DealRepository {
    pub fn new(postgres: PostgresClient) -> Self {
        Self { postgres }
    }
    pub async fn access(
        &self,
        id: String,
        actor: i64,
    ) -> Result<crate::domains::memberships::model::DealAccess, RepositoryError> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                require(tx, &id, actor, Capability::Read).await
            })
            .await
    }
    pub async fn create(
        &self,
        record: CreateDealRecord,
    ) -> Result<DealWithMetadata, RepositoryError> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                validate_companies(tx, record.user_id, &record.fields).await??;
                let now = chrono::Utc::now().to_rfc3339();
                let insert = SqlBuilder::insert_into("deals")
                    .value("deal_id", &record.deal_id)
                    .value("user_id", record.user_id)
                    .value("deal_name", record.fields.deal_name.trim())
                    .value("deal_code", record.fields.deal_code.as_deref())
                    .value("engagement_type", record.fields.engagement_type.code())
                    .value("transaction_type", record.fields.transaction_type.code())
                    .value("status", record.fields.status.code())
                    .value("start_date", record.fields.start_date.as_deref())
                    .value(
                        "expected_close_date",
                        record.fields.expected_close_date.as_deref(),
                    )
                    .value(
                        "actual_close_date",
                        record.fields.actual_close_date.as_deref(),
                    )
                    .value(
                        "engagement_completed_date",
                        record.fields.engagement_completed_date.as_deref(),
                    )
                    .value("created_at", &now)
                    .value("updated_at", &now)
                    .build();
                tx.write(insert).await.map_err(|error| {
                    if error.is_unique_violation() {
                        RepositoryError::Conflict("dealId is reserved".into())
                    } else {
                        error.into()
                    }
                })?;
                tx.write(
                    SqlBuilder::insert_into("deal_metadata")
                        .value("deal_id", &record.deal_id)
                        .value("local_path", record.local_path.as_deref())
                        .value("sharepoint_link", record.sharepoint_link.as_deref())
                        .build(),
                )
                .await?;
                replace_children(tx, &record.deal_id, &record.fields).await?;
                crate::domains::memberships::initialize_deal(
                    tx,
                    &record.deal_id,
                    record.user_id,
                    &now,
                )
                .await?;
                load_one(tx, &record.deal_id)
                    .await?
                    .ok_or_else(|| RepositoryError::storage("created deal missing"))
            })
            .await
    }
    pub async fn update(
        &self,
        id: String,
        actor: i64,
        fields: DealFields,
        expected_revision: i64,
    ) -> Result<DealWithMetadata, RepositoryError> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                super::coordination::lock_authority(tx, &id, RowLock::Share).await?;
                require(tx, &id, actor, Capability::Edit).await?;
                validate_companies_for_update(tx, &id, actor, &fields).await??;
                let next = crate::shared::revision::next(expected_revision)?;
                let changed = tx
                    .write(
                        SqlBuilder::update("deals")
                            .set("deal_name", fields.deal_name.trim())
                            .set("deal_code", fields.deal_code.as_deref())
                            .set("engagement_type", fields.engagement_type.code())
                            .set("transaction_type", fields.transaction_type.code())
                            .set("status", fields.status.code())
                            .set("start_date", fields.start_date.as_deref())
                            .set("expected_close_date", fields.expected_close_date.as_deref())
                            .set("actual_close_date", fields.actual_close_date.as_deref())
                            .set(
                                "engagement_completed_date",
                                fields.engagement_completed_date.as_deref(),
                            )
                            .set("updated_at", chrono::Utc::now().to_rfc3339())
                            .set("content_revision", next)
                            .and_where(Condition::equal("deal_id", &id))
                            .and_where(Condition::equal("content_revision", expected_revision))
                            .build(),
                    )
                    .await?;
                if changed.rows_affected != 1 {
                    return Err(crate::shared::revision::conflict());
                }
                replace_children(tx, &id, &fields).await?;
                event(tx, &id, Some(actor), "deal_updated", Some(&id)).await?;
                load_one(tx, &id)
                    .await?
                    .ok_or_else(|| RepositoryError::storage("updated deal missing"))
            })
            .await
    }
    pub async fn list_owned(&self, owner: i64) -> Result<Vec<DealWithMetadata>, RepositoryError> {
        self.postgres
            .read_transaction_async(move |tx| async move {
                let tx = &tx;
                load(tx, None, Some(owner)).await
            })
            .await
            .map_err(storage)
    }
    pub async fn with_metadata(
        &self,
        id: String,
    ) -> Result<Option<DealWithMetadata>, RepositoryError> {
        self.postgres
            .read_transaction_async(move |tx| async move {
                let tx = &tx;
                load_one(tx, &id).await
            })
            .await
            .map_err(storage)
    }
    pub async fn by_id(&self, id: String) -> Result<Option<Deal>, RepositoryError> {
        Ok(self
            .with_metadata(id)
            .await?
            .map(|aggregate| aggregate.deal))
    }
    pub(super) async fn local_source_root(
        &self,
        id: String,
    ) -> Result<Option<String>, RepositoryError> {
        self.postgres
            .read_transaction_result_async(move |tx| async move {
                let tx = &tx;
                if super::deletion::barrier(tx, &id).await?
                    || tx
                        .read_one_with(
                            SqlBuilder::select("deals")
                                .columns(["deal_id"])
                                .and_where(Condition::equal("deal_id", &id))
                                .build(),
                            |_| Ok(()),
                        )
                        .await?
                        .is_none()
                {
                    return Err(RepositoryError::NotFound("deal not found".into()));
                }
                Ok(tx
                    .read_one_with(
                        SqlBuilder::select("deal_metadata")
                            .columns(["local_path"])
                            .and_where(Condition::equal("deal_id", id))
                            .build(),
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .await?
                    .flatten())
            })
            .await
    }
    pub async fn metadata(&self, id: String) -> Result<Option<DealMetadata>, RepositoryError> {
        self.postgres
            .read_one_with_async(
                SqlBuilder::select("deal_metadata")
                    .columns(METADATA_COLUMNS)
                    .and_where(Condition::equal("deal_id", id))
                    .build(),
                metadata_from_row,
            )
            .await
            .map_err(storage)
    }
    pub async fn archive(&self, id: String, actor: i64) -> Result<Option<Deal>, RepositoryError> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                super::coordination::lock_authority(tx, &id, RowLock::Update).await?;
                let access = require(tx, &id, actor, Capability::Transfer).await?;
                if access.archived {
                    return Ok(load_one(tx, &id).await?.map(|a| a.deal));
                }
                let now = chrono::Utc::now().to_rfc3339();
                let current = load_one(tx, &id)
                    .await?
                    .ok_or_else(|| RepositoryError::NotFound("deal not found".into()))?;
                let next = crate::shared::revision::next(current.deal.revision)?;
                let changed = tx
                    .write(
                        SqlBuilder::update("deals")
                            .set("content_revision", next)
                            .set("archived_at", &now)
                            .set("updated_at", &now)
                            .and_where(Condition::equal("deal_id", &id))
                            .and_where(Condition::is_null("archived_at"))
                            .build(),
                    )
                    .await?;
                if changed.rows_affected != 1 {
                    return Err(crate::shared::revision::conflict());
                }
                event(tx, &id, Some(actor), "deal_archived", Some(&id)).await?;
                Ok(load_one(tx, &id).await?.map(|a| a.deal))
            })
            .await
    }
    pub async fn upsert_metadata(
        &self,
        record: UpsertDealMetadataRecord,
        actor: i64,
    ) -> Result<DealMetadata, RepositoryError> {
        serde_json::from_str::<Vec<String>>(&record.key_questions_json)
            .map_err(|_| RepositoryError::invalid("key questions must be an array of strings"))?;
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                super::coordination::lock_authority(tx, &record.deal_id, RowLock::Share).await?;
                require(tx, &record.deal_id, actor, Capability::Edit).await?;
                let local_path = tx
                    .read_one_with(
                        SqlBuilder::select("deal_metadata")
                            .columns(["local_path"])
                            .and_where(Condition::equal("deal_id", &record.deal_id))
                            .build(),
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .await?
                    .flatten();
                if local_path != record.local_path {
                    require(tx, &record.deal_id, actor, Capability::LocalSource).await?;
                }
                let changed = tx
                    .write(
                        SqlBuilder::update("deal_metadata")
                            .set("key_questions_json", &record.key_questions_json)
                            .set("local_path", record.local_path.as_deref())
                            .set("sharepoint_link", record.sharepoint_link.as_deref())
                            .set("sow_link", record.sow_link.as_deref())
                            .set("fact_sheet_link", record.fact_sheet_link.as_deref())
                            .set("rl_link", record.rl_link.as_deref())
                            .set(
                                "metadata_revision",
                                crate::shared::revision::next(record.expected_revision)?,
                            )
                            .and_where(Condition::equal("deal_id", &record.deal_id))
                            .and_where(Condition::equal(
                                "metadata_revision",
                                record.expected_revision,
                            ))
                            .build(),
                    )
                    .await?;
                if changed.rows_affected != 1 {
                    return Err(crate::shared::revision::conflict());
                }
                event(
                    tx,
                    &record.deal_id,
                    Some(actor),
                    "metadata_updated",
                    Some(&record.deal_id),
                )
                .await?;
                tx.read_with(
                    SqlBuilder::select("deal_metadata")
                        .columns(METADATA_COLUMNS)
                        .and_where(Condition::equal("deal_id", &record.deal_id))
                        .build(),
                    metadata_from_row,
                )
                .await?
                .pop()
                .ok_or_else(|| RepositoryError::storage("metadata missing"))
            })
            .await
    }
}
async fn validate_companies(
    tx: &PostgresTransaction,
    owner: i64,
    fields: &DealFields,
) -> Result<Result<(), RepositoryError>, PostgresError> {
    if fields.parties.is_empty() {
        return Ok(Ok(()));
    }
    let companies = tx
        .read_with(
            SqlBuilder::select("companies")
                .and_where(Condition::in_values(
                    "company_id",
                    fields.parties.iter().map(|p| p.company_id.as_str()),
                ))
                .build(),
            company_from_row,
        )
        .await?;
    let ids: HashSet<_> = companies.iter().map(|c| c.company_id.as_str()).collect();
    if fields
        .parties
        .iter()
        .any(|p| !ids.contains(p.company_id.as_str()))
    {
        return Ok(Err(RepositoryError::NotFound(
            "referenced company not found".into(),
        )));
    }
    if companies.iter().any(|c| c.user_id != owner) {
        return Ok(Err(RepositoryError::Conflict(
            "company and deal must have the same owner".into(),
        )));
    }
    Ok(Ok(()))
}
async fn validate_companies_for_update(
    tx: &PostgresTransaction,
    deal: &str,
    actor: i64,
    fields: &DealFields,
) -> Result<Result<(), RepositoryError>, PostgresError> {
    let existing = tx
        .read_with(
            SqlBuilder::select("deal_parties")
                .columns(["company_id"])
                .and_where(Condition::equal("deal_id", deal))
                .build(),
            |r| r.get::<_, String>(0),
        )
        .await?
        .into_iter()
        .collect::<HashSet<_>>();
    let mut new_fields = fields.clone();
    new_fields
        .parties
        .retain(|p| !existing.contains(&p.company_id));
    validate_companies(tx, actor, &new_fields).await
}
async fn replace_children(
    tx: &PostgresTransaction,
    id: &str,
    fields: &DealFields,
) -> Result<(), PostgresError> {
    let existing = tx
        .read_with(
            SqlBuilder::select("deal_parties")
                .and_where(Condition::equal("deal_id", id))
                .build(),
            |r| {
                Ok((
                    (
                        r.get::<_, String>("company_id")?,
                        r.get::<_, String>("role")?,
                    ),
                    r.get::<_, String>("deal_party_id")?,
                ))
            },
        )
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>();
    for table in ["deal_parties", "deal_scopes"] {
        tx.write(
            SqlBuilder::delete_from(table)
                .and_where(Condition::equal("deal_id", id))
                .build(),
        )
        .await?;
    }
    for party in &fields.parties {
        let party_id = existing
            .get(&(party.company_id.clone(), party.role.code().to_string()))
            .cloned()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        tx.write(
            SqlBuilder::insert_into("deal_parties")
                .value("deal_party_id", party_id)
                .value("deal_id", id)
                .value("company_id", &party.company_id)
                .value("role", party.role.code())
                .value("notes", party.notes.as_deref())
                .build(),
        )
        .await?;
    }
    for scope in &fields.scopes {
        tx.write(
            SqlBuilder::insert_into("deal_scopes")
                .value("deal_id", id)
                .value("scope", scope.code())
                .build(),
        )
        .await?;
    }
    Ok(())
}
async fn load_one(
    tx: &PostgresTransaction,
    id: &str,
) -> Result<Option<DealWithMetadata>, PostgresError> {
    Ok(load(tx, Some(id), None).await?.pop())
}
// Read each collection in bounded batches under one snapshot, without per-deal queries.
async fn load(
    tx: &PostgresTransaction,
    id: Option<&str>,
    owner: Option<i64>,
) -> Result<Vec<DealWithMetadata>, PostgresError> {
    let mut builder = SqlBuilder::select("deals")
        .columns(DEAL_COLUMNS.map(|column| format!("deals.{column}")))
        .columns(
            METADATA_COLUMNS[1..]
                .iter()
                .map(|column| format!("deal_metadata.{column}")),
        )
        .left_join("deal_metadata", "deals.deal_id", "deal_metadata.deal_id")
        .left_join("deal_deletions", "deals.deal_id", "deal_deletions.deal_id")
        .and_where(Condition::is_null("deal_deletions.deal_id"));
    builder = if let Some(id) = id {
        builder.and_where(Condition::equal("deals.deal_id", id))
    } else {
        builder.and_where(Condition::is_null("deals.archived_at"))
    };
    if let Some(owner) = owner {
        builder = builder
            .inner_join(
                "deal_memberships",
                "deals.deal_id",
                "deal_memberships.deal_id",
            )
            .inner_join(
                "deal_access_revisions",
                "deals.deal_id",
                "deal_access_revisions.deal_id",
            )
            .columns(["deal_memberships.role", "deal_access_revisions.revision"])
            .and_where(Condition::equal("deal_memberships.user_id", owner));
    }
    let mut deals = tx
        .read_with(builder.build(), |row| {
            // key_questions_json is NOT NULL on a metadata row, but nullable after the LEFT JOIN.
            let metadata = if row
                .get::<_, Option<String>>("key_questions_json")?
                .is_some()
            {
                Some(metadata_from_row(row)?)
            } else {
                None
            };
            let mut aggregate = DealWithMetadata {
                deal: deal_from_row(row)?,
                metadata,
            };
            if let Some(actor) = owner {
                let access = crate::domains::memberships::model::DealAccess::for_actor(
                    enum_from_row(row, "role")?,
                    aggregate.deal.user_id,
                    aggregate.deal.archived_at.is_some(),
                    row.get("revision")?,
                    actor,
                );
                if !access.capabilities.local_source {
                    if let Some(metadata) = &mut aggregate.metadata {
                        metadata.local_path = None;
                    }
                }
                aggregate.deal.access = Some(access);
            }
            Ok(aggregate)
        })
        .await?;
    if deals.is_empty() {
        return Ok(Vec::new());
    }
    let ids = deals
        .iter()
        .map(|aggregate| aggregate.deal.deal_id.as_str())
        .collect::<Vec<_>>();
    let mut party_map: HashMap<String, Vec<DealParty>> = HashMap::new();
    let mut scope_map: HashMap<String, Vec<DiligenceScope>> = HashMap::new();
    // Both child reads use this transaction's connection and snapshot, off the Tokio worker.
    for batch in ids.chunks(500) {
        let parties = tx
            .read_with(
                SqlBuilder::select("deal_parties")
                    .columns([
                        "deal_parties.deal_id",
                        "deal_parties.deal_party_id",
                        "deal_parties.role",
                        "deal_parties.notes",
                    ])
                    .columns(
                        [
                            "company_id",
                            "revision",
                            "user_id",
                            "name",
                            "website",
                            "industry",
                            "description",
                            "created_at",
                            "updated_at",
                        ]
                        .map(|column| format!("companies.{column}")),
                    )
                    .inner_join(
                        "companies",
                        "deal_parties.company_id",
                        "companies.company_id",
                    )
                    .and_where(Condition::in_values(
                        "deal_parties.deal_id",
                        batch.iter().copied(),
                    ))
                    .build(),
                |row| {
                    let company = company_from_row(row)?;
                    Ok((
                        row.get::<_, String>("deal_id")?,
                        DealParty {
                            deal_party_id: row.get("deal_party_id")?,
                            company_id: company.company_id.clone(),
                            role: enum_from_row::<PartyRole>(row, "role")?,
                            notes: row.get("notes")?,
                            company,
                        },
                    ))
                },
            )
            .await?;
        for (id, party) in parties {
            party_map.entry(id).or_default().push(party);
        }
        let scopes = tx
            .read_with(
                SqlBuilder::select("deal_scopes")
                    .columns(["deal_id", "scope"])
                    .and_where(Condition::in_values("deal_id", batch.iter().copied()))
                    .build(),
                |row| {
                    Ok((
                        row.get::<_, String>("deal_id")?,
                        enum_from_row::<DiligenceScope>(row, "scope")?,
                    ))
                },
            )
            .await?;
        for (id, scope) in scopes {
            scope_map.entry(id).or_default().push(scope);
        }
    }
    for aggregate in &mut deals {
        let deal = &mut aggregate.deal;
        deal.parties = party_map.remove(&deal.deal_id).unwrap_or_default();
        deal.parties.sort_by(|a, b| {
            (a.role, &a.company.name, &a.company_id).cmp(&(b.role, &b.company.name, &b.company_id))
        });
        deal.scopes = scope_map.remove(&deal.deal_id).unwrap_or_default();
        deal.scopes.sort();
    }
    deals.sort_by(|a, b| {
        let (a, b) = (&a.deal, &b.deal);
        let a_date = a
            .actual_close_date
            .as_ref()
            .or(a.expected_close_date.as_ref());
        let b_date = b
            .actual_close_date
            .as_ref()
            .or(b.expected_close_date.as_ref());
        (a_date.is_none(), a_date, &a.deal_id).cmp(&(b_date.is_none(), b_date, &b.deal_id))
    });
    Ok(deals)
}

fn storage(e: PostgresError) -> RepositoryError {
    RepositoryError::storage(e.to_string())
}
fn enum_from_row<T: serde::de::DeserializeOwned>(row: &Row<'_>, column: &str) -> RowResult<T> {
    let value: String = row.get(column)?;
    serde_json::from_value(serde_json::Value::String(value))
        .map_err(crate::adapters::postgres::client::PostgresError::decode)
}
fn deal_from_row(row: &Row<'_>) -> RowResult<Deal> {
    Ok(Deal {
        access: None,
        revision: row.get("content_revision")?,
        deal_id: row.get("deal_id")?,
        user_id: row.get("user_id")?,
        deal_name: row.get("deal_name")?,
        deal_code: row.get("deal_code")?,
        engagement_type: enum_from_row(row, "engagement_type")?,
        transaction_type: enum_from_row(row, "transaction_type")?,
        status: enum_from_row(row, "status")?,
        start_date: row.get("start_date")?,
        expected_close_date: row.get("expected_close_date")?,
        actual_close_date: row.get("actual_close_date")?,
        engagement_completed_date: row.get("engagement_completed_date")?,
        archived_at: row.get("archived_at")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        parties: Vec::new(),
        scopes: Vec::new(),
    })
}
fn metadata_from_row(row: &Row<'_>) -> RowResult<DealMetadata> {
    Ok(DealMetadata {
        revision: row.get("metadata_revision")?,
        deal_id: row.get("deal_id")?,
        key_questions_json: row.get("key_questions_json")?,
        local_path: row.get("local_path")?,
        sharepoint_link: row.get("sharepoint_link")?,
        sow_link: row.get("sow_link")?,
        fact_sheet_link: row.get("fact_sheet_link")?,
        rl_link: row.get("rl_link")?,
    })
}

#[cfg(test)]
#[path = "../../../tests/integration/domains/deals/repository_tests.rs"]
mod tests;

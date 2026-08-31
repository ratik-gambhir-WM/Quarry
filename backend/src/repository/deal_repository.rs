use rusqlite::Row;
use serde::Serialize;

use crate::{
    core::{
        clients::sqlite::SqliteClient,
        sqlbuilder::{Condition, ConflictUpdate, SortDirection, SqlBuilder, SqlQuery},
    },
    repository::RepositoryError,
};

const DEAL_COLUMNS: [&str; 10] = [
    "deal_id",
    "user_id",
    "deal_name",
    "status",
    "start_date",
    "close_date",
    "transaction_type",
    "target_company",
    "primary_buyer",
    "deal_sponsor",
];

const DEAL_METADATA_COLUMNS: [&str; 5] = [
    "deal_id",
    "user_id",
    "key_questions_json",
    "local_path",
    "sharepoint_link",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deal {
    pub deal_id: String,
    pub user_id: i64,
    pub deal_name: String,
    pub status: String,
    pub start_date: String,
    pub close_date: String,
    pub transaction_type: String,
    pub target_company: String,
    pub primary_buyer: String,
    pub deal_sponsor: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealMetadata {
    pub deal_id: String,
    pub user_id: i64,
    pub key_questions_json: String,
    pub local_path: Option<String>,
    pub sharepoint_link: Option<String>,
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
    pub deal_name: String,
    pub status: String,
    pub start_date: String,
    pub close_date: String,
    pub transaction_type: String,
    pub target_company: String,
    pub primary_buyer: String,
    pub deal_sponsor: String,
}

pub struct UpsertDealMetadataRecord {
    pub deal_id: String,
    pub user_id: i64,
    pub key_questions_json: String,
    pub local_path: Option<String>,
    pub sharepoint_link: Option<String>,
}

#[derive(Clone)]
pub struct DealRepository {
    sqlite: SqliteClient,
}

impl DealRepository {
    pub fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }

    pub async fn create(&self, record: CreateDealRecord) -> Result<Deal, RepositoryError> {
        let sqlite = self.sqlite.clone();
        run_blocking(move || create_deal(&sqlite, &record)).await
    }

    pub async fn by_id(&self, deal_id: String) -> Result<Option<Deal>, RepositoryError> {
        let sqlite = self.sqlite.clone();
        run_blocking(move || deal_by_id(&sqlite, &deal_id)).await
    }

    pub async fn list(&self) -> Result<Vec<DealWithMetadata>, RepositoryError> {
        let sqlite = self.sqlite.clone();
        run_blocking(move || list_deals(&sqlite)).await
    }

    pub async fn with_metadata(
        &self,
        deal_id: String,
    ) -> Result<Option<DealWithMetadata>, RepositoryError> {
        let sqlite = self.sqlite.clone();
        run_blocking(move || get_deal_with_metadata(&sqlite, &deal_id)).await
    }

    pub async fn metadata(&self, deal_id: String) -> Result<Option<DealMetadata>, RepositoryError> {
        let sqlite = self.sqlite.clone();
        run_blocking(move || get_deal_metadata_by_deal_id(&sqlite, &deal_id)).await
    }

    pub async fn upsert_metadata(
        &self,
        record: UpsertDealMetadataRecord,
    ) -> Result<DealMetadata, RepositoryError> {
        let sqlite = self.sqlite.clone();
        run_blocking(move || upsert_deal_metadata(&sqlite, &record)).await
    }

    pub async fn archive(&self, deal_id: String) -> Result<Option<Deal>, RepositoryError> {
        let sqlite = self.sqlite.clone();
        run_blocking(move || archive_deal(&sqlite, &deal_id)).await
    }
}

async fn run_blocking<T>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, RepositoryError>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| RepositoryError::BlockingWorker(error.to_string()))?
        .map_err(RepositoryError::storage)
}

fn create_deal(sqlite: &SqliteClient, record: &CreateDealRecord) -> Result<Deal, String> {
    let query = build_query(
        SqlBuilder::insert_into("deals")
            .value("deal_id", &record.deal_id)
            .value("user_id", record.user_id)
            .value("deal_name", &record.deal_name)
            .value("status", &record.status)
            .value("start_date", &record.start_date)
            .value("close_date", &record.close_date)
            .value("transaction_type", &record.transaction_type)
            .value("target_company", &record.target_company)
            .value("primary_buyer", &record.primary_buyer)
            .value("deal_sponsor", &record.deal_sponsor)
            .build(),
        "deal insert",
    )?;
    sqlite
        .write(&query)
        .map_err(|error| format!("failed to insert deal: {error}"))?;
    deal_by_id(sqlite, &record.deal_id)?
        .ok_or_else(|| format!("inserted deal `{}` could not be loaded", record.deal_id))
}

fn list_deals(sqlite: &SqliteClient) -> Result<Vec<DealWithMetadata>, String> {
    let query = build_query(
        SqlBuilder::select("deals")
            .columns(DEAL_COLUMNS)
            .order_by("close_date", SortDirection::Ascending)
            .order_by("deal_id", SortDirection::Ascending)
            .build(),
        "deal list select",
    )?;
    let deals = sqlite
        .read_with(&query, deal_from_row)
        .map_err(|error| format!("failed to list deals: {error}"))?
        .into_iter()
        .filter(|deal| !deal.status.eq_ignore_ascii_case("archived"));

    deals
        .map(|deal| {
            let metadata = get_deal_metadata_by_deal_id(sqlite, &deal.deal_id)?;
            Ok(DealWithMetadata { deal, metadata })
        })
        .collect()
}

fn get_deal_with_metadata(
    sqlite: &SqliteClient,
    deal_id: &str,
) -> Result<Option<DealWithMetadata>, String> {
    let Some(deal) = deal_by_id(sqlite, deal_id)? else {
        return Ok(None);
    };
    let metadata = get_deal_metadata_by_deal_id(sqlite, deal_id)?;
    Ok(Some(DealWithMetadata { deal, metadata }))
}

fn get_deal_metadata_by_deal_id(
    sqlite: &SqliteClient,
    deal_id: &str,
) -> Result<Option<DealMetadata>, String> {
    let query = build_query(
        SqlBuilder::select("deal_metadata")
            .columns(DEAL_METADATA_COLUMNS)
            .and_where(Condition::equal("deal_id", deal_id))
            .build(),
        "deal metadata select",
    )?;
    sqlite
        .read_one_with(&query, deal_metadata_from_row)
        .map_err(|error| format!("failed to read deal metadata: {error}"))
}

fn upsert_deal_metadata(
    sqlite: &SqliteClient,
    record: &UpsertDealMetadataRecord,
) -> Result<DealMetadata, String> {
    let query = build_query(
        SqlBuilder::insert_into("deal_metadata")
            .value("deal_id", &record.deal_id)
            .value("user_id", record.user_id)
            .value("key_questions_json", &record.key_questions_json)
            .value("local_path", record.local_path.as_deref())
            .value("sharepoint_link", record.sharepoint_link.as_deref())
            .on_conflict_update(
                ConflictUpdate::new(["deal_id"])
                    .set_excluded("user_id")
                    .set_excluded("key_questions_json")
                    .set_excluded("local_path")
                    .set_excluded("sharepoint_link"),
            )
            .build(),
        "deal metadata upsert",
    )?;
    sqlite
        .write(&query)
        .map_err(|error| format!("failed to upsert deal metadata: {error}"))?;
    get_deal_metadata_by_deal_id(sqlite, &record.deal_id)?.ok_or_else(|| {
        format!(
            "upserted metadata for deal `{}` could not be loaded",
            record.deal_id
        )
    })
}

fn archive_deal(sqlite: &SqliteClient, deal_id: &str) -> Result<Option<Deal>, String> {
    let query = build_query(
        SqlBuilder::update("deals")
            .set("status", "Archived")
            .and_where(Condition::equal("deal_id", deal_id))
            .build(),
        "deal archive update",
    )?;
    let updated = sqlite
        .write(&query)
        .map_err(|error| format!("failed to archive deal: {error}"))?;
    if updated.rows_affected == 0 {
        return Ok(None);
    }
    deal_by_id(sqlite, deal_id)
}

fn deal_by_id(sqlite: &SqliteClient, deal_id: &str) -> Result<Option<Deal>, String> {
    let query = build_query(
        SqlBuilder::select("deals")
            .columns(DEAL_COLUMNS)
            .and_where(Condition::equal("deal_id", deal_id))
            .build(),
        "deal select",
    )?;
    sqlite
        .read_one_with(&query, deal_from_row)
        .map_err(|error| format!("failed to read deal: {error}"))
}

fn build_query(
    query: Result<SqlQuery, crate::core::sqlbuilder::SqlBuilderError>,
    operation: &str,
) -> Result<SqlQuery, String> {
    query.map_err(|error| format!("failed to build {operation}: {error}"))
}

fn deal_from_row(row: &Row<'_>) -> rusqlite::Result<Deal> {
    Ok(Deal {
        deal_id: row.get("deal_id")?,
        user_id: row.get("user_id")?,
        deal_name: row.get("deal_name")?,
        status: row.get("status")?,
        start_date: row.get("start_date")?,
        close_date: row.get("close_date")?,
        transaction_type: row.get("transaction_type")?,
        target_company: row.get("target_company")?,
        primary_buyer: row.get("primary_buyer")?,
        deal_sponsor: row.get("deal_sponsor")?,
    })
}

fn deal_metadata_from_row(row: &Row<'_>) -> rusqlite::Result<DealMetadata> {
    Ok(DealMetadata {
        deal_id: row.get("deal_id")?,
        user_id: row.get("user_id")?,
        key_questions_json: row.get("key_questions_json")?,
        local_path: row.get("local_path")?,
        sharepoint_link: row.get("sharepoint_link")?,
    })
}

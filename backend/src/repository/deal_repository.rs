use rusqlite::Row;
use serde::Serialize;

use crate::{
    core::sqlbuilder::{Condition, ConflictUpdate, SortDirection, SqlBuilder, SqlQuery},
    state::AppState,
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

pub struct CreateDealRecord<'a> {
    pub deal_id: &'a str,
    pub user_id: i64,
    pub deal_name: &'a str,
    pub status: &'a str,
    pub start_date: &'a str,
    pub close_date: &'a str,
    pub transaction_type: &'a str,
    pub target_company: &'a str,
    pub primary_buyer: &'a str,
    pub deal_sponsor: &'a str,
}

pub struct UpsertDealMetadataRecord<'a> {
    pub deal_id: &'a str,
    pub user_id: i64,
    pub key_questions_json: &'a str,
    pub local_path: Option<&'a str>,
    pub sharepoint_link: Option<&'a str>,
}

pub fn create_deal(state: &AppState, record: CreateDealRecord<'_>) -> Result<Deal, String> {
    let query = build_query(
        SqlBuilder::insert_into("deals")
            .value("deal_id", record.deal_id)
            .value("user_id", record.user_id)
            .value("deal_name", record.deal_name)
            .value("status", record.status)
            .value("start_date", record.start_date)
            .value("close_date", record.close_date)
            .value("transaction_type", record.transaction_type)
            .value("target_company", record.target_company)
            .value("primary_buyer", record.primary_buyer)
            .value("deal_sponsor", record.deal_sponsor)
            .build(),
        "deal insert",
    )?;
    state
        .sqlite()
        .write(&query)
        .map_err(|error| format!("failed to insert deal: {error}"))?;
    deal_by_id(state, record.deal_id)?
        .ok_or_else(|| format!("inserted deal `{}` could not be loaded", record.deal_id))
}

pub fn get_deal_by_id(state: &AppState, deal_id: &str) -> Result<Option<Deal>, String> {
    deal_by_id(state, deal_id)
}

pub fn list_deals(state: &AppState) -> Result<Vec<DealWithMetadata>, String> {
    let query = build_query(
        SqlBuilder::select("deals")
            .columns(DEAL_COLUMNS)
            .order_by("close_date", SortDirection::Ascending)
            .order_by("deal_id", SortDirection::Ascending)
            .build(),
        "deal list select",
    )?;
    let deals = state
        .sqlite()
        .read_with(&query, deal_from_row)
        .map_err(|error| format!("failed to list deals: {error}"))?
        .into_iter()
        .filter(|deal| !deal.status.eq_ignore_ascii_case("archived"));

    deals
        .map(|deal| {
            let metadata = get_deal_metadata_by_deal_id(state, &deal.deal_id)?;
            Ok(DealWithMetadata { deal, metadata })
        })
        .collect()
}

pub fn get_deal_with_metadata(
    state: &AppState,
    deal_id: &str,
) -> Result<Option<DealWithMetadata>, String> {
    let Some(deal) = get_deal_by_id(state, deal_id)? else {
        return Ok(None);
    };
    let metadata = get_deal_metadata_by_deal_id(state, deal_id)?;
    Ok(Some(DealWithMetadata { deal, metadata }))
}

pub fn get_deal_metadata_by_deal_id(
    state: &AppState,
    deal_id: &str,
) -> Result<Option<DealMetadata>, String> {
    let query = build_query(
        SqlBuilder::select("deal_metadata")
            .columns(DEAL_METADATA_COLUMNS)
            .and_where(Condition::equal("deal_id", deal_id))
            .build(),
        "deal metadata select",
    )?;
    state
        .sqlite()
        .read_one_with(&query, deal_metadata_from_row)
        .map_err(|error| format!("failed to read deal metadata: {error}"))
}

pub fn upsert_deal_metadata(
    state: &AppState,
    record: UpsertDealMetadataRecord<'_>,
) -> Result<DealMetadata, String> {
    let query = build_query(
        SqlBuilder::insert_into("deal_metadata")
            .value("deal_id", record.deal_id)
            .value("user_id", record.user_id)
            .value("key_questions_json", record.key_questions_json)
            .value("local_path", record.local_path)
            .value("sharepoint_link", record.sharepoint_link)
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
    state
        .sqlite()
        .write(&query)
        .map_err(|error| format!("failed to upsert deal metadata: {error}"))?;
    get_deal_metadata_by_deal_id(state, record.deal_id)?.ok_or_else(|| {
        format!(
            "upserted metadata for deal `{}` could not be loaded",
            record.deal_id
        )
    })
}

pub fn archive_deal(state: &AppState, deal_id: &str) -> Result<Option<Deal>, String> {
    let query = build_query(
        SqlBuilder::update("deals")
            .set("status", "Archived")
            .and_where(Condition::equal("deal_id", deal_id))
            .build(),
        "deal archive update",
    )?;
    let updated = state
        .sqlite()
        .write(&query)
        .map_err(|error| format!("failed to archive deal: {error}"))?;
    if updated.rows_affected == 0 {
        return Ok(None);
    }
    get_deal_by_id(state, deal_id)
}

fn deal_by_id(state: &AppState, deal_id: &str) -> Result<Option<Deal>, String> {
    let query = build_query(
        SqlBuilder::select("deals")
            .columns(DEAL_COLUMNS)
            .and_where(Condition::equal("deal_id", deal_id))
            .build(),
        "deal select",
    )?;
    state
        .sqlite()
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

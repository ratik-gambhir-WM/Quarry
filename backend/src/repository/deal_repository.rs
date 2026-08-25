use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;

use crate::state::AppState;

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
    state.with_db(|db| {
        db.execute(
            r#"
            INSERT INTO deals (
                deal_id, user_id, deal_name, status, start_date, close_date,
                transaction_type, target_company, primary_buyer, deal_sponsor
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                record.deal_id,
                record.user_id,
                record.deal_name,
                record.status,
                record.start_date,
                record.close_date,
                record.transaction_type,
                record.target_company,
                record.primary_buyer,
                record.deal_sponsor,
            ],
        )?;
        deal_by_id(db, record.deal_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
    })
}

pub fn get_deal_by_id(state: &AppState, deal_id: &str) -> Result<Option<Deal>, String> {
    state.with_db(|db| deal_by_id(db, deal_id))
}

pub fn list_deals(state: &AppState) -> Result<Vec<DealWithMetadata>, String> {
    let deals = state.with_db(|db| {
        let mut statement = db.prepare(
            r#"
            SELECT deal_id, user_id, deal_name, status, start_date, close_date,
                   transaction_type, target_company, primary_buyer, deal_sponsor
            FROM deals
            WHERE lower(status) <> 'archived'
            ORDER BY close_date ASC, deal_id ASC
            "#,
        )?;
        let deals = statement
            .query_map([], deal_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(deals)
    })?;

    deals
        .into_iter()
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
    state.with_db(|db| {
        db.query_row(
            r#"
            SELECT deal_id, user_id, key_questions_json, local_path, sharepoint_link
            FROM deal_metadata
            WHERE deal_id = ?1
            "#,
            [deal_id],
            deal_metadata_from_row,
        )
        .optional()
    })
}

pub fn upsert_deal_metadata(
    state: &AppState,
    record: UpsertDealMetadataRecord<'_>,
) -> Result<DealMetadata, String> {
    state.with_db(|db| {
        db.execute(
            r#"
            INSERT INTO deal_metadata (
                deal_id, user_id, key_questions_json, local_path, sharepoint_link
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(deal_id) DO UPDATE SET
                user_id = excluded.user_id,
                key_questions_json = excluded.key_questions_json,
                local_path = excluded.local_path,
                sharepoint_link = excluded.sharepoint_link
            "#,
            params![
                record.deal_id,
                record.user_id,
                record.key_questions_json,
                record.local_path,
                record.sharepoint_link,
            ],
        )?;
        db.query_row(
            r#"
            SELECT deal_id, user_id, key_questions_json, local_path, sharepoint_link
            FROM deal_metadata
            WHERE deal_id = ?1
            "#,
            [record.deal_id],
            deal_metadata_from_row,
        )
    })
}

pub fn archive_deal(state: &AppState, deal_id: &str) -> Result<Option<Deal>, String> {
    let updated = state.with_db(|db| {
        db.execute(
            "UPDATE deals SET status = 'Archived' WHERE deal_id = ?1",
            [deal_id],
        )
    })?;
    if updated == 0 {
        return Ok(None);
    }
    get_deal_by_id(state, deal_id)
}

fn deal_by_id(db: &rusqlite::Connection, deal_id: &str) -> rusqlite::Result<Option<Deal>> {
    db.query_row(
        r#"
        SELECT deal_id, user_id, deal_name, status, start_date, close_date,
               transaction_type, target_company, primary_buyer, deal_sponsor
        FROM deals
        WHERE deal_id = ?1
        "#,
        [deal_id],
        deal_from_row,
    )
    .optional()
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

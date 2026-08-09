use rusqlite::{params, OptionalExtension, Row};
use serde::Serialize;
use serde_json::Value;

use crate::{
    core::{
        helix_queries::deals::add_deal::{add_deal as build_add_deal_query, create_deal_indexes},
        nodes::deal_node::DealNode,
    },
    state::AppState,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deal {
    pub id: i64,
    pub deal_name: String,
    pub main_data_room_folder: String,
    pub deal_type: String,
    pub pe_firm: String,
    pub status: String,
    pub target_company: Option<String>,
    pub buyer_or_platform_company: Option<String>,
    pub parent_or_seller_company: Option<String>,
    pub carve_out_business: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealMetadata {
    pub id: i64,
    pub deal_id: i64,
    pub key_questions_json: String,
    pub legacy_investment_thesis: Option<String>,
    pub document_count: i64,
    pub data_room_size_bytes: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DealWithMetadata {
    #[serde(flatten)]
    pub deal: Deal,
    pub metadata: Option<DealMetadata>,
}

pub struct CreateDealRecord<'a> {
    pub deal_name: &'a str,
    pub main_data_room_folder: &'a str,
    pub deal_type: &'a str,
    pub pe_firm: &'a str,
    pub target_company: Option<&'a str>,
    pub buyer_or_platform_company: Option<&'a str>,
    pub parent_or_seller_company: Option<&'a str>,
    pub carve_out_business: Option<&'a str>,
}

pub struct UpsertDealMetadataRecord<'a> {
    pub deal_id: i64,
    pub key_questions_json: &'a str,
    pub legacy_investment_thesis: Option<&'a str>,
    pub document_count: i64,
    pub data_room_size_bytes: i64,
}

pub fn create_deal(state: &AppState, record: CreateDealRecord<'_>) -> Result<Deal, String> {
    let deal_id = state.with_sqlite_db(|db| {
        db.execute(
            r#"
            INSERT INTO deals (
                deal_name,
                main_data_room_folder,
                deal_type,
                pe_firm,
                target_company,
                buyer_or_platform_company,
                parent_or_seller_company,
                carve_out_business
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                record.deal_name,
                record.main_data_room_folder,
                record.deal_type,
                record.pe_firm,
                record.target_company,
                record.buyer_or_platform_company,
                record.parent_or_seller_company,
                record.carve_out_business
            ],
        )?;
        Ok(db.last_insert_rowid())
    })?;

    get_deal_by_id(state, deal_id)?
        .ok_or_else(|| format!("failed to fetch deal after insert for id `{deal_id}`"))
}

/// Persists a complete deal row and its user relationship through Helix.
pub async fn upsert_wm_deal(
    state: &AppState,
    deal: DealNode,
    user_id: i64,
) -> Result<Value, String> {
    let query = build_add_deal_query(deal, user_id)?;
    let helix = state.gen_helix_db_client();

    let _: Value = helix.execute_dynamic_query(create_deal_indexes).await?;

    helix.execute_dynamic_query(move || query).await
}

pub fn upsert_deal_metadata(
    state: &AppState,
    record: UpsertDealMetadataRecord<'_>,
) -> Result<DealMetadata, String> {
    state.with_sqlite_db(|db| {
        db.execute(
            r#"
            INSERT INTO deal_metadata (
                deal_id,
                key_questions_json,
                legacy_investment_thesis,
                document_count,
                data_room_size_bytes
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(deal_id) DO UPDATE SET
                key_questions_json = excluded.key_questions_json,
                legacy_investment_thesis = excluded.legacy_investment_thesis,
                document_count = excluded.document_count,
                data_room_size_bytes = excluded.data_room_size_bytes,
                updated_at = CURRENT_TIMESTAMP
            "#,
            params![
                record.deal_id,
                record.key_questions_json,
                record.legacy_investment_thesis,
                record.document_count,
                record.data_room_size_bytes
            ],
        )?;

        db.query_row(
            r#"
            SELECT
                id,
                deal_id,
                key_questions_json,
                legacy_investment_thesis,
                document_count,
                data_room_size_bytes,
                created_at,
                updated_at
            FROM deal_metadata
            WHERE deal_id = ?1
            "#,
            [record.deal_id],
            deal_metadata_from_row,
        )
    })
}

pub fn get_deal_by_id(state: &AppState, deal_id: i64) -> Result<Option<Deal>, String> {
    let deals = state.gen_sqlite_db_client().query_rows(
        r#"
        SELECT
            id,
            deal_name,
            main_data_room_folder,
            deal_type,
            pe_firm,
            status,
            target_company,
            buyer_or_platform_company,
            parent_or_seller_company,
            carve_out_business,
            created_at,
            updated_at
        FROM deals
        WHERE id = ?1
        "#,
        [deal_id],
        deal_from_row,
    )?;

    Ok(deals.into_iter().next())
}

pub fn list_deals(state: &AppState) -> Result<Vec<DealWithMetadata>, String> {
    let deals = state.gen_sqlite_db_client().query_rows(
        r#"
        SELECT
            id,
            deal_name,
            main_data_room_folder,
            deal_type,
            pe_firm,
            status,
            target_company,
            buyer_or_platform_company,
            parent_or_seller_company,
            carve_out_business,
            created_at,
            updated_at
        FROM deals
        WHERE status = 'active'
        ORDER BY updated_at DESC, id DESC
        "#,
        [],
        deal_from_row,
    )?;

    deals
        .into_iter()
        .map(|deal| {
            let metadata = get_deal_metadata_by_deal_id(state, deal.id)?;
            Ok(DealWithMetadata { deal, metadata })
        })
        .collect()
}

pub fn get_deal_with_metadata(
    state: &AppState,
    deal_id: i64,
) -> Result<Option<DealWithMetadata>, String> {
    let Some(deal) = get_deal_by_id(state, deal_id)? else {
        return Ok(None);
    };
    let metadata = get_deal_metadata_by_deal_id(state, deal_id)?;
    Ok(Some(DealWithMetadata { deal, metadata }))
}

pub fn get_deal_metadata_by_deal_id(
    state: &AppState,
    deal_id: i64,
) -> Result<Option<DealMetadata>, String> {
    state.with_sqlite_db(|db| {
        db.query_row(
            r#"
            SELECT
                id,
                deal_id,
                key_questions_json,
            legacy_investment_thesis,
                document_count,
                data_room_size_bytes,
                created_at,
                updated_at
            FROM deal_metadata
            WHERE deal_id = ?1
            "#,
            [deal_id],
            deal_metadata_from_row,
        )
        .optional()
    })
}

pub fn archive_deal(state: &AppState, deal_id: i64) -> Result<Option<Deal>, String> {
    let updated = state.with_sqlite_db(|db| {
        db.execute(
            "UPDATE deals SET status = 'archived', updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [deal_id],
        )
    })?;

    if updated == 0 {
        return Ok(None);
    }

    get_deal_by_id(state, deal_id)
}

fn deal_from_row(row: &Row<'_>) -> rusqlite::Result<Deal> {
    Ok(Deal {
        id: row.get("id")?,
        deal_name: row.get("deal_name")?,
        main_data_room_folder: row.get("main_data_room_folder")?,
        deal_type: row.get("deal_type")?,
        pe_firm: row.get("pe_firm")?,
        status: row.get("status")?,
        target_company: row.get("target_company")?,
        buyer_or_platform_company: row.get("buyer_or_platform_company")?,
        parent_or_seller_company: row.get("parent_or_seller_company")?,
        carve_out_business: row.get("carve_out_business")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn deal_metadata_from_row(row: &Row<'_>) -> rusqlite::Result<DealMetadata> {
    Ok(DealMetadata {
        id: row.get("id")?,
        deal_id: row.get("deal_id")?,
        key_questions_json: row.get("key_questions_json")?,
        legacy_investment_thesis: row.get("legacy_investment_thesis")?,
        document_count: row.get("document_count")?,
        data_room_size_bytes: row.get("data_room_size_bytes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
#[path = "../../tests/repository/deal_repository_tests.rs"]
mod tests;

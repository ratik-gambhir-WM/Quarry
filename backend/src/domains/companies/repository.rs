use super::model::{Company, CompanyInput};
use crate::{
    adapters::sqlite::{
        client::{SqliteClient, SqliteClientError},
        query::{ComparisonOperator, Condition, SortDirection, SqlBuilder},
    },
    shared::error::RepositoryError,
};
#[derive(Clone)]
pub struct CompanyRepository {
    sqlite: SqliteClient,
}
impl CompanyRepository {
    pub fn new(sqlite: SqliteClient) -> Self {
        Self { sqlite }
    }
    pub async fn list(
        &self,
        user_id: i64,
        query: Option<String>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Company>, RepositoryError> {
        let mut builder = SqlBuilder::select("companies")
            .and_where(Condition::equal("user_id", user_id))
            .order_by("name", SortDirection::Ascending)
            .order_by("company_id", SortDirection::Ascending)
            .limit(limit)
            .offset(offset);
        if let Some(query) = query {
            builder = builder.and_where(Condition::compare(
                "name",
                ComparisonOperator::Like,
                format!("%{query}%"),
            ));
        }
        self.sqlite
            .read_with_async(builder.build(), company_from_row)
            .await
            .map_err(storage)
    }
    pub async fn save(
        &self,
        user_id: i64,
        company_id: Option<String>,
        input: CompanyInput,
    ) -> Result<Company, RepositoryError> {
        self.sqlite
            .transaction_async(move |tx| {
                let now = chrono::Utc::now().to_rfc3339();
                let id = company_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                if company_id.is_some() {
                    let existing = tx.read(
                        SqlBuilder::select("companies")
                            .and_where(Condition::equal("company_id", &id))
                            .and_where(Condition::equal("user_id", user_id))
                            .build(),
                    )?;
                    if existing.is_empty() {
                        return Ok(None);
                    }
                    tx.write(
                        SqlBuilder::update("companies")
                            .set("name", input.name.trim())
                            .set("website", input.website.as_deref())
                            .set("industry", input.industry.as_deref())
                            .set("description", input.description.as_deref())
                            .set("updated_at", &now)
                            .and_where(Condition::equal("company_id", &id))
                            .and_where(Condition::equal("user_id", user_id))
                            .build(),
                    )?;
                } else {
                    tx.write(
                        SqlBuilder::insert_into("companies")
                            .value("company_id", &id)
                            .value("user_id", user_id)
                            .value("name", input.name.trim())
                            .value("website", input.website.as_deref())
                            .value("industry", input.industry.as_deref())
                            .value("description", input.description.as_deref())
                            .value("created_at", &now)
                            .value("updated_at", &now)
                            .build(),
                    )?;
                }
                Ok(tx
                    .read_with(
                        SqlBuilder::select("companies")
                            .and_where(Condition::equal("company_id", &id))
                            .build(),
                        company_from_row,
                    )?
                    .pop())
            })
            .await
            .map_err(storage)?
            .ok_or_else(|| RepositoryError::NotFound("company not found".into()))
    }
}
fn storage(e: SqliteClientError) -> RepositoryError {
    RepositoryError::storage(e.to_string())
}
pub(crate) fn company_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Company> {
    Ok(Company {
        company_id: row.get("company_id")?,
        user_id: row.get("user_id")?,
        name: row.get("name")?,
        website: row.get("website")?,
        industry: row.get("industry")?,
        description: row.get("description")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

use super::model::{Company, CompanyInput};
use crate::adapters::postgres::client::RowResult;
use crate::{
    adapters::postgres::{
        client::{PostgresClient, PostgresError},
        query::{ComparisonOperator, Condition, SortDirection, SqlBuilder},
    },
    shared::error::RepositoryError,
};
#[derive(Clone)]
pub struct CompanyRepository {
    postgres: PostgresClient,
}
impl CompanyRepository {
    pub fn new(postgres: PostgresClient) -> Self {
        Self { postgres }
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
        self.postgres
            .read_with_async(builder.build(), company_from_row)
            .await
            .map_err(storage)
    }
    pub async fn save(
        &self,
        user_id: i64,
        company_id: Option<String>,
        input: CompanyInput,
        expected_revision: Option<i64>,
    ) -> Result<Company, RepositoryError> {
        self.postgres
            .transaction_result_async(move |tx| async move {
                let tx = &tx;
                let now = chrono::Utc::now().to_rfc3339();
                let id = company_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                if company_id.is_some() {
                    let expected =
                        expected_revision.ok_or_else(crate::shared::revision::conflict)?;
                    let changed = tx
                        .write(
                            SqlBuilder::update("companies")
                                .set("name", input.name.trim())
                                .set("website", input.website.as_deref())
                                .set("industry", input.industry.as_deref())
                                .set("description", input.description.as_deref())
                                .set("updated_at", &now)
                                .increment("revision")
                                .and_where(Condition::equal("company_id", &id))
                                .and_where(Condition::equal("user_id", user_id))
                                .and_where(Condition::equal("revision", expected))
                                .and_where(Condition::compare(
                                    "revision",
                                    ComparisonOperator::LessThan,
                                    crate::shared::revision::MAX_REVISION,
                                ))
                                .build(),
                        )
                        .await?;
                    if changed.rows_affected != 1 {
                        let exists = tx
                            .read_one(
                                SqlBuilder::select("companies")
                                    .and_where(Condition::equal("company_id", &id))
                                    .and_where(Condition::equal("user_id", user_id))
                                    .build(),
                            )
                            .await?
                            .is_some();
                        return Err(if exists {
                            crate::shared::revision::conflict()
                        } else {
                            RepositoryError::NotFound("company not found".into())
                        });
                    }
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
                    )
                    .await?;
                }
                Ok(tx
                    .read_with(
                        SqlBuilder::select("companies")
                            .and_where(Condition::equal("company_id", &id))
                            .build(),
                        company_from_row,
                    )
                    .await?
                    .pop())
            })
            .await?
            .ok_or_else(|| RepositoryError::NotFound("company not found".into()))
    }
}
fn storage(e: PostgresError) -> RepositoryError {
    RepositoryError::storage(e.to_string())
}
pub(crate) fn company_from_row(
    row: &crate::adapters::postgres::client::Row<'_>,
) -> RowResult<Company> {
    Ok(Company {
        company_id: row.get("company_id")?,
        revision: row.get("revision")?,
        user_id: row.get("user_id")?,
        name: row.get("name")?,
        website: row.get("website")?,
        industry: row.get("industry")?,
        description: row.get("description")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

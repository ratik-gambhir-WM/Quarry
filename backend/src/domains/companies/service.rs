use super::{
    model::{Company, CompanyInput, CompanyPage},
    repository::CompanyRepository,
};
use crate::{
    domains::users::UserDirectory,
    shared::error::{ServiceError, ServiceResult},
};
#[derive(Clone)]
pub struct CompanyService {
    companies: CompanyRepository,
    users: UserDirectory,
}
impl CompanyService {
    pub fn new(companies: CompanyRepository, users: UserDirectory) -> Self {
        Self { companies, users }
    }
    async fn owner(&self, email: &str) -> ServiceResult<i64> {
        self.users
            .by_email(email.trim())
            .await?
            .map(|u| u.id)
            .ok_or_else(|| ServiceError::not_found("user not found"))
    }
    pub async fn list(
        &self,
        email: &str,
        query: Option<String>,
        limit: usize,
        offset: usize,
    ) -> ServiceResult<CompanyPage> {
        let mut companies = self
            .companies
            .list(self.owner(email).await?, query, limit + 1, offset)
            .await?;
        let has_more = companies.len() > limit;
        companies.truncate(limit);
        Ok(CompanyPage {
            companies,
            has_more,
        })
    }
    pub async fn save(&self, id: Option<String>, input: CompanyInput) -> ServiceResult<Company> {
        self.companies
            .save(self.owner(&input.user_email).await?, id, input)
            .await
            .map_err(Into::into)
    }
}

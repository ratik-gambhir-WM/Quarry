//! Small reusable dataset; each caller owns a fresh, migrated PostgreSQL namespace.
use crate::{
    adapters::postgres::client::PostgresClient,
    app::migrations,
    domains::{
        companies::{model::CompanyInput, repository::CompanyRepository, Company},
        users::repository::UserRepository,
    },
    integration_support::seed_identity,
};

pub(crate) const OWNER_EMAIL: &str = "crud-owner@example.invalid";

pub(crate) struct DatabaseFixture {
    pub postgres: PostgresClient,
    pub companies: CompanyRepository,
    pub owner: i64,
    pub other_owner: i64,
    pub company: Company,
}

pub(crate) fn company_input(name: &str) -> CompanyInput {
    CompanyInput {
        user_email: OWNER_EMAIL.into(),
        name: name.into(),
        website: Some("https://example.invalid".into()),
        industry: Some("Software".into()),
        description: None,
    }
}

pub(crate) async fn seeded_database() -> DatabaseFixture {
    let postgres = PostgresClient::test_database().await.unwrap();
    migrations::migrate(&postgres).await.unwrap();
    seed_identity(&postgres, OWNER_EMAIL, 'a').await;
    seed_identity(&postgres, "crud-other@example.invalid", 'b').await;
    let users = UserRepository::new(postgres.clone());
    let owner = users
        .by_email(OWNER_EMAIL.into())
        .await
        .unwrap()
        .unwrap()
        .id;
    let other_owner = users
        .by_email("crud-other@example.invalid".into())
        .await
        .unwrap()
        .unwrap()
        .id;
    let companies = CompanyRepository::new(postgres.clone());
    let company = companies
        .save(owner, None, company_input("Fixture Software"), None)
        .await
        .unwrap();
    let mut other_input = company_input("Other Software");
    other_input.user_email = "crud-other@example.invalid".into();
    companies
        .save(other_owner, None, other_input, None)
        .await
        .unwrap();
    DatabaseFixture {
        postgres,
        companies,
        owner,
        other_owner,
        company,
    }
}

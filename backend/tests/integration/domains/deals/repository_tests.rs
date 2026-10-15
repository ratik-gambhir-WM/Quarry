use super::*;
use crate::{
    app::migrations::migrate,
    domains::{
        companies::{model::CompanyInput, repository::CompanyRepository},
        users::repository::{AddUserInput, UserRepository},
    },
};

fn fields() -> DealFields {
    serde_json::from_value(serde_json::json!({
        "dealName":"Project Test", "dealCode":null, "engagementType":"sell_side",
        "transactionType":"unknown", "status":"pipeline", "startDate":null,
        "expectedCloseDate":null, "actualCloseDate":null, "engagementCompletedDate":null,
        "parties":[], "scopes":[]
    }))
    .unwrap()
}
async fn fixture() -> (SqliteClient, DealRepository, CompanyRepository, i64) {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrate(&sqlite).unwrap();
    let user = UserRepository::new(sqlite.clone())
        .create(AddUserInput {
            first_name: "Test".into(),
            last_name: "Owner".into(),
            email: "owner@example.com".into(),
            api_key: "unused".into(),
            role: "Analyst".into(),
        })
        .await
        .unwrap();
    (
        sqlite.clone(),
        DealRepository::new(sqlite.clone()),
        CompanyRepository::new(sqlite),
        user.id,
    )
}
async fn company(
    repo: &CompanyRepository,
    owner: i64,
    name: &str,
) -> crate::domains::companies::Company {
    repo.save(
        owner,
        None,
        CompanyInput {
            user_email: "owner@example.com".into(),
            name: name.into(),
            website: None,
            industry: None,
            description: None,
        },
    )
    .await
    .unwrap()
}
async fn create(
    repo: &DealRepository,
    owner: i64,
    id: &str,
    fields: DealFields,
) -> Result<Deal, RepositoryError> {
    repo.create(CreateDealRecord {
        deal_id: id.into(),
        user_id: owner,
        fields,
        local_path: None,
        sharepoint_link: None,
    })
    .await
    .map(|aggregate| aggregate.deal)
}
#[tokio::test]
async fn role_reuse_shared_company_edits_and_duplicate_names_round_trip() {
    let (_, repo, directory, owner) = fixture().await;
    let buyer = company(&directory, owner, "Same Name").await;
    let target = company(&directory, owner, "Same Name").await;
    assert_ne!(buyer.company_id, target.company_id);
    let mut input = fields();
    for role in [
        PartyRole::Buyer,
        PartyRole::PortfolioCompany,
        PartyRole::Client,
    ] {
        input
            .parties
            .push(crate::domains::deals::model::PartyInput {
                company_id: buyer.company_id.clone(),
                role,
                notes: None,
            });
    }
    input
        .parties
        .push(crate::domains::deals::model::PartyInput {
            company_id: target.company_id.clone(),
            role: PartyRole::Target,
            notes: None,
        });
    input.scopes = vec![DiligenceScope::CorporateIt, DiligenceScope::Cybersecurity];
    let first = create(&repo, owner, "DEAL-1", input.clone()).await.unwrap();
    create(&repo, owner, "DEAL-2", input.clone()).await.unwrap();
    directory
        .save(
            owner,
            Some(buyer.company_id.clone()),
            CompanyInput {
                user_email: "owner@example.com".into(),
                name: "Updated Buyer".into(),
                website: None,
                industry: Some("Software".into()),
                description: None,
            },
        )
        .await
        .unwrap();
    let listed = repo.list().await.unwrap();
    assert_eq!(listed.len(), 2);
    for row in listed {
        assert_eq!(
            row.deal
                .parties
                .iter()
                .filter(|p| p.company.name == "Updated Buyer")
                .count(),
            3
        );
    }
    input.parties[0].notes = Some("Deal-specific".into());
    let updated = repo.update("DEAL-1".into(), input).await.unwrap();
    for party in &updated.deal.parties {
        assert_eq!(
            first
                .parties
                .iter()
                .find(|p| p.company_id == party.company_id && p.role == party.role)
                .unwrap()
                .deal_party_id,
            party.deal_party_id
        );
    }
    assert_eq!(updated.deal.scopes.len(), 2);
}
#[tokio::test]
async fn failed_child_write_rolls_back_scalars_parties_scopes_and_creation() {
    let (sqlite, repo, directory, owner) = fixture().await;
    let c = company(&directory, owner, "Target").await;
    let mut input = fields();
    input
        .parties
        .push(crate::domains::deals::model::PartyInput {
            company_id: c.company_id,
            role: PartyRole::Target,
            notes: None,
        });
    let before = create(&repo, owner, "DEAL-1", input.clone()).await.unwrap();
    sqlite.with_connection(|c| c.execute_batch("CREATE TRIGGER reject_scope BEFORE INSERT ON deal_scopes BEGIN SELECT RAISE(ABORT, 'test failure'); END;")).unwrap();
    input.deal_name = "Changed".into();
    input.parties.clear();
    input.scopes.push(DiligenceScope::Sdlc);
    assert!(repo.update("DEAL-1".into(), input.clone()).await.is_err());
    let after = repo.by_id("DEAL-1".into()).await.unwrap().unwrap();
    assert_eq!(after.deal_name, before.deal_name);
    assert_eq!(
        after.parties[0].deal_party_id,
        before.parties[0].deal_party_id
    );
    assert!(after.scopes.is_empty());
    assert!(create(&repo, owner, "DEAL-FAIL", input).await.is_err());
    assert!(repo
        .with_metadata("DEAL-FAIL".into())
        .await
        .unwrap()
        .is_none());
    assert!(repo.metadata("DEAL-FAIL".into()).await.unwrap().is_none());
}
#[tokio::test]
async fn missing_cross_owner_and_duplicate_assignments_never_commit() {
    let (sqlite, repo, directory, owner) = fixture().await;
    let second = UserRepository::new(sqlite)
        .create(AddUserInput {
            first_name: "Other".into(),
            last_name: "Owner".into(),
            email: "other@example.com".into(),
            api_key: "unused".into(),
            role: "Analyst".into(),
        })
        .await
        .unwrap();
    let c = company(&directory, second.id, "Other Company").await;
    let mut input = fields();
    input
        .parties
        .push(crate::domains::deals::model::PartyInput {
            company_id: c.company_id,
            role: PartyRole::Target,
            notes: None,
        });
    assert!(matches!(
        create(&repo, owner, "DEAL-1", input.clone()).await,
        Err(RepositoryError::Conflict(_))
    ));
    input.parties[0].company_id = uuid::Uuid::new_v4().to_string();
    assert!(matches!(
        create(&repo, owner, "DEAL-1", input.clone()).await,
        Err(RepositoryError::NotFound(_))
    ));
    let c = company(&directory, owner, "Owned Company").await;
    input.parties[0].company_id = c.company_id;
    input.parties.push(input.parties[0].clone());
    assert!(create(&repo, owner, "DEAL-1", input).await.is_err());
    assert!(repo.list().await.unwrap().is_empty());
}
#[tokio::test]
async fn archive_is_idempotent_retains_status_and_nullable_dates_sort_last() {
    let (_, repo, _, owner) = fixture().await;
    create(&repo, owner, "DEAL-Z", fields()).await.unwrap();
    let mut input = fields();
    input.actual_close_date = Some("2025-01-01".into());
    create(&repo, owner, "DEAL-A", input).await.unwrap();
    assert_eq!(repo.list().await.unwrap()[0].deal.deal_id, "DEAL-A");
    let archived = repo.archive("DEAL-A".into()).await.unwrap().unwrap();
    assert_eq!(
        archived.status,
        crate::domains::deals::model::DealStatus::Pipeline
    );
    assert_eq!(
        archived.archived_at,
        repo.archive("DEAL-A".into())
            .await
            .unwrap()
            .unwrap()
            .archived_at
    );
    assert_eq!(repo.list().await.unwrap().len(), 1);
    assert!(repo.with_metadata("DEAL-A".into()).await.unwrap().is_some());
}

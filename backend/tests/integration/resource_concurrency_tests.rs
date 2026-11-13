//! Domain races on independent PostgreSQL pools with runtime serialization disabled
//! only in the test schema. No application setting bypasses production controls.
use crate::{
    adapters::postgres::client::PostgresClient,
    domains::{
        companies::{model::CompanyInput, repository::CompanyRepository},
        deals::{
            model::DealFields,
            repository::{CreateDealRecord, DealRepository, UpsertDealMetadataRecord},
        },
        memberships::{model::DealRole, repository::MembershipRepository},
    },
    shared::error::RepositoryError,
};
use std::time::Duration;

fn fields() -> DealFields {
    serde_json::from_value(serde_json::json!({"dealName":"Concurrent", "dealCode":null,
        "engagementType":"sell_side", "transactionType":"unknown", "status":"pipeline",
        "startDate":null,"expectedCloseDate":null,"actualCloseDate":null,"engagementCompletedDate":null,
        "parties":[],"scopes":[]})).unwrap()
}
async fn fixture() -> (PostgresClient, PostgresClient) {
    let (a, b) = PostgresClient::concurrent_test_clients().await;
    crate::integration_support::seed_identity(&a, "race-owner@example.invalid", 'a').await;
    crate::integration_support::seed_identity(&a, "race-manager@example.invalid", 'b').await;
    let repo = DealRepository::new(a.clone());
    for id in ["DEAL-A", "DEAL-B"] {
        repo.create(CreateDealRecord {
            deal_id: id.into(),
            user_id: 1,
            fields: fields(),
            local_path: None,
            sharepoint_link: None,
        })
        .await
        .unwrap();
    }
    (a, b)
}
fn metadata(expected_revision: i64, text: &str) -> UpsertDealMetadataRecord {
    UpsertDealMetadataRecord {
        expected_revision,
        deal_id: "DEAL-A".into(),
        key_questions_json: serde_json::to_string(&[text]).unwrap(),
        local_path: None,
        sharepoint_link: None,
        sow_link: None,
        fact_sheet_link: None,
        rl_link: None,
    }
}
#[tokio::test]
async fn same_revision_deal_and_metadata_edits_have_exactly_one_winner() {
    let (a, b) = fixture().await;
    let ra = DealRepository::new(a.clone());
    let rb = DealRepository::new(b.clone());
    let (one, two) = tokio::join!(
        ra.update("DEAL-A".into(), 1, fields(), 1),
        rb.update("DEAL-A".into(), 1, fields(), 1)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert!(matches!(
        one.err().or(two.err()),
        Some(RepositoryError::Conflict(_))
    ));
    let (one, two) = tokio::join!(
        ra.upsert_metadata(metadata(1, "first"), 1),
        rb.upsert_metadata(metadata(1, "second"), 1)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    let row = ra.with_metadata("DEAL-A".into()).await.unwrap().unwrap();
    assert_eq!(row.deal.revision, 2);
    assert_eq!(row.metadata.unwrap().revision, 2);
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM deal_activity_events WHERE kind IN ('deal_updated','metadata_updated')")
        .fetch_one(a.pool()).await.unwrap();
    assert_eq!(count, 2, "losing edits must roll back their events");
    a.close().await;
    b.close().await;
}
#[tokio::test]
async fn an_exclusive_guard_blocks_its_deal_while_another_deal_commits() {
    let (a, b) = fixture().await;
    let mut held = a.pool().begin().await.unwrap();
    sqlx::query("SELECT state FROM deal_lifecycle_guards WHERE deal_id='DEAL-A' FOR UPDATE")
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let repo = DealRepository::new(a.clone());
    let waiting = tokio::spawn(async move { repo.update("DEAL-A".into(), 1, fields(), 1).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(pid)
            .fetch_one(b.pool())
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        DealRepository::new(b.clone()).update("DEAL-B".into(), 1, fields(), 1),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.deal.revision, 2);
    assert!(!waiting.is_finished());
    held.commit().await.unwrap();
    assert_eq!(waiting.await.unwrap().unwrap().deal.revision, 2);
    a.close().await;
    b.close().await;
}
#[tokio::test]
async fn company_replacement_is_atomic_and_inaccessible_stale_input_stays_hidden() {
    let (a, b) = fixture().await;
    let ca = CompanyRepository::new(a.clone());
    let cb = CompanyRepository::new(b.clone());
    let input = CompanyInput {
        user_email: String::new(),
        name: "Original".into(),
        website: None,
        industry: None,
        description: None,
    };
    let company = ca.save(1, None, input.clone(), None).await.unwrap();
    let (one, two) = tokio::join!(
        ca.save(1, Some(company.company_id.clone()), input.clone(), Some(1)),
        cb.save(1, Some(company.company_id.clone()), input.clone(), Some(1))
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert!(matches!(
        cb.save(
            2,
            Some(company.company_id),
            input,
            Some(crate::shared::revision::MAX_REVISION)
        )
        .await,
        Err(RepositoryError::NotFound(_))
    ));
    a.close().await;
    b.close().await;
}
#[tokio::test]
async fn revocation_and_no_op_membership_edits_retain_atomic_authority() {
    let (a, b) = fixture().await;
    let ma = MembershipRepository::new(a.clone());
    let mb = MembershipRepository::new(b.clone());
    ma.add_member(
        "DEAL-A".into(),
        1,
        "race-manager@example.invalid".into(),
        DealRole::Manager,
    )
    .await
    .unwrap();
    ma.change_role("DEAL-A".into(), 1, 2, DealRole::Manager, 1)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM deal_activity_events WHERE kind='member_role_changed'",
    )
    .fetch_one(a.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    let (one, two) = tokio::join!(
        ma.change_role("DEAL-A".into(), 1, 2, DealRole::Viewer, 1),
        mb.remove_member("DEAL-A".into(), 1, 2, 1)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    let owners: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM deal_memberships WHERE deal_id='DEAL-A' AND role='owner'",
    )
    .fetch_one(a.pool())
    .await
    .unwrap();
    assert_eq!(owners, 1);
    assert!(DealRepository::new(b.clone())
        .update("DEAL-A".into(), 2, fields(), 1)
        .await
        .is_err());
    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn revision_exhaustion_keeps_content_and_activity_unchanged() {
    let (a, b) = fixture().await;
    sqlx::query("UPDATE deals SET content_revision=9007199254740991 WHERE deal_id='DEAL-A'")
        .execute(a.pool())
        .await
        .unwrap();
    let deals = DealRepository::new(b.clone());
    assert!(matches!(
        deals
            .update(
                "DEAL-A".into(),
                1,
                fields(),
                crate::shared::revision::MAX_REVISION
            )
            .await,
        Err(RepositoryError::Conflict(_))
    ));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM deal_activity_events WHERE kind='deal_updated'")
            .fetch_one(a.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
    assert!(sqlx::query(
        "UPDATE deals SET content_revision=9007199254740992 WHERE deal_id='DEAL-A'"
    )
    .execute(a.pool())
    .await
    .is_err());
    a.close().await;
    b.close().await;
}

use crate::{
    adapters::postgres::query::{Condition, SqlBuilder},
    app::migrations,
    database_fixtures::{company_input, seeded_database},
};

#[tokio::test]
async fn seeded_queries_are_scoped_and_survive_migration_validation() {
    let fixture = seeded_database().await;
    migrations::migrate(&fixture.postgres).await.unwrap();
    let owned = fixture
        .companies
        .list(fixture.owner, Some("Fixture".into()), 10, 0)
        .await
        .unwrap();
    assert_eq!(owned.len(), 1);
    assert_eq!(owned[0].company_id, fixture.company.company_id);
    assert_eq!(owned[0].industry.as_deref(), Some("Software"));
    assert!(fixture
        .companies
        .list(fixture.other_owner, Some("Fixture".into()), 10, 0)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn insert_update_and_delete_round_trip_without_changing_other_owners() {
    let fixture = seeded_database().await;
    let inserted = fixture
        .companies
        .save(fixture.owner, None, company_input("New Company"), None)
        .await
        .unwrap();
    assert_eq!(inserted.revision, 1);
    let updated = fixture
        .companies
        .save(
            fixture.owner,
            Some(inserted.company_id.clone()),
            company_input("Updated Company"),
            Some(inserted.revision),
        )
        .await
        .unwrap();
    assert_eq!(updated.revision, 2);
    let queried = fixture
        .companies
        .list(fixture.owner, Some("Updated".into()), 10, 0)
        .await
        .unwrap();
    assert_eq!(queried.len(), 1);
    assert_eq!(queried[0].company_id, inserted.company_id);
    assert_eq!(queried[0].name, "Updated Company");
    // Companies have no product delete operation; exercise the canonical typed SQL client.
    let deleted = fixture
        .postgres
        .write_async(
            SqlBuilder::delete_from("companies")
                .and_where(Condition::equal("company_id", inserted.company_id))
                .and_where(Condition::equal("user_id", fixture.owner))
                .build(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.rows_affected, 1);
    let remaining = fixture
        .companies
        .list(fixture.owner, None, 10, 0)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].company_id, fixture.company.company_id);
    let other = fixture
        .companies
        .list(fixture.other_owner, None, 10, 0)
        .await
        .unwrap();
    assert_eq!(other.len(), 1);
    assert_eq!(other[0].name, "Other Software");
}

#[tokio::test]
async fn stale_update_does_not_overwrite_seeded_data() {
    let fixture = seeded_database().await;
    let result = fixture
        .companies
        .save(
            fixture.owner,
            Some(fixture.company.company_id.clone()),
            company_input("Stale edit"),
            Some(0),
        )
        .await;
    assert!(matches!(
        result,
        Err(crate::shared::error::RepositoryError::Conflict(_))
    ));
    let unchanged = fixture
        .companies
        .list(fixture.owner, None, 10, 0)
        .await
        .unwrap();
    assert_eq!(unchanged.len(), 1);
    assert_eq!(unchanged[0].name, fixture.company.name);
    assert_eq!(unchanged[0].revision, fixture.company.revision);
}

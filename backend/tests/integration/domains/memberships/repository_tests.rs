use super::*;
use crate::{
    app::migrations::migrate,
    domains::deals::{
        model::DealFields,
        repository::{CreateDealRecord, DealRepository},
    },
};
async fn fixture() -> (PostgresClient, MembershipRepository, DealRepository) {
    let db = PostgresClient::test_database().await.unwrap();
    migrate(&db).await.unwrap();
    db.with_connection(async |connection| {
        for user_id in 1..=6 {
            connection.execute(
                "INSERT INTO users(id,username,email,first_name,last_name,password_hash) VALUES(?1,?2,?3,'Test','Member','unusable')",
                crate::pg_params![user_id, format!("user{user_id}"), format!("user{user_id}@example.test")],
            ).await?;
        }
        Ok(())
     }).await.unwrap();
    let deals = DealRepository::new(db.clone());
    for id in ["DEAL-1", "DEAL-2"] {
        let fields: DealFields = serde_json::from_value(serde_json::json!({
            "dealName": "Shared",
            "dealCode": null,
            "startDate": null,
            "expectedCloseDate": null,
            "actualCloseDate": null,
            "engagementCompletedDate": null,
            "engagementType": "buy_side",
            "transactionType": "unknown",
            "status": "pipeline",
            "parties": [],
            "scopes": []
        }))
        .unwrap();
        deals
            .create(CreateDealRecord {
                deal_id: id.into(),
                user_id: 1,
                fields,
                local_path: Some("/private/device/source".into()),
                sharepoint_link: None,
            })
            .await
            .unwrap();
    }
    (db.clone(), MembershipRepository::new(db), deals)
}
async fn add(repo: &MembershipRepository, id: i64, role: DealRole) {
    repo.add_member("DEAL-1".into(), 1, format!("user{id}@example.test"), role)
        .await
        .unwrap();
}
#[tokio::test]
async fn role_matrix_including_archived_and_local_creator_policy() {
    let (_, repo, deals) = fixture().await;
    for (id, role) in [
        (2, DealRole::Manager),
        (3, DealRole::Contributor),
        (4, DealRole::Viewer),
    ] {
        add(&repo, id, role).await;
    }
    for actor in 1..=4 {
        for (cap, allowed) in [
            (Capability::Read, true),
            (Capability::Upload, actor <= 3),
            (Capability::Edit, actor <= 2),
            (Capability::ManageTeams, actor <= 2),
            (Capability::ManageMembers, actor == 1),
            (Capability::Transfer, actor == 1),
            (Capability::Archive, actor == 1),
            (Capability::LocalSource, actor == 1),
        ] {
            assert_eq!(
                repo.access("DEAL-1".into(), actor, cap).await.is_ok(),
                allowed,
                "actor {actor}, {cap:?}"
            );
        }
    }
    assert!(matches!(
        repo.access("DEAL-1".into(), 5, Capability::Read).await,
        Err(RepositoryError::NotFound(_))
    ));
    deals.archive("DEAL-1".into(), 1).await.unwrap();
    for actor in 1..=4 {
        assert!(repo
            .access("DEAL-1".into(), actor, Capability::Read)
            .await
            .is_ok());
        assert_eq!(
            repo.access("DEAL-1".into(), actor, Capability::LocalSource)
                .await
                .is_ok(),
            actor == 1
        );
        for cap in [
            Capability::Upload,
            Capability::Edit,
            Capability::ManageTeams,
            Capability::ManageMembers,
        ] {
            assert!(matches!(
                repo.access("DEAL-1".into(), actor, cap).await,
                Err(RepositoryError::Forbidden)
            ));
        }
    }
    repo.remove_member("DEAL-1".into(), 1, 4).await.unwrap();
    repo.remove_member("DEAL-1".into(), 3, 3).await.unwrap();
    repo.transfer_ownership("DEAL-1".into(), 1, 2, 6)
        .await
        .unwrap();
    assert_eq!(
        repo.access("DEAL-1".into(), 2, Capability::Read)
            .await
            .unwrap()
            .created_by_user_id,
        1
    );
    assert!(repo
        .access("DEAL-1".into(), 2, Capability::LocalSource)
        .await
        .is_err());
}
#[tokio::test]
async fn ownership_transfer_is_atomic_and_removed_creator_has_no_access() {
    let (db, repo, deals) = fixture().await;
    add(&repo, 2, DealRole::Contributor).await;
    assert!(matches!(
        repo.remove_member("DEAL-1".into(), 1, 1).await,
        Err(RepositoryError::Forbidden)
    ));
    assert!(repo
        .change_role("DEAL-1".into(), 1, 1, DealRole::Viewer, 1)
        .await
        .is_err());
    let (a, b) = tokio::join!(
        repo.transfer_ownership("DEAL-1".into(), 1, 2, 2),
        repo.transfer_ownership("DEAL-1".into(), 1, 2, 2)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(
        deals.by_id("DEAL-1".into()).await.unwrap().unwrap().user_id,
        1
    );
    assert_eq!(
        repo.access("DEAL-1".into(), 1, Capability::Read)
            .await
            .unwrap()
            .role,
        DealRole::Manager
    );
    repo.remove_member("DEAL-1".into(), 2, 1).await.unwrap();
    assert!(repo
        .access("DEAL-1".into(), 1, Capability::Read)
        .await
        .is_err());
    assert_eq!(repo.accessible_deals(1).await.unwrap(), vec!["DEAL-2"]);
    assert_eq!(repo.accessible_deals(2).await.unwrap(), vec!["DEAL-1"]);
    let count: i64 = db
        .read_one_with_async(
            SqlBuilder::select("deal_memberships")
                .count_rows()
                .and_where(Condition::equal("deal_id", "DEAL-1"))
                .and_where(Condition::equal("role", "owner"))
                .build(),
            |r| r.get(0),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count, 1);
}
#[tokio::test]
async fn teams_only_organize_active_deal_members_and_cascade_on_leave() {
    let (db, repo, _) = fixture().await;
    add(&repo, 2, DealRole::Manager).await;
    add(&repo, 3, DealRole::Viewer).await;
    repo.create_team("DEAL-1".into(), 2, "Engineering".into())
        .await
        .unwrap();
    assert!(matches!(
        repo.create_team("DEAL-1".into(), 2, "Another team".into())
            .await,
        Err(RepositoryError::Conflict(_))
    ));
    let team = repo
        .teams("DEAL-1".into(), 3, String::new(), 30)
        .await
        .unwrap()
        .items
        .remove(0);
    for _ in 0..2 {
        repo.set_team_member("DEAL-1".into(), 2, team.team_id.clone(), 3, true)
            .await
            .unwrap();
    }
    assert!(repo
        .set_team_member("DEAL-2".into(), 1, team.team_id.clone(), 1, true)
        .await
        .is_err());
    assert!(repo
        .set_team_member("DEAL-1".into(), 2, team.team_id.clone(), 5, true)
        .await
        .is_err());
    assert!(repo
        .delete_team("DEAL-1".into(), 3, team.team_id.clone())
        .await
        .is_err());
    assert!(repo
        .rename_team(
            "DEAL-1".into(),
            2,
            team.team_id.clone(),
            "Platform".into(),
            2
        )
        .await
        .is_err());
    repo.remove_member("DEAL-1".into(), 3, 3).await.unwrap();
    assert!(repo
        .teams("DEAL-1".into(), 2, String::new(), 30)
        .await
        .unwrap()
        .items[0]
        .user_ids
        .is_empty());
    // Direct cross-deal writes must also fail at the composite foreign key.
    assert!(db
        .write_async(
            SqlBuilder::insert_into("deal_team_memberships")
                .value("deal_id", "DEAL-2")
                .value("team_id", team.team_id)
                .value("user_id", 1)
                .build()
        )
        .await
        .is_err());
}
#[tokio::test]
async fn revisions_duplicates_and_activity_are_transactional_and_paginated() {
    let (db, repo, _) = fixture().await;
    add(&repo, 2, DealRole::Viewer).await;
    assert!(matches!(
        repo.add_member(
            "DEAL-1".into(),
            1,
            "user2@example.test".into(),
            DealRole::Contributor
        )
        .await,
        Err(RepositoryError::Conflict(_))
    ));
    let (a, b) = tokio::join!(
        repo.change_role("DEAL-1".into(), 1, 2, DealRole::Contributor, 1),
        repo.change_role("DEAL-1".into(), 1, 2, DealRole::Manager, 1)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let members = repo.members("DEAL-1".into(), 1, 0, 1).await.unwrap();
    assert_eq!(members.items.len(), 1);
    assert_eq!(members.next_cursor, Some("1".into()));
    assert_eq!(
        repo.members("DEAL-1".into(), 1, 1, 1).await.unwrap().items[0].revision,
        2
    );
    let mut ids = Vec::new();
    let mut cursor = String::new();
    loop {
        let page = repo.activity("DEAL-1".into(), 1, cursor, 1).await.unwrap();
        ids.extend(page.items.into_iter().map(|e| e.event_id));
        if let Some(next) = page.next_cursor {
            cursor = next;
        } else {
            break;
        }
    }
    assert_eq!(ids.len(), 3);
    let count = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), count);
    db.with_connection(async |c| { c.execute_batch("CREATE OR REPLACE FUNCTION deny_activity_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test' USING ERRCODE='23514'; END $$; CREATE TRIGGER deny_activity BEFORE INSERT ON deal_activity_events FOR EACH ROW EXECUTE FUNCTION deny_activity_fn();").await }).await.unwrap();
    assert!(repo
        .add_member(
            "DEAL-1".into(),
            1,
            "user3@example.test".into(),
            DealRole::Viewer
        )
        .await
        .is_err());
    assert!(repo
        .access("DEAL-1".into(), 3, Capability::Read)
        .await
        .is_err());
}

#[tokio::test]
async fn concurrent_team_creation_allows_one_per_deal_and_replacement_after_deletion() {
    let (_, repo, _) = fixture().await;
    let (first, second) = tokio::join!(
        repo.create_team("DEAL-1".into(), 1, "First".into()),
        repo.create_team("DEAL-1".into(), 1, "Second".into())
    );
    assert!(matches!(
        (&first, &second),
        (Ok(()), Err(RepositoryError::Conflict(_))) | (Err(RepositoryError::Conflict(_)), Ok(()))
    ));
    let teams = repo
        .teams("DEAL-1".into(), 1, String::new(), 30)
        .await
        .unwrap();
    assert_eq!(teams.items.len(), 1);
    let activity = repo
        .activity("DEAL-1".into(), 1, String::new(), 30)
        .await
        .unwrap();
    assert_eq!(
        activity
            .items
            .iter()
            .filter(|event| event.kind == "team_created")
            .count(),
        1
    );
    repo.create_team("DEAL-2".into(), 1, "Independent".into())
        .await
        .unwrap();
    repo.delete_team("DEAL-1".into(), 1, teams.items[0].team_id.clone())
        .await
        .unwrap();
    repo.create_team("DEAL-1".into(), 1, "Replacement".into())
        .await
        .unwrap();
    assert_eq!(
        repo.teams("DEAL-1".into(), 1, String::new(), 30)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

async fn roster_snapshot(repo: &MembershipRepository) -> serde_json::Value {
    serde_json::json!({
        "access": repo.access("DEAL-1".into(), 1, Capability::Read).await.unwrap(),
        "members": repo.members("DEAL-1".into(), 1, 0, 100).await.unwrap(),
        "teams": repo.teams("DEAL-1".into(), 1, String::new(), 100).await.unwrap(),
        "activity": repo.activity("DEAL-1".into(), 1, String::new(), 100).await.unwrap(),
    })
}

#[tokio::test]
async fn service_dispatch_keeps_every_mutation_atomic_with_activity() {
    use crate::domains::memberships::{model::Mutation, service::DealAccessService};
    use crate::shared::error::ServiceError;
    let (db, repo, _) = fixture().await;
    let service = DealAccessService::new(repo.clone());
    add(&repo, 2, DealRole::Viewer).await;
    repo.create_team("DEAL-1".into(), 1, "Platform".into())
        .await
        .unwrap();
    let team_id = repo
        .teams("DEAL-1".into(), 1, String::new(), 30)
        .await
        .unwrap()
        .items
        .remove(0)
        .team_id;
    repo.set_team_member("DEAL-1".into(), 1, team_id.clone(), 2, true)
        .await
        .unwrap();
    db.with_connection(async |c| { c.execute_batch("CREATE OR REPLACE FUNCTION deny_activity_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test' USING ERRCODE='23514'; END $$; CREATE TRIGGER deny_activity BEFORE INSERT ON deal_activity_events FOR EACH ROW EXECUTE FUNCTION deny_activity_fn();").await }).await.unwrap();
    let before = roster_snapshot(&repo).await;
    for mutation in [
        Mutation::AddMember {
            email: "user3@example.test".into(),
            role: DealRole::Viewer,
        },
        Mutation::ChangeRole {
            user_id: 2,
            role: DealRole::Manager,
            revision: 1,
        },
        Mutation::RemoveMember { user_id: 2 },
        Mutation::Transfer {
            user_id: 2,
            revision: 2,
        },
        Mutation::RenameTeam {
            team_id: team_id.clone(),
            name: "Renamed".into(),
            revision: 1,
        },
        Mutation::DeleteTeam {
            team_id: team_id.clone(),
        },
        Mutation::Assign {
            team_id: team_id.clone(),
            user_id: 1,
            add: true,
        },
        Mutation::Assign {
            team_id: team_id.clone(),
            user_id: 2,
            add: false,
        },
    ] {
        assert!(matches!(
            service.mutate("DEAL-1".into(), 1, mutation).await,
            Err(ServiceError::Internal(_))
        ));
        assert_eq!(roster_snapshot(&repo).await, before);
    }
    // No-ops must succeed even when a new activity event would fail.
    for mutation in [
        Mutation::ChangeRole {
            user_id: 2,
            role: DealRole::Viewer,
            revision: 1,
        },
        Mutation::Assign {
            team_id: team_id.clone(),
            user_id: 2,
            add: true,
        },
        Mutation::Assign {
            team_id: team_id.clone(),
            user_id: 1,
            add: false,
        },
    ] {
        service.mutate("DEAL-1".into(), 1, mutation).await.unwrap();
        assert_eq!(roster_snapshot(&repo).await, before);
    }
    db.with_connection(async |c| {
        c.execute_batch("DROP TRIGGER deny_activity ON deal_activity_events;")
            .await
    })
    .await
    .unwrap();
    repo.delete_team("DEAL-1".into(), 1, team_id).await.unwrap();
    db.with_connection(async |c| { c.execute_batch("CREATE OR REPLACE FUNCTION deny_activity_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test' USING ERRCODE='23514'; END $$; CREATE TRIGGER deny_activity BEFORE INSERT ON deal_activity_events FOR EACH ROW EXECUTE FUNCTION deny_activity_fn();").await }).await.unwrap();
    let before = roster_snapshot(&repo).await;
    assert!(matches!(
        service
            .mutate(
                "DEAL-1".into(),
                1,
                Mutation::CreateTeam {
                    name: "Replacement".into()
                }
            )
            .await,
        Err(ServiceError::Internal(_))
    ));
    assert_eq!(roster_snapshot(&repo).await, before);
}

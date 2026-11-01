use super::*;
use crate::{
    app::migrations::migrate,
    domains::deals::{
        model::DealFields,
        repository::{CreateDealRecord, DealRepository},
    },
};
async fn fixture() -> (SqliteClient, MembershipRepository, DealRepository) {
    let db = SqliteClient::open_in_memory().unwrap();
    migrate(&db).unwrap();
    db.with_connection(|connection| {
        for user_id in 1..=6 {
            connection.execute(
                "INSERT INTO users(id,username,email,first_name,last_name,password_hash) VALUES(?1,?2,?3,'Test','Member','unusable')",
                rusqlite::params![user_id, format!("user{user_id}"), format!("user{user_id}@example.test")],
            )?;
        }
        Ok(())
    }).unwrap();
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
    repo.mutate(
        "DEAL-1".into(),
        1,
        Mutation::AddMember {
            username: format!("user{id}"),
            role,
        },
    )
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
    repo.mutate("DEAL-1".into(), 1, Mutation::RemoveMember { user_id: 4 })
        .await
        .unwrap();
    repo.mutate("DEAL-1".into(), 3, Mutation::RemoveMember { user_id: 3 })
        .await
        .unwrap();
    repo.mutate(
        "DEAL-1".into(),
        1,
        Mutation::Transfer {
            user_id: 2,
            revision: 6,
        },
    )
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
        repo.mutate("DEAL-1".into(), 1, Mutation::RemoveMember { user_id: 1 })
            .await,
        Err(RepositoryError::Forbidden)
    ));
    assert!(repo
        .mutate(
            "DEAL-1".into(),
            1,
            Mutation::ChangeRole {
                user_id: 1,
                role: DealRole::Viewer,
                revision: 1
            }
        )
        .await
        .is_err());
    let (a, b) = tokio::join!(
        repo.mutate(
            "DEAL-1".into(),
            1,
            Mutation::Transfer {
                user_id: 2,
                revision: 2
            }
        ),
        repo.mutate(
            "DEAL-1".into(),
            1,
            Mutation::Transfer {
                user_id: 2,
                revision: 2
            }
        )
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
    repo.mutate("DEAL-1".into(), 2, Mutation::RemoveMember { user_id: 1 })
        .await
        .unwrap();
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
    repo.mutate(
        "DEAL-1".into(),
        2,
        Mutation::CreateTeam {
            name: "Engineering".into(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        repo.mutate(
            "DEAL-1".into(),
            2,
            Mutation::CreateTeam {
                name: "Another team".into()
            }
        )
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
        repo.mutate(
            "DEAL-1".into(),
            2,
            Mutation::Assign {
                team_id: team.team_id.clone(),
                user_id: 3,
                add: true,
            },
        )
        .await
        .unwrap();
    }
    assert!(repo
        .mutate(
            "DEAL-2".into(),
            1,
            Mutation::Assign {
                team_id: team.team_id.clone(),
                user_id: 1,
                add: true
            }
        )
        .await
        .is_err());
    assert!(repo
        .mutate(
            "DEAL-1".into(),
            2,
            Mutation::Assign {
                team_id: team.team_id.clone(),
                user_id: 5,
                add: true
            }
        )
        .await
        .is_err());
    assert!(repo
        .mutate(
            "DEAL-1".into(),
            3,
            Mutation::DeleteTeam {
                team_id: team.team_id.clone()
            }
        )
        .await
        .is_err());
    assert!(repo
        .mutate(
            "DEAL-1".into(),
            2,
            Mutation::RenameTeam {
                team_id: team.team_id.clone(),
                name: "Platform".into(),
                revision: 2
            }
        )
        .await
        .is_err());
    repo.mutate("DEAL-1".into(), 3, Mutation::RemoveMember { user_id: 3 })
        .await
        .unwrap();
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
        repo.mutate(
            "DEAL-1".into(),
            1,
            Mutation::AddMember {
                username: "user2".into(),
                role: DealRole::Contributor
            }
        )
        .await,
        Err(RepositoryError::Conflict(_))
    ));
    let (a, b) = tokio::join!(
        repo.mutate(
            "DEAL-1".into(),
            1,
            Mutation::ChangeRole {
                user_id: 2,
                role: DealRole::Contributor,
                revision: 1
            }
        ),
        repo.mutate(
            "DEAL-1".into(),
            1,
            Mutation::ChangeRole {
                user_id: 2,
                role: DealRole::Manager,
                revision: 1
            }
        )
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
    db.with_connection(|c|c.execute_batch("CREATE TRIGGER deny_activity BEFORE INSERT ON deal_activity_events BEGIN SELECT RAISE(ABORT,'test'); END;")).unwrap();
    assert!(repo
        .mutate(
            "DEAL-1".into(),
            1,
            Mutation::AddMember {
                username: "user3".into(),
                role: DealRole::Viewer
            }
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
        repo.mutate(
            "DEAL-1".into(),
            1,
            Mutation::CreateTeam {
                name: "First".into()
            }
        ),
        repo.mutate(
            "DEAL-1".into(),
            1,
            Mutation::CreateTeam {
                name: "Second".into()
            }
        )
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
    repo.mutate(
        "DEAL-2".into(),
        1,
        Mutation::CreateTeam {
            name: "Independent".into(),
        },
    )
    .await
    .unwrap();
    repo.mutate(
        "DEAL-1".into(),
        1,
        Mutation::DeleteTeam {
            team_id: teams.items[0].team_id.clone(),
        },
    )
    .await
    .unwrap();
    repo.mutate(
        "DEAL-1".into(),
        1,
        Mutation::CreateTeam {
            name: "Replacement".into(),
        },
    )
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

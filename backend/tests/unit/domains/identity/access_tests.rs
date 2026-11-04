use super::*;
use crate::{app::migrations, domains::memberships::repository::MembershipRepository};

#[tokio::test]
async fn final_search_snapshot_excludes_foreign_deleted_historical_and_revoked_results() {
    let sqlite = SqliteClient::open_in_memory().unwrap();
    migrations::migrate(&sqlite).unwrap();
    sqlite.with_connection(|connection| {
        connection.execute("INSERT INTO users(id,username,email,first_name,last_name,password_hash) VALUES(1,'actor','actor@example.test','Test','Actor','unusable'),(2,'other','other@example.test','Test','Other','unusable')",[])?;
        for (deal,actor) in [("DEAL-OWN",1),("DEAL-FOREIGN",2)] {
            connection.execute("INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES(?1,?2,'Search','buy_side','unknown','pipeline','now','now')",rusqlite::params![deal,actor])?;
            connection.execute("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES(?1,?2,'owner','now','now')",rusqlite::params![deal,actor])?;
            connection.execute("INSERT INTO deal_access_revisions(deal_id) VALUES(?1)",[deal])?;
        }
        for (file,deal,current,deleted,generation) in [
            ("current","DEAL-OWN",true,false,"active"),
            ("foreign","DEAL-FOREIGN",true,false,"active"),
            ("deleted","DEAL-OWN",true,true,"active"),
            ("historical-version","DEAL-OWN",false,false,"active"),
            ("historical-generation","DEAL-OWN",true,false,"old"),
        ] {
            connection.execute("INSERT INTO quarry_files(file_id,deal_id,display_name,metadata_json,created_at,updated_at,deleted_at) VALUES(?1,?2,'synthetic.docx','{}','now','now',?3)",rusqlite::params![file,deal,deleted.then_some("now")])?;
            connection.execute("INSERT INTO quarry_file_versions(version_id,file_id,version_number,original_filename,mime_type,content_sha256,byte_size,is_current,created_at) VALUES(?1,?1,1,'synthetic.docx','application/octet-stream',?2,1,?3,'now')",rusqlite::params![file,"a".repeat(64),current])?;
            connection.execute("INSERT INTO document_identity_manifests(version_id,derivation_version,document_id,active_generation,chunk_count,graph_migrated) VALUES(?1,2,?1,'active',1,1)",[file])?;
            connection.execute("INSERT INTO document_chunk_identities(chunk_id,version_id,index_generation,chunk_index,chunk_sha256) VALUES(?1,?1,?2,1,?3)",rusqlite::params![file,generation,"b".repeat(64)])?;
        }
        Ok(())
    }).unwrap();
    let policy = Arc::new(DealAccessService::new(MembershipRepository::new(
        sqlite.clone(),
    )));
    let access = ResourceAccess::new(sqlite.clone(), policy);
    let candidates = vec![
        "current",
        "foreign",
        "deleted",
        "historical-version",
        "historical-generation",
        "unknown",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(
        access.current_result_ids(1, candidates).await.unwrap(),
        std::collections::HashSet::from(["current".to_string()])
    );
    sqlite
        .write_async(
            SqlBuilder::delete_from("deal_memberships")
                .and_where(Condition::equal("deal_id", "DEAL-OWN"))
                .and_where(Condition::equal("user_id", 1))
                .build(),
        )
        .await
        .unwrap();
    assert!(access
        .current_result_ids(1, vec!["current".into()])
        .await
        .unwrap()
        .is_empty());
    assert!(access
        .current_result_ids(1, vec![])
        .await
        .unwrap()
        .is_empty());
    assert!(access
        .current_result_ids(
            1,
            vec![
                "current".into();
                crate::domains::documents::graph::query::MAX_FILE_CHUNK_SEARCH_LIMIT + 1
            ]
        )
        .await
        .is_err());
}

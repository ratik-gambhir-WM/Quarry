use super::*;
async fn empty() -> PostgresClient {
    PostgresClient::test_database().await.unwrap()
}
#[tokio::test]
async fn fresh_schema_reuses_and_detects_constraint_drift_without_discarding_data() {
    let client = empty().await;
    migrate(&client).await.unwrap();
    migrate(&client).await.unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT tablename FROM pg_tables WHERE schemaname=current_schema() ORDER BY tablename",
    )
    .fetch_all(client.pool())
    .await
    .unwrap();
    assert_eq!(tables.len(), 42); // marker-18 inventory + lexical postings + schema metadata
    for table in [
        "quarry_files",
        "document_identity_manifests",
        "document_image_recovery",
        "template_classification_status",
        "deal_deletions",
        "notebook_save_jobs",
        "template_lexical_terms",
    ] {
        assert!(tables.iter().any(|s| s == table));
    }
    sqlx::query("INSERT INTO app_metadata VALUES ('preserved','value')")
        .execute(client.pool())
        .await
        .unwrap();
    sqlx::query("DROP INDEX idx_deal_single_owner")
        .execute(client.pool())
        .await
        .unwrap();
    assert!(matches!(
        migrate(&client).await,
        Err(MigrationError::Incompatible)
    ));
    let value: String = sqlx::query_scalar("SELECT value FROM app_metadata WHERE key='preserved'")
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert_eq!(value, "value");
}
#[tokio::test]
async fn refuses_an_unknown_nonempty_layout() {
    let client = empty().await;
    sqlx::query("CREATE TABLE unrelated (id BIGINT)")
        .execute(client.pool())
        .await
        .unwrap();
    assert!(matches!(
        migrate(&client).await,
        Err(MigrationError::Incompatible)
    ));
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('unrelated') IS NOT NULL")
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert!(exists);
}
#[tokio::test]
async fn schema_constraints_protect_json_safe_ids_and_same_notebook_folders() {
    let client = empty().await;
    migrate(&client).await.unwrap();
    assert!(sqlx::query("INSERT INTO users(id,first_name,last_name,email,username,password_hash) VALUES(9007199254740992,'a','b','a','a','x')").execute(client.pool()).await.is_err());
    crate::integration_support::seed_identity(&client, "schema@example.invalid", 'a').await;
    assert!(
        sqlx::query("INSERT INTO notebooks VALUES('n',1,NULL,'title','now','now')")
            .execute(client.pool())
            .await
            .is_ok()
    );
    assert!(sqlx::query("INSERT INTO notebook_notes(note_id,notebook_id,blocks_json,created_at,updated_at) VALUES('bad','n','{}','now','now')").execute(client.pool()).await.is_err());
    assert!(sqlx::query("INSERT INTO notebook_notes(note_id,notebook_id,folder_id,created_at,updated_at) VALUES('bad','n','missing','now','now')").execute(client.pool()).await.is_err());
    assert!(crate::domains::documents::store::identity::ready(&client)
        .await
        .unwrap());
}
#[tokio::test]
async fn concurrent_startup_serializes_schema_inspection_and_creation() {
    let client = empty().await;
    let (a, b) = tokio::join!(migrate(&client), migrate(&client));
    a.unwrap();
    b.unwrap();
}
#[tokio::test]
async fn partial_ddl_is_rolled_back() {
    let client = empty().await;
    let mut tx = client.pool().begin().await.unwrap();
    let ddl = format!("{SCHEMA}\nCREATE TABLE users(duplicate BIGINT);");
    assert!(sqlx::raw_sql(&ddl).execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
    migrate(&client).await.unwrap();
}

#[tokio::test]
async fn query_plans_use_indexes_for_scoped_access_claims_jobs_and_activity() {
    let client = empty().await;
    migrate(&client).await.unwrap();
    crate::integration_support::seed_identity(&client, "plans-a@example.invalid", 'a').await;
    crate::integration_support::seed_identity(&client, "plans-b@example.invalid", 'b').await;
    sqlx::raw_sql(
        "INSERT INTO deals(deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at)
         SELECT 'plan-'||n,1,'Plan','buy_side','unknown','pipeline','now','now' FROM generate_series(1,10000) n;
         INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at)
         SELECT deal_id,1,'owner','now','now' FROM deals;
         INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at)
         VALUES('plan-5000',2,'viewer','now','now');
         INSERT INTO quarry_files(file_id,deal_id,display_name,created_at,updated_at)
         SELECT 'file-'||n,'plan-'||n,'File','now','now' FROM generate_series(1,10000) n;
         INSERT INTO quarry_file_versions(version_id,file_id,version_number,original_filename,mime_type,content_sha256,byte_size,is_current,created_at)
         SELECT 'version-'||n,'file-'||n,1,'file.pdf','application/pdf',repeat('a',64),0,1,'now' FROM generate_series(1,10000) n;
         INSERT INTO quarry_current_content(deal_id,content_sha256,file_id,version_id)
         SELECT 'plan-'||n,repeat('a',64),'file-'||n,'version-'||n FROM generate_series(1,10000) n;
         INSERT INTO deal_activity_events(event_id,deal_id,actor_user_id,kind,created_at)
         SELECT 'event-'||n,'plan-5000',1,'deal_updated',lpad(n::text,8,'0') FROM generate_series(1,10000) n;
         INSERT INTO notebooks(notebook_id,user_id,title,created_at,updated_at) VALUES('book',1,'Book','now','now');
         INSERT INTO notebook_save_jobs(job_id,user_id,notebook_id,note_id,operation_id,request_hash,input_json,status,stage,created_at,updated_at)
         SELECT 'job-'||n,1,'book','note-'||n,'operation-'||n,repeat('b',64),'{}',
                CASE WHEN n=5000 THEN 'queued' ELSE 'completed' END,'persisting','now','now'
         FROM generate_series(1,10000) n;
         ANALYZE deals; ANALYZE deal_memberships; ANALYZE quarry_files;
         ANALYZE quarry_file_versions; ANALYZE quarry_current_content;
         ANALYZE deal_activity_events; ANALYZE notebook_save_jobs;",
    )
    .execute(client.pool())
    .await
    .unwrap();
    for (name, query, index) in [
        (
            "scoped authorization",
            "SELECT d.deal_id FROM deal_memberships m JOIN deals d USING(deal_id) WHERE m.user_id=2",
            "idx_deal_memberships_user",
        ),
        (
            "exact content claim",
            "SELECT file_id,version_id FROM quarry_current_content WHERE deal_id='plan-5000' AND content_sha256=repeat('a',64)",
            "quarry_current_content_pkey",
        ),
        (
            "activity cursor",
            "SELECT event_id FROM deal_activity_events WHERE deal_id='plan-5000' AND created_at<'00005000' ORDER BY created_at DESC,event_id DESC LIMIT 50",
            "idx_deal_activity_cursor",
        ),
        (
            "durable job claim",
            "SELECT job_id FROM notebook_save_jobs WHERE status='queued' ORDER BY created_at ASC,job_id ASC LIMIT 1",
            "idx_notebook_jobs_queue",
        ),
    ] {
        let plan: Vec<String> = sqlx::query_scalar(&format!(
            "EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT) {query}"
        ))
        .fetch_all(client.pool())
        .await
        .unwrap();
        let plan = plan.join("\n");
        println!("{name}:\n{plan}");
        assert!(plan.contains(index), "{name} did not use {index}:\n{plan}");
    }
}

#[tokio::test]
async fn previous_schema_version_is_rejected_without_resetting_records() {
    let client = empty().await;
    migrate(&client).await.unwrap();
    sqlx::query("INSERT INTO app_metadata VALUES ('preserved','value')")
        .execute(client.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE quarry_schema_version SET version=1")
        .execute(client.pool())
        .await
        .unwrap();
    assert!(matches!(
        migrate(&client).await,
        Err(MigrationError::Incompatible)
    ));
    let value: String = sqlx::query_scalar("SELECT value FROM app_metadata WHERE key='preserved'")
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert_eq!(value, "value");
}

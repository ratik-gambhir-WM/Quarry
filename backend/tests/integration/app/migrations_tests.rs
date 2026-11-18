use super::*;
async fn empty() -> PostgresClient {
    PostgresClient::test_database().await.unwrap()
}
#[tokio::test]
async fn fresh_schema_reuses_and_detects_constraint_drift_without_discarding_data() {
    let client = empty().await;
    migrate(&client, Default::default()).await.unwrap();
    migrate(&client, Default::default()).await.unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT tablename FROM pg_tables WHERE schemaname=current_schema() ORDER BY tablename",
    )
    .fetch_all(client.pool())
    .await
    .unwrap();
    assert_eq!(tables.len(), 50); // schema 6 removes app registry and template memberships
    assert!(!tables
        .iter()
        .any(|table| matches!(table.as_str(), "apps" | "app_templates")));
    assert!(sqlx::query(
        "INSERT INTO templates(template_id,template_json) VALUES('ownerless','{}')"
    )
    .execute(client.pool())
    .await
    .is_err());
    for table in [
        "quarry_files",
        "document_identity_manifests",
        "document_image_recovery",
        "template_classification_status",
        "deal_deletions",
        "notebook_save_jobs",
        "template_lexical_terms",
        "template_embedding_profiles",
        "template_embeddings",
    ] {
        assert!(tables.iter().any(|s| s == table));
    }
    let phrase_columns: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='notebook_chunks' AND column_name IN ('phrase_text','phrase_title','phrase_vector','phrase_title_vector')")
        .fetch_one(client.pool()).await.unwrap();
    assert_eq!(phrase_columns, 4);
    sqlx::query("INSERT INTO app_metadata VALUES ('preserved','value')")
        .execute(client.pool())
        .await
        .unwrap();
    sqlx::query("DROP INDEX idx_notebook_chunks_phrase")
        .execute(client.pool())
        .await
        .unwrap();
    assert!(matches!(
        migrate(&client, Default::default()).await,
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
        migrate(&client, Default::default()).await,
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
    migrate(&client, Default::default()).await.unwrap();
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
    assert!(
        crate::domains::documents::store::identity_repository::ready(&client)
            .await
            .unwrap()
    );
}
#[tokio::test]
async fn concurrent_startup_serializes_schema_inspection_and_creation() {
    let client = empty().await;
    let (a, b) = tokio::join!(
        migrate(&client, Default::default()),
        migrate(&client, Default::default())
    );
    a.unwrap();
    b.unwrap();
}
#[tokio::test]
async fn partial_ddl_is_rolled_back() {
    let client = empty().await;
    let mut tx = client.pool().begin().await.unwrap();
    let schema = SchemaDimensions::default().render().unwrap();
    let ddl = format!("{schema}\nCREATE TABLE users(duplicate BIGINT);");
    assert!(sqlx::raw_sql(&ddl).execute(&mut *tx).await.is_err());
    tx.rollback().await.unwrap();
    migrate(&client, Default::default()).await.unwrap();
}

#[tokio::test]
async fn query_plans_use_indexes_for_scoped_access_claims_jobs_and_activity() {
    let client = empty().await;
    migrate(&client, Default::default()).await.unwrap();
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
         INSERT INTO notebook_save_jobs(job_id,user_id,notebook_id,note_id,operation_id,request_hash,input_json,status,stage,created_at,updated_at,committed_revision,committed_set_id)
         SELECT 'job-'||n,1,'book','note-'||n,'operation-'||n,repeat('b',64),'{}',
                CASE WHEN n=5000 THEN 'queued' ELSE 'completed' END,'persisting','now','now',CASE WHEN n=5000 THEN NULL ELSE 1 END,CASE WHEN n=5000 THEN NULL ELSE 'fixture-set' END
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
    migrate(&client, Default::default()).await.unwrap();
    sqlx::query("INSERT INTO app_metadata VALUES ('preserved','value')")
        .execute(client.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE quarry_schema_version SET version=1")
        .execute(client.pool())
        .await
        .unwrap();
    assert!(matches!(
        migrate(&client, Default::default()).await,
        Err(MigrationError::Incompatible)
    ));
    let value: String = sqlx::query_scalar("SELECT value FROM app_metadata WHERE key='preserved'")
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert_eq!(value, "value");
}

#[tokio::test]
async fn explicit_mvp_reset_recreates_owned_schema_and_refuses_a_live_writer() {
    let client = empty().await;
    migrate(&client, Default::default()).await.unwrap();
    sqlx::query("INSERT INTO app_metadata VALUES('old','content')")
        .execute(client.pool())
        .await
        .unwrap();
    // Simulate the retired registry in a previous schema. Startup must refuse it intact;
    // the explicit reset may remove exactly these recognized legacy product objects.
    sqlx::raw_sql("CREATE TABLE apps(app_id TEXT PRIMARY KEY); CREATE TABLE app_templates(app_id TEXT REFERENCES apps(app_id), template_id TEXT REFERENCES templates(template_id)); UPDATE quarry_schema_version SET version=5;")
        .execute(client.pool()).await.unwrap();
    assert!(matches!(
        migrate(&client, Default::default()).await,
        Err(MigrationError::Incompatible)
    ));
    let writer = client.clone().with_writer_lease().await.unwrap();
    let result = reset_product_schema(&client, Default::default()).await;
    assert!(
        matches!(&result, Err(MigrationError::ResetRefused(_))),
        "unexpected reset outcome: {result:?}"
    );
    drop(writer);
    // Closing the retained writer session is asynchronous at drop; wait for release.
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            match reset_product_schema(&client, Default::default()).await {
                Ok(()) => break,
                Err(MigrationError::ResetRefused(_)) => tokio::task::yield_now().await,
                Err(error) => panic!("{error}"),
            }
        }
    })
    .await
    .unwrap();
    migrate(&client, Default::default()).await.unwrap();
    let old: i64 = sqlx::query_scalar("SELECT count(*) FROM app_metadata WHERE key='old'")
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert_eq!(old, 0);
    sqlx::query("CREATE TABLE unrelated(id BIGINT)")
        .execute(client.pool())
        .await
        .unwrap();
    assert!(matches!(
        reset_product_schema(&client, Default::default()).await,
        Err(MigrationError::ResetRefused(_))
    ));
    assert!(sqlx::query("SELECT * FROM unrelated")
        .fetch_all(client.pool())
        .await
        .is_ok());
}

async fn vector_dimensions(client: &PostgresClient, table: &str) -> String {
    sqlx::query_scalar(
        "SELECT format_type(a.atttypid,a.atttypmod) FROM pg_attribute a
         JOIN pg_class c ON c.oid=a.attrelid
         WHERE c.relnamespace=current_schema()::regnamespace AND c.relname=$1 AND a.attname='embedding'",
    )
    .bind(table)
    .fetch_one(client.pool())
    .await
    .unwrap()
}

#[tokio::test]
async fn template_vectors_are_an_independent_dimension_family() {
    let client = empty().await;
    let dimensions = SchemaDimensions {
        shared: 3,
        template: 5,
    };
    migrate(&client, dimensions).await.unwrap();
    for table in ["document_chunks", "notebook_chunks"] {
        assert!(vector_dimensions(&client, table)
            .await
            .ends_with("vector(3)"));
    }
    assert!(vector_dimensions(&client, "template_embeddings")
        .await
        .ends_with("vector(5)"));
    // A different template family is a different checksum; startup never resizes it.
    assert!(matches!(
        migrate(
            &client,
            SchemaDimensions {
                shared: 3,
                template: 3
            }
        )
        .await,
        Err(MigrationError::Incompatible)
    ));
    assert!(matches!(
        migrate(
            &client,
            SchemaDimensions {
                shared: 3,
                template: 16_001
            }
        )
        .await,
        Err(MigrationError::Incompatible)
    ));
}

#[tokio::test]
async fn deferred_template_pair_integrity_rejects_partial_or_stale_readiness() {
    let client = empty().await;
    migrate(
        &client,
        SchemaDimensions {
            shared: 1536,
            template: 2,
        },
    )
    .await
    .unwrap();
    crate::integration_support::seed_identity(&client, "template-integrity@example.invalid", 'i')
        .await;
    sqlx::raw_sql(
        "INSERT INTO templates(template_id,template_json,owner_user_id) VALUES('t','{}',1);
         INSERT INTO template_classification_status(template_id,status,input_fingerprint,schema_version,prompt_version,
             subject_embedding_document,subject_embedding_fingerprint,capability_embedding_document,capability_embedding_fingerprint,embedding_status)
         VALUES('t','ready','in',2,'p','s','sf','c','cf','pending');
         INSERT INTO template_embedding_profiles VALUES(repeat('a',64),'openai','m',2,'cosine',1);",
    )
    .execute(client.pool())
    .await
    .unwrap();
    let ready = "UPDATE template_classification_status SET embedding_status='ready',embedding_profile_id=repeat('a',64) WHERE template_id='t'";
    let row = |role: &str, fingerprint: &str| {
        format!("INSERT INTO template_embeddings VALUES('t','{role}',repeat('a',64),2,'{fingerprint}','[1,0]');")
    };
    let attempt = |sql: String| {
        let client = client.clone();
        async move {
            let mut tx = client.pool().begin().await.unwrap();
            let result = sqlx::raw_sql(&sql).execute(&mut *tx).await;
            match result {
                Ok(_) => tx.commit().await.map(|_| ()),
                Err(error) => Err(error),
            }
        }
    };
    // Readiness with no rows, one role, or a stale fingerprint fails at commit.
    assert!(attempt(ready.to_owned()).await.is_err());
    assert!(attempt(format!("{}{ready}", row("subject", "sf")))
        .await
        .is_err());
    assert!(attempt(format!(
        "{}{}{ready}",
        row("subject", "sf"),
        row("capability", "stale")
    ))
    .await
    .is_err());
    // Vectors are never retained for a non-ready stage.
    assert!(attempt(row("subject", "sf")).await.is_err());
    // The complete current pair publishes atomically.
    attempt(format!(
        "{}{}{ready}",
        row("subject", "sf"),
        row("capability", "cf")
    ))
    .await
    .unwrap();
    // Removing one role from a ready pair is rejected.
    assert!(
        attempt("DELETE FROM template_embeddings WHERE role='capability'".into())
            .await
            .is_err()
    );
    // A zero vector or the wrong dimension is rejected by the typed column.
    assert!(attempt(
        "UPDATE template_embeddings SET embedding='[0,0]' WHERE role='subject'".into()
    )
    .await
    .is_err());
    assert!(attempt(
        "UPDATE template_embeddings SET embedding='[1,0,0]' WHERE role='subject'".into()
    )
    .await
    .is_err());
    // Startup integrity validation accepts the complete pair.
    migrate(
        &client,
        SchemaDimensions {
            shared: 1536,
            template: 2,
        },
    )
    .await
    .unwrap();
}

use crate::domains::templates::slide_templates::*;
use serde_json::json;
use std::sync::Arc;
fn record(id: &str, source: TemplateSource) -> TemplateInsert {
    let document: CanvasDocument = serde_json::from_value(json!({
        "presentation": {
            "title": "Record",
            "preserveElementOrder": true,
            "showBranding": false,
            "slides": [{
                "id": "slide-1", "name": "Record", "width": 1280, "height": 720,
                "backgroundColor": "FFFFFF", "elements": []
            }]
        }
    }))
    .unwrap();
    TemplateInsert {
        template: StoredTemplate {
            revision: 1,
            template_id: id.into(),
            template_json: document,
        },
        assets: vec![],
        metadata: StoredTemplateMetadata {
            template_id: id.into(),
            title: "Record".into(),
            description: "Imported PowerPoint template".into(),
            kind: TemplateKind::Diagram,
            source,
            created_at: 0,
            catalog_version: None,
            catalog_checksum: Some("checksum".into()),
        },
        preview: None,
    }
}
#[tokio::test]
async fn unknown_legacy_schema_is_refused_without_modifying_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.sqlite");
    let c = crate::adapters::postgres::client::test_support::TestConnection::open(&path)
        .await
        .unwrap();
    c.execute_batch("CREATE TABLE keep(value TEXT); INSERT INTO keep VALUES ('retained');")
        .await
        .unwrap();
    drop(c);
    assert!(crate::slide_template_test_support::repository(&path)
        .await
        .is_err());
    let c = crate::adapters::postgres::client::test_support::TestConnection::open(&path)
        .await
        .unwrap();
    assert_eq!(
        c.query_row(
            "SELECT value FROM keep",
            Vec::<crate::adapters::postgres::query::SqlValue>::new(),
            |r| r.get::<_, String>(0)
        )
        .await
        .unwrap(),
        "retained"
    );
}
#[tokio::test]
async fn newer_schema_is_rejected_without_changing_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.sqlite");
    {
        let c = crate::adapters::postgres::client::test_support::TestConnection::open(&path)
            .await
            .unwrap();
        c.execute_batch(
            "CREATE TABLE keep_me(value TEXT); INSERT INTO keep_me VALUES ('retained');",
        )
        .await
        .unwrap();
    }
    assert!(crate::slide_template_test_support::repository(&path)
        .await
        .is_err());
    let c = crate::adapters::postgres::client::test_support::TestConnection::open(&path)
        .await
        .unwrap();
    assert_eq!(
        c.query_row(
            "SELECT value FROM keep_me",
            Vec::<crate::adapters::postgres::query::SqlValue>::new(),
            |r| r.get::<_, String>(0)
        )
        .await
        .unwrap(),
        "retained"
    );
}
#[tokio::test]
async fn builtin_deletion_is_app_scoped_and_unchanged_reseed_keeps_deletion() {
    let repo = Arc::new(
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:"))
            .await
            .unwrap(),
    );
    let builtin = record("builtin", TemplateSource::Builtin);
    repo.upsert_builtin(&builtin.template, &[], &builtin.metadata, None)
        .await
        .unwrap();
    let a = repo.for_app("AppA").await.unwrap().unwrap();
    let b = repo.for_app("AppB").await.unwrap().unwrap();
    assert!(a.delete("builtin", 1).await.unwrap());
    assert!(a.find_by_id("builtin").await.unwrap().is_none());
    assert!(b.find_by_id("builtin").await.unwrap().is_some());
    repo.upsert_builtin(&builtin.template, &[], &builtin.metadata, None)
        .await
        .unwrap();
    let a = repo.for_app("AppA").await.unwrap().unwrap();
    assert!(a.find_by_id("builtin").await.unwrap().is_none());
    let c = repo.for_app("AppC").await.unwrap().unwrap();
    assert!(c.find_by_id("builtin").await.unwrap().is_some());
    assert!(b.delete("builtin", 1).await.unwrap());
    assert!(c.delete("builtin", 1).await.unwrap());
    assert!(repo.delete("builtin", 1).await.unwrap());
    assert!(repo.find_by_id("builtin").await.unwrap().is_none());
}
#[tokio::test]
async fn cross_app_assets_updates_and_deletes_cannot_access_imports() {
    let repo = crate::slide_template_test_support::repository(std::path::Path::new(":memory:"))
        .await
        .unwrap();
    let a = repo.for_app("A").await.unwrap().unwrap();
    let b = repo.for_app("B").await.unwrap().unwrap();
    let mut record = record("private", TemplateSource::Import);
    record.assets.push(TemplateAsset {
        asset_id: "image".into(),
        template_id: "private".into(),
        content_type: "image/png".into(),
        bytes: vec![1],
    });
    a.insert_many(&[record.clone()]).await.unwrap();
    assert!(b.find_asset("private", "image").await.unwrap().is_none());
    assert!(b.find_by_id_with_assets("private").await.unwrap().is_none());
    assert!(b.update(&record.template, &[]).await.is_err());
    assert!(!b.delete("private", 1).await.unwrap());
    assert!(a.find_asset("private", "image").await.unwrap().is_some());
    assert_eq!(a.list(None).await.unwrap().len(), 1);
    assert!(b.list(None).await.unwrap().is_empty());
}

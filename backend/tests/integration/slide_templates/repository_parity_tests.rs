use crate::domains::templates::slide_templates::*;
use serde_json::json;
use std::sync::Arc;
fn record(id: &str, source: TemplateSource) -> TemplateInsert {
    let document:CanvasDocument=serde_json::from_value(json!({"presentation":{"title":"Record","preserveElementOrder":true,"showBranding":false,"slides":[{"id":"slide-1","name":"Record","width":1280,"height":720,"backgroundColor":"FFFFFF","elements":[]}]}})).unwrap();
    TemplateInsert {
        template: StoredTemplate {
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
#[test]
fn unknown_legacy_schema_is_refused_without_modifying_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.sqlite");
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("CREATE TABLE keep(value TEXT); INSERT INTO keep VALUES ('retained'); PRAGMA user_version=1;").unwrap();
    drop(c);
    assert!(crate::slide_template_test_support::repository(&path).is_err());
    let c = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        c.query_row("SELECT value FROM keep", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "retained"
    );
}
#[test]
fn newer_schema_is_rejected_without_changing_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.sqlite");
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("PRAGMA user_version=999; CREATE TABLE keep_me(value TEXT); INSERT INTO keep_me VALUES ('retained');").unwrap();
    }
    assert!(crate::slide_template_test_support::repository(&path).is_err());
    let c = rusqlite::Connection::open(&path).unwrap();
    assert_eq!(
        c.query_row("SELECT value FROM keep_me", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "retained"
    );
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))
            .unwrap(),
        999
    );
}
#[test]
fn builtin_deletion_is_app_scoped_and_unchanged_reseed_keeps_deletion() {
    let repo = Arc::new(
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap(),
    );
    let builtin = record("builtin", TemplateSource::Builtin);
    repo.upsert_builtin(&builtin.template, &[], &builtin.metadata, None)
        .unwrap();
    let a = repo.for_app("AppA").unwrap().unwrap();
    let b = repo.for_app("AppB").unwrap().unwrap();
    assert!(a.delete("builtin").unwrap());
    assert!(a.find_by_id("builtin").unwrap().is_none());
    assert!(b.find_by_id("builtin").unwrap().is_some());
    repo.upsert_builtin(&builtin.template, &[], &builtin.metadata, None)
        .unwrap();
    let a = repo.for_app("AppA").unwrap().unwrap();
    assert!(a.find_by_id("builtin").unwrap().is_none());
    let c = repo.for_app("AppC").unwrap().unwrap();
    assert!(c.find_by_id("builtin").unwrap().is_some());
    assert!(b.delete("builtin").unwrap());
    assert!(c.delete("builtin").unwrap());
    assert!(repo.delete("builtin").unwrap());
    assert!(repo.find_by_id("builtin").unwrap().is_none());
}
#[test]
fn cross_app_assets_updates_and_deletes_cannot_access_imports() {
    let repo =
        crate::slide_template_test_support::repository(std::path::Path::new(":memory:")).unwrap();
    let a = repo.for_app("A").unwrap().unwrap();
    let b = repo.for_app("B").unwrap().unwrap();
    let mut record = record("private", TemplateSource::Import);
    record.assets.push(TemplateAsset {
        asset_id: "image".into(),
        template_id: "private".into(),
        content_type: "image/png".into(),
        bytes: vec![1],
    });
    a.insert_many(&[record.clone()]).unwrap();
    assert!(b.find_asset("private", "image").unwrap().is_none());
    assert!(b.find_by_id_with_assets("private").unwrap().is_none());
    assert!(b.update(&record.template, &[]).is_err());
    assert!(!b.delete("private").unwrap());
    assert!(a.find_asset("private", "image").unwrap().is_some());
    assert_eq!(a.list(None).unwrap().len(), 1);
    assert!(b.list(None).unwrap().is_empty());
}

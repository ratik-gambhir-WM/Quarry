use crate::{
    adapters::{helix::client::HelixClient, postgres::client::PostgresClient},
    app::{bootstrap::assemble_api_with_templates, migrations},
    domains::templates::{self, slide_templates::*},
    integration_support, slide_template_test_support as support, AppConfig,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

const PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

async fn application() -> (Router, PostgresClient, Arc<SlideTemplates>) {
    let postgres = PostgresClient::test_database().await.unwrap();
    migrations::migrate(&postgres).await.unwrap();
    integration_support::seed_identity(&postgres, "first@example.com", 'a').await;
    integration_support::seed_identity(&postgres, "second@example.com", 'b').await;
    let repository = Arc::new(PostgresTemplateRepository::new(postgres.clone()));
    repository
        .ensure_app("Quarry_WestMonroe", None)
        .await
        .unwrap();
    let classification = Arc::new(SlideClassificationService::new(
        SlideClassificationServiceOptions {
            classifier: Arc::new(support::FixtureClassifier),
            embedder: Arc::new(support::FixtureEmbedder(false)),
            embedding_model: "fixture".into(),
            classification_model: "fixture".into(),
            repository: repository.clone(),
            input_limits: SlideClassificationInputLimits::default(),
            embedding_dimensions: Some(2),
            embedding_max_text_bytes: 10000,
        },
    ));
    let studio = Arc::new(SlideTemplates::new(
        repository,
        Arc::new(support::FixturePreview),
        Some(classification),
        None,
        false,
    ));
    let config = AppConfig::default();
    let api = assemble_api_with_templates(
        &config,
        postgres.clone(),
        Arc::new(HelixClient::from_config(&config.helix).unwrap()),
        reqwest::Client::new(),
        integration_support::prompt_catalog(),
        Arc::new(templates::service::TemplateService::embedded(
            studio.clone(),
        )),
        Some(studio.clone()),
    );
    (crate::create_router(api, &config.http), postgres, studio)
}
fn document() -> CanvasDocument {
    serde_json::from_value(json!({"presentation":{"title":"Private template","preserveElementOrder":true,"showBranding":false,"slides":[{"id":"slide-1","name":"Private","width":1280,"height":720,"backgroundColor":"FFFFFF","elements":[{"id":"image-1","type":"image","x":0,"y":0,"w":10,"h":10,"src":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=","fit":"contain"}]}]}})).unwrap()
}
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    user: Option<i64>,
    content_type: &str,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", content_type);
    if let Some(id) = user {
        request = request
            .header(
                "authorization",
                format!(
                    "Bearer {}",
                    if id == 1 { 'a' } else { 'b' }.to_string().repeat(43)
                ),
            )
            .header("x-user-id", id);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    (
        response.status(),
        to_bytes(response.into_body(), 64 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
}
async fn json_call(app: &Router, method: &str, path: &str, user: i64) -> (StatusCode, Value) {
    let body = if method == "POST" {
        b"{}".to_vec()
    } else {
        vec![]
    };
    let (status, bytes) = call(app, method, path, Some(user), "application/json", body).await;
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn user_private_templates_reject_cross_account_documents_previews_assets_retry_delete_and_export(
) {
    let (app, postgres, studio) = application().await;
    for prefix in ["/api/v1", "/api"] {
        let (_, empty) = json_call(
            &app,
            "GET",
            &format!("{prefix}/templates/previews?page=1"),
            2,
        )
        .await;
        assert_eq!(empty["pagination"]["totalItems"], 0);
    }
    let bytes = pptx::write(&document()).unwrap();
    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/templates/slide-templates/import",
        Some(1),
        PPTX,
        bytes.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let repository = PostgresTemplateRepository::new(postgres.clone());
    let id = repository.list(None).await.unwrap()[0]
        .template
        .template_id
        .clone();
    let record = repository
        .find_by_id_with_assets(&id)
        .await
        .unwrap()
        .unwrap();
    let asset = &record.assets[0].asset_id;
    for prefix in ["/api/v1", "/api"] {
        let (_, page) = json_call(
            &app,
            "GET",
            &format!("{prefix}/templates/previews?page=1"),
            1,
        )
        .await;
        assert_eq!(page["pagination"]["totalItems"], 1);
        let (_, empty) = json_call(
            &app,
            "GET",
            &format!("{prefix}/templates/previews?page=1"),
            2,
        )
        .await;
        assert_eq!(empty["pagination"]["totalItems"], 0);
        let (_, catalog) = json_call(
            &app,
            "GET",
            &format!("{prefix}/templates/slide-templates/catalog"),
            2,
        )
        .await;
        assert_eq!(catalog["templates"].as_array().unwrap().len(), 0);
        for suffix in [
            format!("/templates/{id}"),
            format!("/templates/slide-templates/catalog/{id}"),
            format!("/templates/slide-templates/import/{id}"),
            format!("/templates/slide-templates/catalog/{id}/preview"),
            format!("/templates/slide-templates/catalog/{id}/retrieval?metadata=true"),
            format!("/templates/slide-templates/import/{id}/assets/{asset}"),
        ] {
            let path = format!("{prefix}{suffix}");
            assert_eq!(
                call(&app, "GET", &path, Some(1), "application/json", vec![])
                    .await
                    .0,
                StatusCode::OK,
                "{path}"
            );
            assert_eq!(
                call(&app, "GET", &path, Some(2), "application/json", vec![])
                    .await
                    .0,
                StatusCode::NOT_FOUND,
                "{path}"
            );
            assert_eq!(
                call(&app, "GET", &path, None, "application/json", vec![])
                    .await
                    .0,
                StatusCode::UNAUTHORIZED,
                "{path}"
            );
        }
        for suffix in [
            format!("/templates/{id}"),
            format!("/templates/slide-templates/catalog/{id}"),
        ] {
            assert_eq!(
                json_call(&app, "DELETE", &format!("{prefix}{suffix}"), 2)
                    .await
                    .0,
                StatusCode::NOT_FOUND
            );
        }
        assert_eq!(
            json_call(
                &app,
                "POST",
                &format!("{prefix}/templates/slide-templates/catalog/{id}/retrieval/retry"),
                2
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        for suffix in ["/templates/export", "/templates/slide-templates/export"] {
            let payload = serde_json::to_vec(&record.template_json).unwrap();
            assert_eq!(
                call(
                    &app,
                    "POST",
                    &format!("{prefix}{suffix}"),
                    Some(2),
                    "application/json",
                    payload.clone()
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
            assert_eq!(
                call(
                    &app,
                    "POST",
                    &format!("{prefix}{suffix}"),
                    Some(1),
                    "application/json",
                    payload
                )
                .await
                .0,
                StatusCode::OK
            );
        }
    }
    let spoof = app
        .clone()
        .oneshot(
            Request::get("/api/v1/templates/previews?page=1")
                .header("authorization", format!("Bearer {}", "b".repeat(43)))
                .header("x-user-id", "1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spoof.status(), StatusCode::FORBIDDEN);
    let (status, _) = call(
        &app,
        "POST",
        "/api/templates/slide-templates/import",
        Some(2),
        PPTX,
        bytes,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let other = repository.for_user(2).unwrap().list(None).await.unwrap()[0]
        .template
        .template_id
        .clone();
    assert_eq!(
        json_call(&app, "DELETE", &format!("/api/v1/templates/{id}"), 1)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert!(repository.find_by_id(&other).await.unwrap().is_some());
    studio.shutdown();
    studio.stop_worker().await;
}

#[tokio::test]
async fn all_import_modes_persist_authenticated_ownership_and_resume_shared_worker_jobs() {
    let (app, postgres, studio) = application().await;
    let bytes = pptx::write(&document()).unwrap();
    for (index, path) in [
        "/api/v1/templates/slide-templates/import",
        "/api/templates/slide-templates/batch-import",
        "/api/v1/templates/slide-templates/import/classified",
        "/api/templates/slide-templates/batch-import/classified",
    ]
    .iter()
    .enumerate()
    {
        let user = if index % 2 == 0 { 1 } else { 2 };
        assert_eq!(
            call(&app, "POST", path, Some(user), PPTX, bytes.clone())
                .await
                .0,
            StatusCode::CREATED,
            "{path}"
        );
    }
    for (user, mode) in [(1, "single"), (2, "batch")] {
        let boundary = "ownership-boundary";
        let mut body=format!("--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"private.pptx\"\r\nContent-Type: {PPTX}\r\n\r\n").into_bytes();
        body.extend(&bytes);
        body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        assert_eq!(
            call(
                &app,
                "POST",
                &format!("/api/v1/templates/import?mode={mode}"),
                Some(user),
                &format!("multipart/form-data; boundary={boundary}"),
                body
            )
            .await
            .0,
            StatusCode::CREATED
        );
    }
    let owners = postgres.with_connection(async |c| { c.prepare("SELECT owner_user_id,count(*) FROM templates GROUP BY owner_user_id ORDER BY owner_user_id").await?.query_map(Vec::<crate::adapters::postgres::query::SqlValue>::new(),|r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).await?.collect::<crate::adapters::postgres::client::test_support::TestResult<Vec<_>>>() }).await.unwrap();
    assert_eq!(owners, [(1, 3), (2, 3)]);
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let ready = postgres
            .with_connection(async |c| {
                c.query_row(
                    "SELECT count(*) FROM template_classification_status WHERE status='ready'",
                    Vec::<crate::adapters::postgres::query::SqlValue>::new(),
                    |r| r.get::<_, i64>(0),
                )
                .await
            })
            .await
            .unwrap();
        if ready == 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "owned classification jobs must complete"
        );
        tokio::task::yield_now().await;
    }
    for user in [1, 2] {
        let (_, page) = json_call(&app, "GET", "/api/v1/templates/previews?page=1", user).await;
        assert_eq!(page["pagination"]["totalItems"], 3);
    }
    let base = PostgresTemplateRepository::new(postgres.clone());
    for user in [1, 2] {
        let scoped = base.for_user(user).unwrap();
        let records = scoped
            .list_retrieval_classifications("Quarry_WestMonroe")
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        let matches = scoped
            .search_slide_classifications("Quarry_WestMonroe", "cloud", 10)
            .await
            .unwrap();
        assert_eq!(matches, [records[0].template_id.clone()]);
        let owner = postgres
            .with_connection(async |c| {
                c.query_row(
                    "SELECT owner_user_id FROM templates WHERE template_id=?1",
                    [&records[0].template_id],
                    |r| r.get::<_, i64>(0),
                )
                .await
            })
            .await
            .unwrap();
        assert_eq!(owner, user);
    }
    studio.shutdown();
    studio.stop_worker().await;
}

#[tokio::test]
async fn legacy_template_routes_fail_closed_without_relaying_a_shared_catalog() {
    let client = crate::adapters::diligence_studio::client::DiligenceStudioClient::new(
        reqwest::Client::new(),
        reqwest::Url::parse("http://127.0.0.1:1/api/v1/").unwrap(),
    );
    let api = templates::route::routes(Arc::new(templates::service::TemplateService::new(Some(
        Arc::new(client),
    ))))
    .layer(axum::Extension(integration_support::test_actor()));
    let app = crate::create_router(api, &AppConfig::default().http);
    for prefix in ["/api/v1", "/api"] {
        for (method, path) in [
            ("GET", "/templates/previews?page=1"),
            ("GET", "/templates/shared"),
            ("DELETE", "/templates/shared"),
            ("POST", "/templates/import?mode=single"),
            ("POST", "/templates/export"),
        ] {
            let (status, body) = json_call(&app, method, &format!("{prefix}{path}"), 1).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert!(body
                .to_string()
                .contains("user-private templates require the embedded template backend"));
        }
    }
}

#[tokio::test]
async fn user_scoped_repository_hides_unassigned_records_and_rejects_owner_changes() {
    let postgres = PostgresClient::test_database().await.unwrap();
    migrations::migrate(&postgres).await.unwrap();
    integration_support::seed_identity(&postgres, "first@example.com", 'a').await;
    integration_support::seed_identity(&postgres, "second@example.com", 'b').await;
    let base = PostgresTemplateRepository::new(postgres);
    base.ensure_app("Quarry_WestMonroe", None).await.unwrap();
    // Old shared imports remain canonical, with no inferred owner.
    let legacy = ImportService::new(Arc::new(base.clone()))
        .import(&pptx::write(&document()).unwrap(), TemplateKind::Diagram)
        .await
        .unwrap();
    for user in [1, 2] {
        let scoped = base.for_user(user).unwrap();
        assert!(scoped.list(None).await.unwrap().is_empty());
        assert!(scoped
            .find_by_id(&legacy.template_id)
            .await
            .unwrap()
            .is_none());
        assert!(!scoped.delete(&legacy.template_id).await.unwrap());
    }
    let owner = base.for_user(1).unwrap();
    assert!(owner.for_user(2).is_err());
    let imported = ImportService::new(owner.clone())
        .import(&pptx::write(&document()).unwrap(), TemplateKind::Diagram)
        .await
        .unwrap();
    let template = owner
        .find_by_id(&imported.template_id)
        .await
        .unwrap()
        .unwrap();
    let other = base.for_user(2).unwrap();
    assert!(other.update(&template, &[]).await.is_err());
    assert!(other
        .find_metadata(&imported.template_id)
        .await
        .unwrap()
        .is_none());
    assert!(!other.delete(&imported.template_id).await.unwrap());
    assert!(owner
        .find_by_id(&imported.template_id)
        .await
        .unwrap()
        .is_some());
    assert!(base
        .find_by_id(&legacy.template_id)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn foreign_template_deletion_never_touches_its_graph_projection() {
    let (_, postgres, application_studio) = application().await;
    let repository = Arc::new(PostgresTemplateRepository::new(postgres));
    let owner = repository.for_user(1).unwrap();
    let imported = ImportService::new(owner.clone())
        .import(&pptx::write(&document()).unwrap(), TemplateKind::Diagram)
        .await
        .unwrap();
    // Any attempted graph deletion would fail against this unavailable local transport.
    let graph = Arc::new(graph::TemplateGraph::new(
        Arc::new(HelixClient::with_config("http://127.0.0.1:1", None).unwrap()),
        std::num::NonZeroUsize::new(2).unwrap(),
    ));
    let studio = SlideTemplates::new(
        repository,
        Arc::new(DisabledTemplatePreviewGenerator),
        None,
        Some(graph),
        false,
    );
    assert!(!studio
        .for_user(2)
        .unwrap()
        .delete(imported.template_id.clone())
        .await
        .unwrap());
    assert!(owner
        .find_by_id(&imported.template_id)
        .await
        .unwrap()
        .is_some());
    application_studio.shutdown();
    application_studio.stop_worker().await;
}

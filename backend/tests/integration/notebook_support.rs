use super::{
    graph::SavePipeline, model::*, repository::NotebookRepository, service::NotebookService,
};
use crate::{
    adapters::postgres::client::PostgresClient,
    app::{
        config::AppConfig,
        http::{
            auth::{authenticate, AuthBoundary},
            create_router,
        },
        migrations,
    },
    domains::{
        identity::{
            self, access::ResourceAccess, repository::IdentityRepository, service::IdentityService,
        },
        users::repository::UserRepository,
    },
};
use axum::{routing::get, Router};
use futures_util::future::BoxFuture;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

#[derive(Default)]
pub(crate) struct FakePipeline {
    pub fail: AtomicBool,
    pub blocked: AtomicBool,
    pub embeds: AtomicUsize,
    pub gate: tokio::sync::Notify,
    pub published: Mutex<HashSet<(String, i64)>>,
}
impl SavePipeline for FakePipeline {
    fn fingerprint(&self) -> String {
        PIPELINE.into()
    }
    fn published<'a>(
        &'a self,
        _owner: i64,
        note: &'a Note,
        _hash: &'a str,
    ) -> BoxFuture<'a, Result<bool, &'static str>> {
        Box::pin(async move {
            Ok(self
                .published
                .lock()
                .unwrap()
                .contains(&(note.summary.note_id.clone(), note.summary.revision)))
        })
    }
    fn embed<'a>(
        &'a self,
        chunks: &'a [String],
    ) -> BoxFuture<'a, Result<Vec<Vec<f32>>, &'static str>> {
        Box::pin(async move {
            if !chunks.is_empty() {
                self.embeds.fetch_add(1, Ordering::SeqCst);
            }
            if self.blocked.load(Ordering::SeqCst) {
                self.gate.notified().await;
            }
            if self.fail.load(Ordering::SeqCst) {
                return Err("embedding_failed");
            }
            Ok(chunks.iter().map(|_| vec![0.5, 0.5]).collect())
        })
    }
    fn publish<'a>(
        &'a self,
        _owner: i64,
        note: &'a Note,
        _hash: &'a str,
        chunks: &'a [String],
        vectors: Vec<Vec<f32>>,
    ) -> BoxFuture<'a, Result<(), &'static str>> {
        Box::pin(async move {
            assert_eq!(chunks.len(), vectors.len());
            self.published
                .lock()
                .unwrap()
                .insert((note.summary.note_id.clone(), note.summary.revision));
            Ok(())
        })
    }
    fn delete(&self, _owner: i64, note: String) -> BoxFuture<'_, Result<(), &'static str>> {
        Box::pin(async move {
            self.published.lock().unwrap().retain(|(id, _)| *id != note);
            Ok(())
        })
    }
}
pub(crate) async fn database() -> PostgresClient {
    let postgres = PostgresClient::test_database().await.unwrap();
    migrations::migrate(&postgres).await.unwrap();
    crate::integration_support::seed_identity(&postgres, "one@example.invalid", 'a').await;
    crate::integration_support::seed_identity(&postgres, "two@example.invalid", 'b').await;
    postgres
}
pub(super) fn input(title: &str) -> SaveInput {
    serde_json::from_value(serde_json::json!({"operationId":uuid::Uuid::new_v4().to_string(),"title":title,"blocks":[],"contentSchemaVersion":1})).unwrap()
}
pub(crate) fn blocks() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../unit/domains/notebooks/fixtures/native-v1.json"
    ))
    .unwrap()
}
pub(crate) struct App {
    pub router: Router,
    pub postgres: PostgresClient,
    pub pipeline: Arc<FakePipeline>,
    pub service: Arc<NotebookService>,
}
pub(crate) async fn application() -> App {
    let postgres = database().await;
    let pipeline = Arc::new(FakePipeline::default());
    let service =
        NotebookService::with_pipeline(NotebookRepository::new(postgres.clone()), pipeline.clone());
    let identity = Arc::new(IdentityService::new(IdentityRepository::new(
        postgres.clone(),
        UserRepository::new(postgres.clone()),
    )));
    let protected = super::route::routes(service.clone())
        .merge(identity::route::protected(identity.clone()))
        .route(
            "/deals",
            get(|| async { axum::Json(serde_json::json!([])) }),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            AuthBoundary {
                identity: identity.clone(),
                access: ResourceAccess::new(
                    postgres.clone(),
                    std::sync::Arc::new(
                        crate::domains::memberships::service::DealAccessService::new(
                            crate::domains::memberships::repository::MembershipRepository::new(
                                postgres.clone(),
                            ),
                        ),
                    ),
                ),
            },
            authenticate,
        ));
    let api = Router::new()
        .merge(protected)
        .merge(identity::route::public(identity))
        .merge(crate::domains::system::route::routes());
    App {
        router: create_router(api, &AppConfig::default().http),
        postgres,
        pipeline,
        service,
    }
}
pub(crate) async fn terminal(service: &NotebookService, notebook: &str, job: &str) -> SaveJob {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let job = service
                .job(1, notebook.to_owned(), job.to_owned())
                .await
                .unwrap();
            if job.terminal() {
                return job;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

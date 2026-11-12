use std::sync::Arc;

use axum::Router;

use crate::{
    adapters::{helix::client::HelixClient, postgres::client::PostgresClient},
    app::{
        bootstrap::{test_support::assemble_api, BootstrapError},
        config::AppConfig,
        http::create_router,
        migrations,
    },
    shared::prompts::PromptCatalog,
};

pub(crate) struct TestApplication {
    pub router: Router,
    pub postgres: PostgresClient,
}

pub(crate) fn prompt_catalog() -> Arc<PromptCatalog> {
    Arc::new(PromptCatalog::load(std::path::Path::new("prompts")).unwrap())
}

pub(crate) async fn test_application() -> Result<TestApplication, BootstrapError> {
    let config = AppConfig::default();
    let postgres = PostgresClient::test_database()
        .await
        .map_err(BootstrapError::Postgres)?;
    migrations::migrate(&postgres)
        .await
        .map_err(BootstrapError::Migration)?;
    let helix =
        Arc::new(HelixClient::from_config(&config.helix).map_err(BootstrapError::HelixClient)?);
    let api = assemble_api(
        &config,
        postgres.clone(),
        helix,
        reqwest::Client::new(),
        prompt_catalog(),
    );
    let router = create_router(api, &config.http);
    Ok(TestApplication { router, postgres })
}

/// Legacy route suites still exercise real authentication with disposable fixture credentials.
pub(crate) async fn authenticated_test_application() -> Result<TestApplication, BootstrapError> {
    let mut app = test_application().await?;
    seed_identity(&app.postgres, "analyst@example.com", 'a').await;
    seed_identity(&app.postgres, "someone@example.com", 'b').await;
    // Existing upload boundary tests now require a real owned deal before parsing a body.
    app.postgres.with_connection(async |c| {
        for id in ["DEAL-LARGE", "DEAL-1", "DEAL-TEST", "DEAL-IMAGES", "DEAL-OFFICE", "DEAL-PATH"] {
            c.execute("INSERT INTO deals (deal_id,user_id,deal_name,engagement_type,transaction_type,status,created_at,updated_at) VALUES (?1,1,'Test','buy_side','unknown','pipeline',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)", [id]).await?;
            c.execute("INSERT INTO deal_memberships(deal_id,user_id,role,joined_at,updated_at) VALUES(?1,1,'owner','now','now')",[id]).await?;
            c.execute("INSERT INTO deal_access_revisions(deal_id) VALUES(?1)",[id]).await?;
        }
        Ok(())
     }).await.unwrap();
    app.router = with_test_credentials(app.router).await;
    Ok(app)
}
pub(crate) async fn seed_identity(postgres: &PostgresClient, email: &str, token_character: char) {
    use sha2::{Digest, Sha256};
    postgres.with_connection(async |c| {
        let id:i64 = c.query_row("INSERT INTO users (username,email,first_name,last_name,password_hash) VALUES (?1,?1,'Test','User','unusable-test-hash') RETURNING id", [email], |r|r.get(0)).await?;
        let hash = format!("{:x}", Sha256::digest(token_character.to_string().repeat(43).as_bytes()));
        c.execute("INSERT INTO auth_sessions (token_hash,user_id,created_at,expires_at) VALUES (?1,?2,0,4102444800)", crate::pg_params![hash,id]).await?;
        Ok(())
     }).await.unwrap();
}
pub(crate) async fn with_test_credentials(router: Router) -> Router {
    router.layer(axum::middleware::from_fn(
        |mut request: axum::extract::Request, next: axum::middleware::Next| async move {
            if !request.headers().contains_key("authorization") {
                request.headers_mut().insert(
                    "authorization",
                    format!("Bearer {}", "a".repeat(43)).parse().unwrap(),
                );
                request
                    .headers_mut()
                    .insert("x-user-id", "1".parse().unwrap());
            }
            next.run(request).await
        },
    ))
}

pub(crate) fn test_actor() -> crate::domains::identity::model::AuthenticatedUser {
    crate::domains::identity::model::AuthenticatedUser {
        user: crate::domains::users::User {
            id: 1,
            username: "analyst".into(),
            email: "analyst@example.com".into(),
            first_name: "Test".into(),
            last_name: "User".into(),
            job_title: None,
            created_at: String::new(),
            updated_at: String::new(),
        },
        expires_at: 4102444800,
        token_hash: String::new(),
    }
}
pub(crate) fn config<I, K, V>(values: I) -> Result<AppConfig, String>
where
    I: IntoIterator<Item = (K, V)>,
    K: Into<String>,
    V: Into<String>,
{
    AppConfig::from_values(
        std::iter::once((
            "QUARRY_DATABASE_URL".to_owned(),
            "postgresql://localhost/Quarry".to_owned(),
        ))
        .chain(values.into_iter().map(|(k, v)| (k.into(), v.into()))),
    )
}

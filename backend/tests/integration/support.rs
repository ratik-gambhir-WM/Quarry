use std::sync::Arc;

use axum::Router;

use crate::{
    adapters::{helix::client::HelixClient, sqlite::client::SqliteClient},
    app::{
        bootstrap::{assemble_api, BootstrapError},
        config::AppConfig,
        http::create_router,
        migrations,
    },
};

pub(crate) struct TestApplication {
    pub router: Router,
    pub sqlite: SqliteClient,
}

pub(crate) fn test_application() -> Result<TestApplication, BootstrapError> {
    let config = AppConfig::default();
    let sqlite = SqliteClient::open_in_memory().map_err(BootstrapError::SqliteOpen)?;
    migrations::migrate(&sqlite).map_err(BootstrapError::Migration)?;
    let helix =
        Arc::new(HelixClient::from_config(&config.helix).map_err(BootstrapError::HelixClient)?);
    let api = assemble_api(&config, sqlite.clone(), helix, reqwest::Client::new());
    let router = create_router(api, &config.http);
    Ok(TestApplication { router, sqlite })
}

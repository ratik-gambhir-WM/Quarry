//! Real isolated PostgreSQL test namespaces. Never resets an existing database.
use super::*;
use crate::adapters::postgres::query::SqlValue;
use crate::app::config::{PostgresConfig, SecretString};
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock, Mutex as StdMutex,
    },
    time::Duration,
};
static PATHS: LazyLock<StdMutex<HashMap<std::path::PathBuf, Arc<TestNamespace>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

static CLEANUP_FAILED: AtomicBool = AtomicBool::new(false);

pub(crate) struct TestNamespace {
    name: String,
    config: PostgresConfig,
}
impl Drop for TestNamespace {
    fn drop(&mut self) {
        let name = self.name.clone();
        let config = self.config.clone();
        // The test may have dropped its runtime already. Cleanup owns a separate runtime
        // and can only drop the namespace this factory generated.
        let result = std::thread::Builder::new().spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| ())?;
            runtime.block_on(async {
                let client = PostgresClient::connect(&config).await.map_err(|_| ())?;
                let result = sqlx::query(&format!("DROP SCHEMA IF EXISTS \"{name}\" CASCADE"))
                    .execute(client.pool())
                    .await
                    .map_err(|_| ());
                client.close().await;
                result.map(|_| ())
            })
        });
        if !matches!(result.map(|thread| thread.join()), Ok(Ok(Ok(())))) {
            CLEANUP_FAILED.store(true, Ordering::Relaxed);
            eprintln!("test PostgreSQL namespace cleanup failed: {}", self.name);
        }
    }
}
fn config() -> PostgresConfig {
    let url = std::env::var("QUARRY_TEST_DATABASE_URL").expect(
        "QUARRY_TEST_DATABASE_URL must select a disposable PostgreSQL database ending in _test",
    );
    let parsed = url::Url::parse(&url).expect("valid test PostgreSQL URL");
    assert!(
        parsed.path().ends_with("_test") && matches!(parsed.scheme(), "postgres" | "postgresql"),
        "test database must end in _test"
    );
    PostgresConfig {
        url: SecretString::new(url),
        max_connections: 4,
        acquire_timeout: Duration::from_secs(5),
        statement_timeout: Duration::from_secs(15),
    }
}
impl PostgresClient {
    pub(crate) async fn test_database() -> Result<Self, PostgresError> {
        Self::test_database_at(Path::new(&format!(
            "quarry-anonymous-{}",
            uuid::Uuid::new_v4()
        )))
        .await
    }
    pub(crate) async fn test_database_at(path: impl AsRef<Path>) -> Result<Self, PostgresError> {
        static REGISTER: std::sync::Once = std::sync::Once::new();
        REGISTER.call_once(|| {
            // SAFETY: cleanup has the required C ABI, captures no references, and only
            // releases the registry of namespaces created by this process in Quarry_test.
            assert_eq!(unsafe { libc::atexit(cleanup) }, 0);
        });
        let config = config();
        let namespace = {
            let mut paths = PATHS.lock().unwrap();
            paths
                .entry(path.as_ref().to_owned())
                .or_insert_with(|| {
                    Arc::new(TestNamespace {
                        name: format!("quarry_test_{}", uuid::Uuid::new_v4().simple()),
                        config: config.clone(),
                    })
                })
                .clone()
        };
        let admin = Self::connect(&config).await?;
        sqlx::query(&format!(
            "CREATE SCHEMA IF NOT EXISTS \"{}\"",
            namespace.name
        ))
        .execute(admin.pool())
        .await?;
        let mut scoped = config.clone();
        let mut url = url::Url::parse(config.url.expose()).unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-c search_path={}", namespace.name));
        scoped.url = SecretString::new(url.as_str());
        let client = Self::connect(&scoped).await?;
        admin.close().await;
        Ok(client)
    }
    pub(crate) async fn with_connection<T, F, Fut>(&self, operation: F) -> Result<T, PostgresError>
    where
        F: FnOnce(TestConnection) -> Fut,
        Fut: Future<Output = Result<T, PostgresError>>,
    {
        operation(TestConnection(self.clone())).await
    }
}
pub(crate) struct TestConnection(PostgresClient);
fn sql(sql: &str) -> String {
    // PostgreSQL's numbered placeholders are translated only in retained test fixtures.
    static PARAM: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\?(\d+)").unwrap());
    let sql = PARAM.replace_all(sql, "$$$1");
    sql.replace(
        "CURRENT_TIMESTAMP",
        "to_char(clock_timestamp() AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS')",
    )
}
fn query(
    sql_text: &str,
    params: Vec<SqlValue>,
    kind: QueryKind,
) -> Result<SqlQuery, PostgresError> {
    Ok(SqlQuery::new(sql(sql_text), params, kind))
}
impl TestConnection {
    pub(crate) async fn execute(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = impl Into<SqlValue>>,
    ) -> Result<usize, PostgresError> {
        let q = query(
            sql,
            params.into_iter().map(Into::into).collect(),
            QueryKind::Update,
        )?;
        Ok(bind(&q)
            .execute(self.0.pool())
            .await?
            .rows_affected()
            .try_into()
            .unwrap())
    }
    pub(crate) async fn query_row<T>(
        &self,
        sql: &str,
        params: impl IntoIterator<Item = impl Into<SqlValue>>,
        mapper: impl FnOnce(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<T, PostgresError> {
        let q = query(
            sql,
            params.into_iter().map(Into::into).collect(),
            QueryKind::Select,
        )?;
        self.0
            .read_one_with_async(Ok(q), mapper)
            .await?
            .ok_or_else(|| PostgresError::Database(sqlx::Error::RowNotFound))
    }
    pub(crate) async fn execute_batch(&self, sql_text: &str) -> Result<(), PostgresError> {
        sqlx::raw_sql(&sql(sql_text)).execute(self.0.pool()).await?;
        Ok(())
    }
}
pub(crate) type TestResult<T> = Result<T, PostgresError>;
#[macro_export]
macro_rules! pg_params {
    ($($value:expr),* $(,)?) => {
        vec![$($crate::adapters::postgres::query::SqlValue::from($value)),*]
    };
}
pub(crate) struct TestStatement {
    client: PostgresClient,
    sql: String,
}
impl TestConnection {
    pub(crate) async fn open(path: impl AsRef<Path>) -> Result<Self, PostgresError> {
        Ok(Self(PostgresClient::test_database_at(path.as_ref()).await?))
    }
    pub(crate) async fn prepare(&self, sql: &str) -> Result<TestStatement, PostgresError> {
        Ok(TestStatement {
            client: self.0.clone(),
            sql: sql.to_owned(),
        })
    }
}
impl TestStatement {
    pub(crate) async fn query_map<T>(
        &mut self,
        params: impl IntoIterator<Item = impl Into<SqlValue>>,
        mut mapper: impl FnMut(&Row<'_>) -> Result<T, PostgresError>,
    ) -> Result<std::vec::IntoIter<Result<T, PostgresError>>, PostgresError> {
        let q = query(
            &self.sql,
            params.into_iter().map(Into::into).collect(),
            QueryKind::Select,
        )?;
        let rows = bind(&q).fetch_all(self.client.pool()).await?;
        Ok(rows
            .iter()
            .map(|r| mapper(&Row(r)))
            .collect::<Vec<_>>()
            .into_iter())
    }
}
pub(crate) trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>, PostgresError>;
}
impl<T> OptionalExtension<T> for Result<T, PostgresError> {
    fn optional(self) -> Result<Option<T>, PostgresError> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(PostgresError::Database(sqlx::Error::RowNotFound)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
pub(crate) struct TestRows {
    rows: Vec<sqlx::postgres::PgRow>,
    index: usize,
}
impl TestRows {
    pub(crate) fn next(&mut self) -> Result<Option<Row<'_>>, PostgresError> {
        let row = self.rows.get(self.index);
        self.index += 1;
        Ok(row.map(Row))
    }
}
impl TestStatement {
    pub(crate) async fn query(
        &mut self,
        params: impl IntoIterator<Item = impl Into<SqlValue>>,
    ) -> Result<TestRows, PostgresError> {
        let q = query(
            &self.sql,
            params.into_iter().map(Into::into).collect(),
            QueryKind::Select,
        )?;
        Ok(TestRows {
            rows: bind(&q).fetch_all(self.client.pool()).await?,
            index: 0,
        })
    }
}

extern "C" fn cleanup() {
    // Do not hold the registry mutex while waiting for PostgreSQL cleanup.
    let namespaces = std::mem::take(&mut *PATHS.lock().unwrap_or_else(|e| e.into_inner()));
    drop(namespaces);
    if CLEANUP_FAILED.load(Ordering::Relaxed) {
        // SAFETY: cleanup is complete; report failure without unwinding across the C ABI
        // or recursively invoking the exit callbacks currently being executed.
        unsafe { libc::_exit(1) };
    }
}

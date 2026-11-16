//! Fresh PostgreSQL schema ownership; old local SQLite stores are never opened.
use crate::adapters::postgres::client::PostgresClient;
use sha2::{Digest, Sha256};
use sqlx::Row;
use thiserror::Error;
const SCHEMA: &str = include_str!("schema.sql");
const VERSION: i64 = 4;
// Startup only. Runtime guards are retained separately until worker/remote cutover.
const SCHEMA_INITIALIZATION_LOCK: i64 = 0x51756172727a;
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("PostgreSQL schema operation failed")]
    Database(#[from] sqlx::Error),
    #[error("incompatible PostgreSQL schema; use a fresh dedicated Quarry database")]
    Incompatible,
    #[error("provision vector 0.8.6 in quarry_extensions using backend/tools/provision-pgvector.sql before startup")]
    VectorExtension,
    #[error("storage reset refused: {0}")]
    ResetRefused(&'static str),
}
pub(crate) async fn migrate(client: &PostgresClient) -> Result<(), MigrationError> {
    migrate_with_dimensions(client, 1536).await
}
pub(crate) async fn migrate_with_dimensions(
    client: &PostgresClient,
    dimensions: usize,
) -> Result<(), MigrationError> {
    if !(1..=16000).contains(&dimensions) {
        return Err(MigrationError::Incompatible);
    }
    let schema = SCHEMA.replace("vector(1536)", &format!("vector({dimensions})"));
    validate_vector_extension(client).await?;
    let mut tx = client.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),$1))")
        .bind(SCHEMA_INITIALIZATION_LOCK)
        .execute(&mut *tx)
        .await?;
    let tables: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_class
         WHERE relnamespace = current_schema()::regnamespace
           AND relkind IN ('r','p','v','m','S','f')",
    )
    .fetch_one(&mut *tx)
    .await?;
    let checksum = format!("{:x}", Sha256::digest(schema.as_bytes()));
    if tables == 0 {
        initialize(&mut tx, &schema, &checksum).await?;
    } else {
        let exists: bool = sqlx::query_scalar(
            "SELECT to_regclass(current_schema() || '.quarry_schema_version') IS NOT NULL",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(MigrationError::Incompatible);
        }
        let rows =
            sqlx::query("SELECT version,checksum,catalog_fingerprint FROM quarry_schema_version")
                .fetch_all(&mut *tx)
                .await?;
        if rows.len() != 1
            || rows[0].try_get::<i64, _>(0)? != VERSION
            || rows[0].try_get::<String, _>(1)? != checksum
            || rows[0].try_get::<String, _>(2)? != catalog(&mut tx).await?
        {
            return Err(MigrationError::Incompatible);
        }
    }
    let indexes_ready: bool=sqlx::query_scalar("SELECT COALESCE(bool_and(i.indisvalid AND i.indisready),true) FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid WHERE c.relnamespace=current_schema()::regnamespace").fetch_one(&mut *tx).await?;
    if !indexes_ready {
        return Err(MigrationError::Incompatible);
    }
    // Exactly one owner is a transactional workflow invariant beyond the partial unique index.
    let invalid: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM deals d
            WHERE (SELECT count(*) FROM deal_memberships m
                   WHERE m.deal_id=d.deal_id AND m.role='owner')<>1
               OR NOT EXISTS(SELECT 1 FROM deal_access_revisions r WHERE r.deal_id=d.deal_id)
         )",
    )
    .fetch_one(&mut *tx)
    .await?;
    if invalid {
        return Err(MigrationError::Incompatible);
    }
    let invalid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM notebook_search_integrity)")
            .fetch_one(&mut *tx)
            .await?;
    if invalid {
        return Err(MigrationError::Incompatible);
    }
    tx.commit().await?;
    Ok(())
}

async fn validate_vector_extension(client: &PostgresClient) -> Result<(), MigrationError> {
    let extension: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_extension e JOIN pg_namespace n ON n.oid=e.extnamespace WHERE e.extname='vector' AND e.extversion='0.8.6' AND n.nspname='quarry_extensions') AND to_regtype('quarry_extensions.vector') IS NOT NULL AND to_regoperator('quarry_extensions.<=>(quarry_extensions.vector,quarry_extensions.vector)') IS NOT NULL")
        .fetch_one(client.pool()).await?;
    if !extension {
        return Err(MigrationError::VectorExtension);
    }
    Ok(())
}
async fn initialize(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    schema: &str,
    checksum: &str,
) -> Result<(), MigrationError> {
    sqlx::raw_sql(schema).execute(&mut **tx).await?;
    sqlx::raw_sql("CREATE TABLE quarry_schema_version (version BIGINT PRIMARY KEY, checksum TEXT NOT NULL, catalog_fingerprint TEXT NOT NULL)").execute(&mut **tx).await?;
    let fingerprint = catalog(tx).await?;
    sqlx::query("INSERT INTO quarry_schema_version VALUES($1,$2,$3)")
        .bind(VERSION)
        .bind(checksum)
        .bind(fingerprint)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Explicit MVP reset: recreate only the recognized Quarry product schema atomically.
/// Normal startup never calls this operation. Old Helix/SQLite stores are untouched.
///
/// # Errors
/// Refuses unrelated databases/objects and any active application writer. A DDL
/// failure rolls back both deletion and initialization, preserving the old schema.
pub async fn reset_product_schema(
    client: &PostgresClient,
    dimensions: usize,
) -> Result<(), MigrationError> {
    if !(1..=16000).contains(&dimensions) {
        return Err(MigrationError::Incompatible);
    }
    validate_vector_extension(client).await?;
    let mut tx = client.pool().begin().await?;
    let database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&mut *tx)
        .await?;
    if database != "Quarry" && !database.ends_with("_test") {
        return Err(MigrationError::ResetRefused(
            "select Quarry or an isolated _test database",
        ));
    }
    let drained: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended(current_schema(),$1))",
    )
    .bind(crate::adapters::postgres::client::WRITE_LOCK + 1)
    .fetch_one(&mut *tx)
    .await?;
    if !drained {
        return Err(MigrationError::ResetRefused(
            "stop the active Quarry API writer before resetting",
        ));
    }
    // Check ownership before waiting on schema startup: the retained writer
    // also excludes startup, and must be refused rather than waited out.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),$1))")
        .bind(SCHEMA_INITIALIZATION_LOCK)
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),$1))")
        .bind(crate::adapters::postgres::client::WRITE_LOCK)
        .execute(&mut *tx)
        .await?;
    let allowed: std::collections::HashSet<_> = SCHEMA
        .lines()
        .filter_map(|line| line.strip_prefix("CREATE TABLE "))
        .filter_map(|line| line.split_whitespace().next())
        .map(|name| name.trim_matches('"'))
        .chain(["quarry_schema_version", "notebook_cleanup"])
        .collect();
    let tables:Vec<String>=sqlx::query_scalar("SELECT relname FROM pg_class WHERE relnamespace=current_schema()::regnamespace AND relkind IN ('r','p','v','m','f')").fetch_all(&mut *tx).await?;
    if tables.iter().any(|name| {
        !allowed.contains(name.as_str())
            && !["document_search_integrity", "notebook_search_integrity"].contains(&name.as_str())
    }) {
        return Err(MigrationError::ResetRefused(
            "selected schema contains unrelated relations",
        ));
    }
    let functions: Vec<String> = sqlx::query_scalar(
        "SELECT proname FROM pg_proc WHERE pronamespace=current_schema()::regnamespace",
    )
    .fetch_all(&mut *tx)
    .await?;
    if functions.iter().any(|name| {
        ![
            "reserve_deal_identity",
            "retire_deal_identity",
            "verify_document_search_set",
            "verify_notebook_search_set",
        ]
        .contains(&name.as_str())
    }) {
        return Err(MigrationError::ResetRefused(
            "selected schema contains unrelated functions",
        ));
    }
    sqlx::raw_sql("DROP VIEW IF EXISTS document_search_integrity, notebook_search_integrity")
        .execute(&mut *tx)
        .await?;
    let tables: Vec<_> = tables
        .iter()
        .filter(|name| {
            !["document_search_integrity", "notebook_search_integrity"].contains(&name.as_str())
        })
        .map(|name| format!("\"{name}\""))
        .collect();
    if !tables.is_empty() {
        sqlx::raw_sql(&format!("DROP TABLE {}", tables.join(",")))
            .execute(&mut *tx)
            .await?;
    }
    for function in functions {
        sqlx::raw_sql(&format!("DROP FUNCTION \"{function}\"()"))
            .execute(&mut *tx)
            .await?;
    }
    let schema = SCHEMA.replace("vector(1536)", &format!("vector({dimensions})"));
    let checksum = format!("{:x}", Sha256::digest(schema.as_bytes()));
    initialize(&mut tx, &schema, &checksum).await?;
    tx.commit().await?;
    Ok(())
}
async fn catalog(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<String, sqlx::Error> {
    // Include column types/defaults/nullability, constraints, indexes, sequences and functions.
    // Exclude row data and normalize the selected schema name for isolated test namespaces.
    let entries: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT entry FROM (
        SELECT 'relation:' || c.relname || ':' || c.relkind::text AS entry FROM pg_class c
          WHERE c.relnamespace=current_schema()::regnamespace AND c.relkind IN ('r','p','v','m','S','f')
        UNION ALL
        SELECT 'column:' || c.relname || ':' || a.attnum || ':' || a.attname || ':' || format_type(a.atttypid,a.atttypmod) || ':' || a.attnotnull || ':' || a.attidentity::text || ':' || COALESCE(pg_get_expr(d.adbin,d.adrelid),'') || ':' || COALESCE(coll.collname,'')
          FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid
          LEFT JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum
          LEFT JOIN pg_collation coll ON coll.oid=a.attcollation
          WHERE c.relnamespace=current_schema()::regnamespace AND a.attnum>0 AND NOT a.attisdropped
        UNION ALL
        SELECT 'constraint:' || c.relname || ':' || con.conname || ':' || pg_get_constraintdef(con.oid,true)
          FROM pg_constraint con JOIN pg_class c ON c.oid=con.conrelid WHERE c.relnamespace=current_schema()::regnamespace
        UNION ALL
        SELECT 'index:' || indexname || ':' || indexdef FROM pg_indexes WHERE schemaname=current_schema()
        UNION ALL
        SELECT 'function:' || p.proname || ':' || pg_get_functiondef(p.oid) FROM pg_proc p WHERE p.pronamespace=current_schema()::regnamespace
        UNION ALL
        SELECT 'trigger:' || t.tgname || ':' || pg_get_triggerdef(t.oid,true) FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid WHERE c.relnamespace=current_schema()::regnamespace AND NOT t.tgisinternal
        ) definitions ORDER BY entry COLLATE "C"
    "#,
    )
    .fetch_all(&mut **tx)
    .await?;
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&mut **tx)
        .await?;
    let normalized = entries
        .join("\n")
        .replace(&format!("{schema}."), "quarry_schema.");
    Ok(format!("{:x}", Sha256::digest(normalized.as_bytes())))
}
#[cfg(test)]
#[path = "../../tests/integration/app/migrations_tests.rs"]
mod tests;

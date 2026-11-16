//! Codec spike against the shared extension and isolated product namespaces.
use super::*;
use crate::adapters::postgres::{
    client::{PostgresClient, PostgresError},
    query::{Condition, SqlBuilder},
};

#[tokio::test]
async fn qualified_vector_codec_round_trips_without_extension_in_search_path() {
    let client = PostgresClient::test_database().await.unwrap();
    let extension: (String, String) = sqlx::query_as(
        "SELECT extversion,nspname FROM pg_extension e
         JOIN pg_namespace n ON n.oid=e.extnamespace WHERE extname='vector'",
    )
    .fetch_one(client.pool())
    .await
    .expect("provision pgvector once in Quarry_test using backend/tools/provision-pgvector.sql");
    assert_eq!(extension, ("0.8.6".into(), "quarry_extensions".into()));
    let search_path: String = sqlx::query_scalar("SHOW search_path")
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert!(!search_path.contains("quarry_extensions"));
    // An unqualified codec would resolve this shadow type or fail on another pool.
    sqlx::raw_sql(
        "CREATE TYPE vector AS ENUM ('shadow');
         CREATE TABLE codec_vectors (id BIGINT PRIMARY KEY,
                                     embedding quarry_extensions.vector NOT NULL)",
    )
    .execute(client.pool())
    .await
    .unwrap();
    for dimensions in [1536, 3072] {
        let id = i64::try_from(dimensions).unwrap();
        let vector = CosineVector::new(vec![0.5; dimensions], dimensions).unwrap();
        client
            .write_async(
                SqlBuilder::insert_into("codec_vectors")
                    .value("id", id)
                    .value("embedding", vector.clone())
                    .build(),
            )
            .await
            .unwrap();
        let decoded: CosineVector = client
            .read_one_with_async(
                SqlBuilder::select("codec_vectors")
                    .column("embedding")
                    .and_where(Condition::equal("id", id))
                    .build(),
                |row| row.get(0),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decoded, vector);
        let distance: f64 = sqlx::query_scalar(
            "SELECT embedding OPERATOR(quarry_extensions.<=>) $1
             FROM codec_vectors WHERE id=$2",
        )
        .bind(&vector)
        .bind(id)
        .fetch_one(client.pool())
        .await
        .unwrap();
        assert!(distance.abs() < 1e-6);
    }
}

#[tokio::test]
async fn vector_insert_retains_transaction_rollback_and_read_only_enforcement() {
    let client = PostgresClient::test_database().await.unwrap();
    sqlx::raw_sql(
        "CREATE TABLE codec_vectors (id BIGINT PRIMARY KEY,
                                     embedding quarry_extensions.vector(3) NOT NULL)",
    )
    .execute(client.pool())
    .await
    .unwrap();
    let result: Result<(), PostgresError> = client
        .transaction_async(|tx| async move {
            tx.write(
                SqlBuilder::insert_into("codec_vectors")
                    .value("id", 1)
                    .value(
                        "embedding",
                        CosineVector::new(vec![1.0, 0.0, 0.0], 3).unwrap(),
                    )
                    .build(),
            )
            .await?;
            tx.write(
                SqlBuilder::insert_into("codec_vectors")
                    .value("id", 2)
                    .value("embedding", CosineVector::new(vec![1.0, 0.0], 2).unwrap())
                    .build(),
            )
            .await?;
            Ok(())
        })
        .await;
    assert!(result.is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM codec_vectors")
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    let result = client
        .read_transaction_async(|tx| async move {
            tx.write(
                SqlBuilder::insert_into("codec_vectors")
                    .value("id", 3)
                    .value(
                        "embedding",
                        CosineVector::new(vec![1.0, 0.0, 0.0], 3).unwrap(),
                    )
                    .build(),
            )
            .await
        })
        .await;
    assert!(matches!(result, Err(PostgresError::TransactionAborted(_))));
}

#[tokio::test]
async fn cosine_distance_preserves_the_legacy_api_normalization() {
    let client = PostgresClient::test_database().await.unwrap();
    let query = CosineVector::new(vec![1.0, 0.0, 0.0], 3).unwrap();
    for (values, expected) in [
        (vec![1.0, 0.0, 0.0], 0.0),
        (vec![0.0, 1.0, 0.0], 1.0),
        (vec![-1.0, 0.0, 0.0], 2.0),
    ] {
        let distance: f64 = sqlx::query_scalar("SELECT $1 OPERATOR(quarry_extensions.<=>) $2")
            .bind(&query)
            .bind(CosineVector::new(values, 3).unwrap())
            .fetch_one(client.pool())
            .await
            .unwrap();
        assert!((distance - expected).abs() < 1e-6);
        assert!((distance / 2.0 - expected / 2.0).abs() < 1e-6);
    }
}

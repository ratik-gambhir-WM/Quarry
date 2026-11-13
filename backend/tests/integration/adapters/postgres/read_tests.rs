use super::*;
use crate::{
    adapters::postgres::query::{Condition, SqlBuilder},
    app::migrations,
};
use std::time::Duration;

fn select() -> Result<SqlQuery, PostgresError> {
    SqlBuilder::select("app_metadata")
        .columns(["value"])
        .and_where(Condition::equal("key", "read-test"))
        .build()
}

async fn clients() -> (PostgresClient, PostgresClient) {
    let path = format!("read-independence-{}", uuid::Uuid::new_v4());
    let writer = PostgresClient::test_database_at(&path).await.unwrap();
    migrations::migrate(&writer).await.unwrap();
    writer
        .write_async(
            SqlBuilder::insert_into("app_metadata")
                .value("key", "read-test")
                .value("value", "accepted")
                .build(),
        )
        .await
        .unwrap();
    let reader = PostgresClient::test_database_at(&path).await.unwrap();
    (writer.with_writer_lease().await.unwrap(), reader)
}

async fn assert_all_read_entrypoints(client: &PostgresClient) {
    assert_eq!(client.read_async(select()).await.unwrap().len(), 1);
    assert!(client.read_one_async(select()).await.unwrap().is_some());
    assert_eq!(
        client
            .read_with_async(select(), |r| r.get::<_, String>("value"))
            .await
            .unwrap(),
        ["accepted"]
    );
    assert_eq!(
        client
            .read_one_with_async(select(), |r| r.get::<_, String>("value"))
            .await
            .unwrap()
            .as_deref(),
        Some("accepted")
    );
    client
        .read_transaction_async(|tx| async move {
            assert_eq!(tx.read(select()).await?.len(), 1);
            assert!(tx.read_one(select()).await?.is_some());
            assert_eq!(
                tx.read_with(select(), |r| r.get::<_, String>("value"))
                    .await?,
                ["accepted"]
            );
            assert_eq!(
                tx.read_one_with(select(), |r| r.get::<_, String>("value"))
                    .await?
                    .as_deref(),
                Some("accepted")
            );
            Ok(())
        })
        .await
        .unwrap();
    client
        .read_transaction_result_async::<_, anyhow::Error, _, _>(|tx| async move {
            let values = tx
                .read_result_with(select(), |r| {
                    Ok::<_, anyhow::Error>(r.get::<_, String>("value")?)
                })
                .await?;
            assert_eq!(values, ["accepted"]);
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn every_read_entrypoint_is_independent_of_the_writer_session_mutex() {
    let (writer, reader) = clients().await;
    let session = writer
        .writer_lease
        .as_ref()
        .unwrap()
        .connection
        .lock()
        .await;
    tokio::time::timeout(Duration::from_secs(2), assert_all_read_entrypoints(&writer))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), assert_all_read_entrypoints(&reader))
        .await
        .unwrap();
    drop(session);
    writer.close().await;
    reader.close().await;
}

#[tokio::test]
async fn missing_writer_metadata_and_lost_ownership_do_not_disable_reads() {
    let (writer, reader) = clients().await;
    let lease = writer.writer_lease.as_ref().unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut **lease.connection.lock().await.as_mut().unwrap())
        .await
        .unwrap();
    sqlx::query("SELECT pg_terminate_backend($1, 5000)")
        .bind(pid)
        .execute(reader.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM app_metadata WHERE key=$1")
        .bind(WRITER_GENERATION_KEY)
        .execute(reader.pool())
        .await
        .unwrap();
    assert!(matches!(
        writer.transaction_async(|_| async { Ok(()) }).await,
        Err(PostgresError::WriterLeaseLost)
    ));
    assert_all_read_entrypoints(&writer).await;
    assert_all_read_entrypoints(&reader).await;
    writer.close().await;
    reader.close().await;
}

#[tokio::test]
async fn read_snapshot_survives_writer_loss_and_takeover_before_commit() {
    let (writer, reader) = clients().await;
    let lease = writer.writer_lease.as_ref().unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut **lease.connection.lock().await.as_mut().unwrap())
        .await
        .unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let old = writer.clone();
    let task = tokio::spawn(async move {
        old.read_transaction_result_async::<_, anyhow::Error, _, _>(|tx| async move {
            let before = tx
                .read_one_with(select(), |r| r.get::<_, String>("value"))
                .await?;
            started.send(()).unwrap();
            released.await.unwrap();
            let after = tx
                .read_one_with(select(), |r| r.get::<_, String>("value"))
                .await?;
            Ok((before, after))
        })
        .await
    });
    ready.await.unwrap();
    sqlx::query("SELECT pg_terminate_backend($1, 5000)")
        .bind(pid)
        .execute(reader.pool())
        .await
        .unwrap();
    let next = reader.with_writer_lease().await.unwrap();
    next.write_async(
        SqlBuilder::update("app_metadata")
            .set("value", "new")
            .and_where(Condition::equal("key", "read-test"))
            .build(),
    )
    .await
    .unwrap();
    release.send(()).unwrap();
    let (before, after) = task.await.unwrap().unwrap();
    assert_eq!(before.as_deref(), Some("accepted"));
    assert_eq!(after, before);
    assert_eq!(
        next.read_one_with_async(select(), |r| r.get::<_, String>("value"))
            .await
            .unwrap()
            .as_deref(),
        Some("new")
    );
    writer.close().await;
    next.close().await;
}

#[tokio::test]
async fn independent_reader_pool_remains_usable_after_writer_pool_closes() {
    let (writer, reader) = clients().await;
    writer.close().await;
    assert_all_read_entrypoints(&reader).await;
    assert!(matches!(
        writer.read_async(select()).await,
        Err(PostgresError::Database(sqlx::Error::PoolClosed))
    ));
    reader.close().await;
}

#[tokio::test]
async fn reads_finish_while_an_unrelated_write_holds_the_global_guard() {
    let (writer, reader) = clients().await;
    let (written, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        writer
            .transaction_async(|tx| async move {
                tx.write(
                    SqlBuilder::insert_into("app_metadata")
                        .value("key", "uncommitted")
                        .value("value", "pending")
                        .build(),
                )
                .await?;
                written.send(()).unwrap();
                released.await.unwrap();
                Ok(())
            })
            .await
            .unwrap();
        writer.close().await;
    });
    ready.await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), assert_all_read_entrypoints(&reader))
        .await
        .unwrap();
    assert!(!task.is_finished());
    assert!(reader
        .read_one_async(
            SqlBuilder::select("app_metadata")
                .and_where(Condition::equal("key", "uncommitted"))
                .build()
        )
        .await
        .unwrap()
        .is_none());
    release.send(()).unwrap();
    task.await.unwrap();
    assert!(reader
        .read_one_async(
            SqlBuilder::select("app_metadata")
                .and_where(Condition::equal("key", "uncommitted"))
                .build()
        )
        .await
        .unwrap()
        .is_some());
    reader.close().await;
}

#[tokio::test]
async fn cancelling_read_snapshots_releases_their_connections() {
    let (writer, reader) = clients().await;
    let mut tasks = Vec::new();
    // Fill the independent reader pool with paused snapshots.
    for _ in 0..4 {
        let client = reader.clone();
        let (started, ready) = tokio::sync::oneshot::channel();
        tasks.push(tokio::spawn(async move {
            client
                .read_transaction_async(|tx| async move {
                    tx.read(select()).await?;
                    started.send(()).unwrap();
                    std::future::pending::<()>().await;
                    Ok(())
                })
                .await
        }));
        ready.await.unwrap();
    }
    for task in tasks {
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
    tokio::time::timeout(Duration::from_secs(2), assert_all_read_entrypoints(&reader))
        .await
        .unwrap();
    writer.close().await;
    reader.close().await;
}

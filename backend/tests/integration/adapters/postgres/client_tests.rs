use super::*;
use crate::{
    adapters::postgres::query::{Condition, SqlBuilder},
    app::migrations,
};
async fn client() -> PostgresClient {
    let c = PostgresClient::test_database().await.unwrap();
    migrations::migrate(&c).await.unwrap();
    c
}
fn insert(key: &str) -> Result<SqlQuery, PostgresError> {
    SqlBuilder::insert_into("app_metadata")
        .value("key", key)
        .value("value", "text")
        .build()
}
fn select(key: &str) -> Result<SqlQuery, PostgresError> {
    SqlBuilder::select("app_metadata")
        .and_where(Condition::equal("key", key))
        .build()
}

#[tokio::test]
async fn borrowed_parameters_preserve_types_nulls_and_large_buffers_on_reuse() {
    use crate::adapters::postgres::query::SqlValue;
    let c = client().await;
    let blob = vec![0xa5; 1024 * 1024];
    let text = "Unicode café 'quoted' $1".repeat(1024);
    let query = SqlQuery::new(
        "SELECT $1::bigint, $2::double precision, $3::text, $4::bytea, $5::text".into(),
        vec![
            SqlValue::Integer(i64::MAX),
            SqlValue::Real(1.25),
            SqlValue::Text(text.clone()),
            SqlValue::Blob(blob.clone()),
            SqlValue::Null,
        ],
        QueryKind::Select,
    );
    for _ in 0..2 {
        let row = bind(&query).fetch_one(c.pool()).await.unwrap();
        let row = Row(&row);
        assert_eq!(row.get::<_, i64>(0).unwrap(), i64::MAX);
        assert_eq!(row.get::<_, f64>(1).unwrap(), 1.25);
        assert_eq!(row.get::<_, String>(2).unwrap(), text);
        assert_eq!(row.get::<_, Vec<u8>>(3).unwrap(), blob);
        assert_eq!(row.get::<_, Option<String>>(4).unwrap(), None);
    }
}
async fn writer_pid(client: &PostgresClient) -> i32 {
    let lease = client.writer_lease.as_ref().unwrap();
    let mut guard = lease.connection.lock().await;
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut **guard.as_mut().unwrap())
        .await
        .unwrap()
}
async fn terminate_writer(client: &PostgresClient, pid: i32) {
    // Wait for actual termination, not just signal delivery, before testing takeover.
    let stopped: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1, 5000)")
        .bind(pid)
        .fetch_one(client.pool())
        .await
        .unwrap();
    assert!(stopped);
}
async fn wait_for_blocked_transaction(client: &PostgresClient, blocker: i32) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(blocker)
            .fetch_one(client.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn rollback_read_only_and_affected_rows_preserve_transaction_contract() {
    let c = client().await;
    let error = c
        .transaction_async(|tx| async move {
            tx.write(insert("rolled-back")).await?;
            Err::<(), _>(PostgresError::transaction_aborted("deliberate failure"))
        })
        .await;
    assert!(error.is_err());
    assert!(c
        .read_one_async(select("rolled-back"))
        .await
        .unwrap()
        .is_none());
    assert!(c
        .read_transaction_async(|tx| async move { tx.write(insert("forbidden")).await })
        .await
        .is_err());
    assert_eq!(
        c.write_async(insert("committed"))
            .await
            .unwrap()
            .rows_affected,
        1
    );
    assert!(c
        .write_async(insert("committed"))
        .await
        .unwrap_err()
        .is_unique_violation());
    let no_op = SqlBuilder::insert_into("app_metadata")
        .value("key", "committed")
        .value("value", "different")
        .on_conflict_do_nothing(["key"])
        .build();
    assert_eq!(c.write_async(no_op).await.unwrap().rows_affected, 0);
    assert_eq!(
        c.read_one_async(select("committed"))
            .await
            .unwrap()
            .unwrap()
            .get("value"),
        Some(&super::super::query::SqlValue::from("text"))
    );
}
#[tokio::test]
async fn repeatable_read_snapshot_overlaps_writer_without_changing_its_view() {
    let c = client().await.with_writer_lease().await.unwrap();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let reader = c.clone();
    let snapshot = tokio::spawn(async move {
        reader
            .read_transaction_async(|tx| async move {
                let first = tx.read(select("later")).await?;
                started.send(()).unwrap();
                released.await.unwrap();
                let second = tx.read(select("later")).await?;
                Ok((first, second))
            })
            .await
            .unwrap()
    });
    ready.await.unwrap();
    c.write_async(insert("later")).await.unwrap();
    release.send(()).unwrap();
    let (before, after) = snapshot.await.unwrap();
    assert!(before.is_empty() && after.is_empty());
    assert!(c.read_one_async(select("later")).await.unwrap().is_some());
}
#[tokio::test]
async fn dropping_a_transaction_rolls_back_and_releases_the_writer_fence() {
    let c = client().await.with_writer_lease().await.unwrap();
    let task_client = c.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        task_client
            .transaction_async(|tx| async move {
                tx.write(insert("cancelled")).await?;
                started.send(()).unwrap();
                std::future::pending::<()>().await;
                Ok(())
            })
            .await
    });
    ready.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    c.write_async(insert("after-cancel")).await.unwrap();
    assert!(c
        .read_one_async(select("cancelled"))
        .await
        .unwrap()
        .is_none());
}
#[tokio::test]
async fn exhausted_or_closed_pool_is_bounded_and_builder_failures_need_no_connection() {
    let c = client().await;
    let mut leases = Vec::new();
    for _ in 0..4 {
        leases.push(c.pool().acquire().await.unwrap());
    }
    let construction = SqlBuilder::update("users")
        .set("first_name", "missing condition")
        .build();
    assert!(tokio::time::timeout(
        std::time::Duration::from_millis(100),
        c.write_async(construction)
    )
    .await
    .unwrap()
    .is_err());
    // Five-second acquisition deadline belongs to the adapter, not only the caller.
    let began = std::time::Instant::now();
    assert!(matches!(
        c.read_one_async(select("anything")).await,
        Err(PostgresError::Database(sqlx::Error::PoolTimedOut))
    ));
    assert!(began.elapsed() < std::time::Duration::from_secs(7));
    drop(leases);
    c.close().await;
    assert!(matches!(
        c.read_one_async(select("anything")).await,
        Err(PostgresError::Database(sqlx::Error::PoolClosed))
    ));
}
#[tokio::test]
async fn application_writer_ownership_lives_through_clones_and_fails_closed_on_loss() {
    let c = client().await;
    let first = c.clone().with_writer_lease().await.unwrap();
    let detached_worker = first.clone();
    drop(first);
    assert!(matches!(
        c.clone().with_writer_lease().await,
        Err(PostgresError::WriterBusy)
    ));
    detached_worker.write_async(insert("owned")).await.unwrap();
    // Terminate only the dedicated session created by this test, never another application's.
    let pid = writer_pid(&detached_worker).await;
    terminate_writer(&c, pid).await;
    assert!(matches!(
        detached_worker.write_async(insert("lost")).await,
        Err(PostgresError::WriterLeaseLost)
    ));
    assert!(detached_worker
        .read_one_async(select("owned"))
        .await
        .unwrap()
        .is_some());
    drop(detached_worker);
    let next = c.clone().with_writer_lease().await.unwrap();
    next.write_async(insert("next-owner")).await.unwrap();
    next.close().await;
}

#[tokio::test]
async fn writer_loss_during_write_transactions_rolls_back_before_takeover_writes() {
    let c = client().await;
    let writer = c.clone().with_writer_lease().await.unwrap();
    let pid = writer_pid(&writer).await;
    let old = writer.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        old.transaction_async(|tx| async move {
            tx.write(insert("old-owner-write")).await?;
            started.send(()).unwrap();
            released.await.unwrap();
            Ok::<_, PostgresError>(())
        })
        .await
    });
    ready.await.unwrap();
    terminate_writer(&c, pid).await;
    // Takeover must complete even while the stale aggregate holds WRITE_LOCK.
    let next = c.clone().with_writer_lease().await.unwrap();
    release.send(()).unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(PostgresError::WriterLeaseLost)
    ));
    assert!(next
        .read_one_async(select("old-owner-write"))
        .await
        .unwrap()
        .is_none());
    next.write_async(insert("replacement-write")).await.unwrap();
    assert!(matches!(
        writer.write_async(insert("stale-retry")).await,
        Err(PostgresError::WriterLeaseLost)
    ));
    drop(writer);
    next.close().await;
}

#[tokio::test]
async fn a_changed_writer_generation_fences_an_otherwise_healthy_session() {
    let c = client().await;
    let writer = c.clone().with_writer_lease().await.unwrap();
    let admin = c.clone();
    let result = writer
        .transaction_async(|tx| async move {
            tx.write(insert("stale-generation")).await?;
            sqlx::query("UPDATE app_metadata SET value='replacement' WHERE key=$1")
                .bind(WRITER_GENERATION_KEY)
                .execute(admin.pool())
                .await?;
            Ok(())
        })
        .await;
    assert!(matches!(result, Err(PostgresError::WriterLeaseLost)));
    assert!(c
        .read_one_async(select("stale-generation"))
        .await
        .unwrap()
        .is_none());
    writer.close().await;
}

#[tokio::test]
async fn takeover_waits_for_a_transaction_already_at_the_commit_fence() {
    let c = client().await;
    let writer = c.clone().with_writer_lease().await.unwrap();
    let pid = writer_pid(&writer).await;
    let mut tx = c.pool().begin().await.unwrap();
    let transaction_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    writer.fence_commit(&mut tx).await.unwrap();
    terminate_writer(&c, pid).await;
    let replacement = c.clone();
    let takeover = tokio::spawn(async move { replacement.with_writer_lease().await });
    // Observe the actual row-lock wait instead of relying on a scheduling delay.
    wait_for_blocked_transaction(&c, transaction_pid).await;
    assert!(!takeover.is_finished());
    tx.commit().await.unwrap();
    let next = takeover.await.unwrap().unwrap();
    next.write_async(insert("new-owner")).await.unwrap();
    drop(writer);
    next.close().await;
}

#[tokio::test]
async fn a_transaction_waiting_for_the_aggregate_lock_rejects_takeover_before_running_work() {
    let c = client().await;
    let writer = c.clone().with_writer_lease().await.unwrap();
    let pid = writer_pid(&writer).await;
    let mut blocker = c.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),$1))")
        .bind(WRITE_LOCK)
        .execute(&mut *blocker)
        .await
        .unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let operation_called = called.clone();
    let task = tokio::spawn(async move {
        writer
            .transaction_async(|_| async move {
                operation_called.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(())
            })
            .await
    });
    wait_for_blocked_transaction(&c, blocker_pid).await;
    terminate_writer(&c, pid).await;
    let next = c.clone().with_writer_lease().await.unwrap();
    blocker.rollback().await.unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(PostgresError::WriterLeaseLost)
    ));
    assert!(!called.load(std::sync::atomic::Ordering::Relaxed));
    next.close().await;
}

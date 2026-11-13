//! Exercise SQL primitives on independent pools without the retained global write
//! guard. This harness exists only in tests; it is not a production bypass.
use super::*;
use crate::{
    adapters::postgres::query::{
        Condition as C, LockWait, RowLock, SortDirection, SqlBuilder as Q,
    },
    app::migrations,
};
use std::time::Duration;

async fn clients() -> (PostgresClient, PostgresClient) {
    let path = format!("concurrent-primitives-{}", uuid::Uuid::new_v4());
    let a = PostgresClient::test_database_at(&path).await.unwrap();
    migrations::migrate(&a).await.unwrap();
    for key in ["a", "b"] {
        a.write_async(
            Q::insert_into("app_metadata")
                .value("key", key)
                .value("value", "0")
                .build(),
        )
        .await
        .unwrap();
    }
    let b = PostgresClient::test_database_at(&path).await.unwrap();
    (a, b)
}

async fn unguarded<T, F, Fut>(client: &PostgresClient, operation: F) -> Result<T, PostgresError>
where
    F: FnOnce(PostgresTransaction) -> Fut,
    Fut: Future<Output = Result<T, PostgresError>>,
{
    assert!(client.writer_lease.is_none());
    let tx = client.pool().begin().await?;
    client.execute_transaction(tx, false, operation).await
}

fn lock(key: &str, mode: RowLock) -> Result<SqlQuery, PostgresError> {
    Q::select("app_metadata")
        .and_where(C::equal("key", key))
        .lock(mode, LockWait::Wait)
        .build()
}

fn replace(key: &str) -> Result<SqlQuery, PostgresError> {
    Q::update("app_metadata")
        .set("value", "1")
        .and_where(C::equal("key", key))
        .and_where(C::equal("value", "0"))
        .returning("key")
        .build()
}

#[tokio::test]
async fn locking_and_returning_queries_cannot_enter_any_read_runner() {
    let (a, b) = clients().await;
    a.read_transaction_async(|tx| async move {
        assert!(matches!(
            tx.read(lock("a", RowLock::Share)).await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(matches!(
            tx.read_one(lock("a", RowLock::Update)).await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(matches!(
            tx.read_with(replace("a"), |r| r.get::<_, String>("key"))
                .await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(tx
            .lock_with(lock("a", RowLock::Share), |r| r.get::<_, String>("key"))
            .await
            .is_err());
        assert!(tx
            .write_returning_with(replace("a"), |r| r.get::<_, String>("key"))
            .await
            .is_err());
        assert!(tx
            .write(
                Q::delete_from("app_metadata")
                    .and_where(C::equal("key", "a"))
                    .build()
            )
            .await
            .is_err());
        Ok(())
    })
    .await
    .unwrap();
    a.close().await;
    for query in [
        lock("a", RowLock::Share),
        lock("a", RowLock::Update),
        replace("a"),
        Q::delete_from("app_metadata")
            .and_where(C::equal("key", "a"))
            .returning("key")
            .build(),
    ] {
        // Classification must reject before a closed/exhausted pool or a mapper runs.
        let query = query.unwrap();
        assert!(matches!(
            a.read_async(Ok(query.clone())).await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(matches!(
            a.read_one_async(Ok(query.clone())).await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(matches!(
            a.read_with_async(Ok(query.clone()), |_| -> Result<(), PostgresError> {
                panic!("mapper ran")
            })
            .await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(matches!(
            a.read_one_with_async(Ok(query), |_| -> Result<(), PostgresError> {
                panic!("mapper ran")
            })
            .await,
            Err(PostgresError::InvalidOperation { .. })
        ));
    }
    b.close().await;
}

#[tokio::test]
async fn skip_locked_claims_a_different_resource_before_the_first_lock_is_released() {
    let (a, b) = clients().await;
    let (locked, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        unguarded(&a, |tx| async move {
            assert_eq!(
                tx.lock_with(lock("a", RowLock::Update), |r| r.get::<_, String>("key"))
                    .await?,
                ["a"]
            );
            locked.send(()).unwrap();
            released.await.unwrap();
            Ok(())
        })
        .await
        .unwrap();
        a.close().await;
    });
    ready.await.unwrap();
    let selected = tokio::time::timeout(
        Duration::from_secs(2),
        unguarded(&b, |tx| async move {
            tx.lock_with(
                Q::select("app_metadata")
                    .order_by("key", SortDirection::Ascending)
                    .limit(1)
                    .lock(RowLock::Update, LockWait::SkipLocked)
                    .build(),
                |r| r.get::<_, String>("key"),
            )
            .await
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(selected, ["b"]);
    assert!(!task.is_finished());
    release.send(()).unwrap();
    task.await.unwrap();
    b.close().await;
}

#[tokio::test]
async fn shared_resource_locks_overlap_but_exclusive_mutation_waits_for_commit() {
    let (a, b) = clients().await;
    let mut blocker = a.pool().begin().await.unwrap();
    let query = lock("a", RowLock::Share).unwrap();
    bind(&query).fetch_all(&mut *blocker).await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    tokio::time::timeout(
        Duration::from_secs(2),
        unguarded(&b, |tx| async move {
            tx.lock_with(lock("a", RowLock::Share), |r| r.get::<_, String>("key"))
                .await
        }),
    )
    .await
    .unwrap()
    .unwrap();
    let contender = b.clone();
    let task = tokio::spawn(async move {
        unguarded(&contender, |tx| async move {
            tx.write_returning_with(replace("a"), |r| r.get::<_, String>("key"))
                .await
        })
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(pid)
            .fetch_one(a.pool())
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
    assert!(!task.is_finished());
    blocker.commit().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap(), ["a"]);
    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn atomic_predicate_has_one_winner_on_independent_connections() {
    let (a, b) = clients().await;
    let start = Arc::new(tokio::sync::Barrier::new(2));
    let compete = |client: PostgresClient, barrier: Arc<tokio::sync::Barrier>| async move {
        let result = unguarded(&client, |tx| async move {
            barrier.wait().await;
            tx.write_returning_with(replace("a"), |r| r.get::<_, String>("key"))
                .await
        })
        .await
        .unwrap();
        client.close().await;
        result.len()
    };
    let (first, second) = tokio::join!(compete(a, start.clone()), compete(b, start));
    assert_eq!(first + second, 1);
}

#[tokio::test]
async fn independent_resource_mutations_commit_while_another_transaction_is_paused() {
    let (a, b) = clients().await;
    let (written, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        unguarded(&a, |tx| async move {
            tx.write_returning_with(replace("a"), |r| r.get::<_, String>("key"))
                .await?;
            written.send(()).unwrap();
            released.await.unwrap();
            Err::<(), _>(PostgresError::transaction_aborted("rollback paused write"))
        })
        .await
        .unwrap_err();
        a.close().await;
    });
    ready.await.unwrap();
    let winner = tokio::time::timeout(
        Duration::from_secs(2),
        unguarded(&b, |tx| async move {
            tx.write_returning_with(replace("b"), |r| r.get::<_, String>("key"))
                .await
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(winner, ["b"]);
    assert!(!task.is_finished());
    release.send(()).unwrap();
    task.await.unwrap();
    let rows = b
        .read_with_async(
            Q::select("app_metadata")
                .order_by("key", SortDirection::Ascending)
                .build(),
            |r| r.get::<_, String>("value"),
        )
        .await
        .unwrap();
    assert_eq!(rows, ["0", "1"]);
    b.close().await;
}

#[tokio::test]
async fn database_deadlines_fence_expired_updates_and_return_the_accepted_rows() {
    let (a, b) = clients().await;
    sqlx::query("CREATE TABLE deadlines (id TEXT PRIMARY KEY, lease_until BIGINT)")
        .execute(a.pool())
        .await
        .unwrap();
    a.write_async(
        Q::insert_into("deadlines")
            .value("id", "job")
            .value("lease_until", 0_i64)
            .build(),
    )
    .await
    .unwrap();
    unguarded(&a, |tx| async move {
        let claimed = tx
            .write_returning_with(
                Q::update("deadlines")
                    .set_epoch_seconds_after("lease_until", 180)
                    .and_where(C::equal("id", "job"))
                    .and_where(C::expired("lease_until"))
                    .returning("lease_until")
                    .build(),
                |r| r.get::<_, i64>("lease_until"),
            )
            .await?;
        assert_eq!(claimed.len(), 1);
        let clock_query = SqlQuery::new(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint AS now".into(),
            Vec::new(),
            QueryKind::Select,
        );
        let now = tx
            .read_one_with(Ok(clock_query), |r| r.get::<_, i64>("now"))
            .await?
            .unwrap();
        assert!((179..=180).contains(&(claimed[0] - now)));
        let expired = tx
            .write_returning_with(
                Q::delete_from("deadlines")
                    .and_where(C::equal("id", "job"))
                    .and_where(C::expired("lease_until"))
                    .returning("id")
                    .build(),
                |r| r.get::<_, String>("id"),
            )
            .await?;
        assert!(expired.is_empty());
        let live = tx
            .write_returning_with(
                Q::delete_from("deadlines")
                    .and_where(C::equal("id", "job"))
                    .and_where(C::unexpired("lease_until"))
                    .returning("id")
                    .build(),
                |r| r.get::<_, String>("id"),
            )
            .await?;
        assert_eq!(live, ["job"]);
        Ok(())
    })
    .await
    .unwrap();
    assert!(b
        .read_async(Q::select("deadlines").build())
        .await
        .unwrap()
        .is_empty());
    a.close().await;
    b.close().await;
}

#[tokio::test]
async fn failed_returning_mapper_rolls_back_and_wrong_query_kinds_never_run() {
    let (a, b) = clients().await;
    let result = unguarded(&a, |tx| async move {
        let inserted = tx
            .write_returning_with(
                Q::insert_into("users")
                    .value("first_name", "Test")
                    .value("last_name", "Fixture")
                    .value("email", "returning@example.invalid")
                    .value("username", "returning")
                    .value("password_hash", "unused-test-hash")
                    .build(),
                |r| r.get::<_, i64>("id"),
            )
            .await?;
        assert_eq!(inserted.len(), 1);
        assert!(inserted[0] > 0);
        assert!(matches!(
            tx.lock_with(Q::select("app_metadata").build(), |_| Ok(()))
                .await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(matches!(
            tx.write(lock("a", RowLock::Update)).await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        assert!(matches!(
            tx.write_returning_with(
                Q::update("app_metadata")
                    .set("value", "1")
                    .and_where(C::equal("key", "a"))
                    .build(),
                |_| Ok(())
            )
            .await,
            Err(PostgresError::InvalidOperation { .. })
        ));
        tx.write_returning_with(replace("a"), |_| {
            Err::<(), _>(PostgresError::decode("deliberate mapper error"))
        })
        .await
    })
    .await;
    assert!(matches!(result, Err(PostgresError::Decode(_))));
    let value = b
        .read_one_with_async(
            Q::select("app_metadata")
                .and_where(C::equal("key", "a"))
                .build(),
            |r| r.get::<_, String>("value"),
        )
        .await
        .unwrap();
    assert_eq!(value.as_deref(), Some("0"));
    assert!(b
        .read_async(Q::select("users").build())
        .await
        .unwrap()
        .is_empty());
    a.close().await;
    b.close().await;
}

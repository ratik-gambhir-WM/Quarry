use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn stop_signals_the_supervisor_and_joins_it() {
    let observed = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&observed);
    let handle = WorkerHandle::spawn(move |mut stop| async move {
        stop.requested().await;
        seen.store(1, Ordering::SeqCst);
    });
    tokio::time::timeout(Duration::from_secs(2), handle.stop())
        .await
        .unwrap();
    assert_eq!(observed.load(Ordering::SeqCst), 1);
    // A second stop is a no-op rather than a hang.
    tokio::time::timeout(Duration::from_secs(2), handle.stop())
        .await
        .unwrap();
}

#[tokio::test]
async fn dropping_every_handle_detaches_instead_of_stopping() {
    let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
    let handle = WorkerHandle::spawn(move |mut stop| async move {
        tokio::select! {
            _ = stop.requested() => panic!("a dropped handle must not request a stop"),
            _ = receiver => {}
        }
    });
    drop(handle);
    tokio::task::yield_now().await;
    sender.send(()).unwrap();
}

#[tokio::test]
async fn drain_waits_for_quick_tasks_and_aborts_stuck_ones() {
    let finished = Arc::new(AtomicUsize::new(0));
    let mut active = JoinSet::new();
    let quick = Arc::clone(&finished);
    active.spawn(async move {
        quick.fetch_add(1, Ordering::SeqCst);
    });
    let stuck = Arc::clone(&finished);
    active.spawn(async move {
        std::future::pending::<()>().await;
        stuck.fetch_add(10, Ordering::SeqCst);
    });
    tokio::time::timeout(
        Duration::from_secs(2),
        drain_owned(&mut active, Duration::from_millis(50)),
    )
    .await
    .unwrap();
    assert!(active.is_empty());
    assert_eq!(finished.load(Ordering::SeqCst), 1);
}

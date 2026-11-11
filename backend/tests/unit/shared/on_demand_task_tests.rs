use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_notifications_share_one_coordinator() {
    let task = Arc::new(OnDemandTask::default());
    let calls = Arc::new(AtomicUsize::new(0));
    let mut notifications = tokio::task::JoinSet::new();
    for _ in 0..100 {
        let calls = calls.clone();
        let task = task.clone();
        notifications.spawn(async move {
            task.notify(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                std::future::pending::<()>().await;
            });
        });
    }
    while let Some(result) = notifications.join_next().await {
        result.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    task.request_stop();
    let handle = task.take_task().unwrap();
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn enqueue_during_empty_scan_cannot_be_lost_at_retirement() {
    let task = Arc::new(OnDemandTask::default());
    let (scanned, scan) = tokio::sync::oneshot::channel();
    let (finish, finished) = tokio::sync::oneshot::channel();
    let runner = task.clone();
    task.notify(async move {
        runner.begin_scan();
        scanned.send(()).unwrap();
        // The queue was observed empty, but an enqueue commits before retirement.
        finished.await.unwrap();
        assert!(!runner.retire_if_idle());
        runner.begin_scan();
        assert!(runner.retire_if_idle());
    });
    scan.await.unwrap();
    task.notify(async { panic!("must wake the existing coordinator") });
    let handle = task.take_task().unwrap();
    finish.send(()).unwrap();
    handle.await.unwrap();

    // A request after retirement must start a successor.
    let (ran, received) = tokio::sync::oneshot::channel();
    task.notify(async { ran.send(()).unwrap() });
    received.await.unwrap();
    task.take_task().unwrap().await.unwrap();
}

#[tokio::test]
async fn stop_prevents_late_requests_from_starting_tasks() {
    let task = OnDemandTask::default();
    task.request_stop();
    task.notify(async { panic!("a stopped worker must not restart") });
    assert!(task.take_task().is_none());
}

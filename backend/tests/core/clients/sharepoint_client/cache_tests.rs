use super::*;

#[tokio::test]
async fn stores_expires_deletes_and_clears_values() {
    let cache = InMemoryCache::default();
    cache
        .set("present", Value::String("value".into()), None)
        .await;
    assert_eq!(
        cache.get("present").await,
        Some(Value::String("value".into()))
    );

    cache
        .set(
            "short",
            Value::String("value".into()),
            Some(Duration::from_millis(1)),
        )
        .await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(cache.get("short").await, None);

    cache.set("a", Value::Bool(true), None).await;
    cache.delete("a").await;
    assert_eq!(cache.get("a").await, None);

    cache.set("a", Value::Bool(true), None).await;
    cache.set("b", Value::Bool(false), None).await;
    cache.clear().await;
    assert_eq!(cache.get("a").await, None);
    assert_eq!(cache.get("b").await, None);
}

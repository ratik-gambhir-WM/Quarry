import { describe, expect, test, beforeEach } from "bun:test";
import { InMemoryCache } from "../cache";

describe("InMemoryCache", () => {
  let cache: InMemoryCache;

  beforeEach(() => {
    cache = new InMemoryCache();
  });

  test("returns undefined for missing keys", async () => {
    expect(await cache.get("nonexistent")).toBeUndefined();
  });

  test("stores and retrieves a value", async () => {
    await cache.set("key", { foo: "bar" });
    expect(await cache.get<{ foo: string }>("key")).toEqual({ foo: "bar" });
  });

  test("stores values of different types", async () => {
    await cache.set("string", "hello");
    await cache.set("number", 42);
    await cache.set("boolean", true);
    await cache.set("array", [1, 2, 3]);

    expect(await cache.get<string>("string")).toBe("hello");
    expect(await cache.get<number>("number")).toBe(42);
    expect(await cache.get<boolean>("boolean")).toBe(true);
    expect(await cache.get<number[]>("array")).toEqual([1, 2, 3]);
  });

  test("expires entries after TTL", async () => {
    await cache.set("key", "value", 50); // 50ms TTL
    expect(await cache.get<string>("key")).toBe("value");

    await new Promise((r) => setTimeout(r, 60));
    expect(await cache.get("key")).toBeUndefined();
  });

  test("delete removes a key", async () => {
    await cache.set("key", "value");
    await cache.delete("key");
    expect(await cache.get("key")).toBeUndefined();
  });

  test("delete is a no-op for missing keys", async () => {
    await cache.delete("nonexistent"); // should not throw
  });

  test("clear removes all keys", async () => {
    await cache.set("a", 1);
    await cache.set("b", 2);
    await cache.clear();
    expect(await cache.get("a")).toBeUndefined();
    expect(await cache.get("b")).toBeUndefined();
  });

  test("overwriting a key replaces the value", async () => {
    await cache.set("key", "old");
    await cache.set("key", "new");
    expect(await cache.get<string>("key")).toBe("new");
  });

  test("overwriting a key resets the TTL", async () => {
    await cache.set("key", "old", 50);
    await new Promise((r) => setTimeout(r, 30));
    await cache.set("key", "new", 100);
    await new Promise((r) => setTimeout(r, 40));
    // Original TTL would have expired, but re-set extended it
    expect(await cache.get<string>("key")).toBe("new");
  });

  test("uses default TTL of 5 minutes when not specified", async () => {
    await cache.set("key", "value");
    // Should still be alive immediately
    expect(await cache.get<string>("key")).toBe("value");
  });
});

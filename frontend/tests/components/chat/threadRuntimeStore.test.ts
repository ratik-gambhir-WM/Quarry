import { describe, expect, it, vi } from "vitest";
import { createThreadRuntimeStore, type ThreadRuntimeState } from "@/components/chat/threadRuntimeStore";
import { acceptDealDeletion } from "@/data/dealDeletion/lifecycle";

function initial(scopeDealId: string): ThreadRuntimeState {
  return {
    scopeDealId,
    nextCursor: null,
    copiedMessageId: null,
    deletingThreadId: null,
    draft: "Private draft",
    draftFiles: [],
    draftReferences: [],
    attachmentError: null,
    selectedModel: { type: "default" },
    sendError: null,
    isPreparingThread: false,
    isRunning: false,
    isThreadListLoading: false,
    isThreadLoading: false,
    messages: [],
    selectedThreadId: null,
    threadDeleteError: false,
    threadLoadError: false,
    threadListError: false,
    threads: [],
  };
}

describe("threadRuntimeStore subscriptions", () => {
  it("does no subscription work during construction or snapshot reads and catches a missed deletion on subscribe", async () => {
    const invalidate = vi.fn();
    const store = createThreadRuntimeStore(initial("DEAL-STORE-GAP"), invalidate);
    const snapshot = store.getSnapshot();
    expect(store.getSnapshot()).toBe(snapshot);
    await acceptDealDeletion("DEAL-STORE-GAP", async () => null);
    expect(invalidate).not.toHaveBeenCalled();
    expect(store.getSnapshot()).toBe(snapshot);
    const stop = store.subscribe(vi.fn());
    expect(invalidate).toHaveBeenCalledTimes(1);
    expect(store.getSnapshot().draft).toBe("");
    stop();
  });

  it("keeps one active subscription across reconnections and ignores unrelated deletions", async () => {
    const invalidate = vi.fn();
    const listener = vi.fn();
    const store = createThreadRuntimeStore(initial("DEAL-STORE-RECONNECT"), invalidate);
    store.subscribe(listener)();
    const stop = store.subscribe(listener);
    const snapshot = store.getSnapshot();
    await acceptDealDeletion("DEAL-STORE-OTHER", async () => null);
    expect(store.getSnapshot()).toBe(snapshot);
    expect(listener).not.toHaveBeenCalled();
    await acceptDealDeletion("DEAL-STORE-RECONNECT", async () => null);
    expect(invalidate).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledTimes(1);
    stop();
    await acceptDealDeletion("DEAL-STORE-RECONNECT", async () => null);
    expect(invalidate).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledTimes(1);
  });
});

it("removes unselected deleted-deal chats without clearing or cancelling another conversation", async () => {
  const removed = { threadId: "deleted-thread", dealId: "DEAL-STORE-LIST", title: "Synthetic", status: "regular" as const, createdAt: "now", updatedAt: "now", lastMessageAt: "now" };
  const kept = { ...removed, threadId: "kept-thread", dealId: null };
  const invalidate = vi.fn();
  const store = createThreadRuntimeStore({ ...initial(""), scopeDealId: null, selectedThreadId: kept.threadId, threads: [removed, kept] }, invalidate);
  const stop = store.subscribe(vi.fn());
  await acceptDealDeletion(removed.dealId, async () => null);
  expect(store.getSnapshot()).toMatchObject({ selectedThreadId: kept.threadId, draft: "Private draft", threads: [kept] });
  expect(invalidate).not.toHaveBeenCalled();
  stop();
});

it("preserves the thread catalog reference during composer and streaming updates", () => {
  const store = createThreadRuntimeStore(initial(""), vi.fn());
  const threads = store.getSnapshot().threads;
  store.update({ draft: "Edited" });
  expect(store.getSnapshot().threads).toBe(threads);
  store.update({ messages: [{ id: "reply", role: "assistant", content: [{ type: "text", text: "Streaming" }], status: { type: "running" } }] });
  expect(store.getSnapshot().threads).toBe(threads);
});

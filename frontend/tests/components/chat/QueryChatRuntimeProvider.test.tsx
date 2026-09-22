// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";
import { QueryChatRuntimeProvider } from "@/components/chat/QueryChatRuntimeProvider";
import { QueryChatThread } from "@/components/chat/QueryChat";
import { AssistantWorkspaceSidebar } from "@/components/hub/sidebar/AssistantWorkspaceSidebar";

const { api } = vi.hoisted(() => ({
  api: {
    createAssistantThread: vi.fn(),
    deleteAssistantThread: vi.fn(),
    getAssistantThread: vi.fn(),
    listAssistantThreads: vi.fn(),
    queryModel: vi.fn(),
    runAssistantThread: vi.fn(),
  },
}));

vi.mock("@quarry/runtime", () => ({ runtime: { api } }));

afterEach(() => {
  cleanup();
  api.createAssistantThread.mockReset();
  api.deleteAssistantThread.mockReset();
  api.getAssistantThread.mockReset();
  api.listAssistantThreads.mockReset();
  api.queryModel.mockReset();
  api.runAssistantThread.mockReset();
});

describe("QueryChatRuntimeProvider", () => {
  it("loads the selected previous thread's messages", async () => {
    const user = userEvent.setup();
    let resolveThread: ((thread: typeof threadDetail) => void) | undefined;
    const threadLoad = new Promise<typeof threadDetail>((resolve) => {
      resolveThread = resolve;
    });
    api.listAssistantThreads.mockResolvedValue({
      nextCursor: null,
      threads: [threadMetadata],
    });
    api.getAssistantThread.mockReturnValue(threadLoad);

    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
        <MemoryRouter>
          <AssistantWorkspaceSidebar activeHomeSection="assistant" email="analyst@example.com" tools={[]} />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    await user.click(await screen.findByRole("button", { name: threadMetadata.title }));

    expect(await screen.findByTestId("thread-history-skeleton")).toBeTruthy();
    resolveThread?.(threadDetail);
    const userMessage = await screen.findByText("Earlier user question");
    const assistantMessage = screen.getByText("Earlier assistant answer");
    expect(userMessage.closest('[data-role="user"]')).toBeTruthy();
    expect(assistantMessage.closest('[data-role="assistant"]')).toBeTruthy();
    expect(api.getAssistantThread).toHaveBeenCalledWith(threadMetadata.threadId, "analyst@example.com");
  });

  it("creates a persistent thread before sending its first run", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [] });
    api.createAssistantThread.mockResolvedValue(threadMetadata);
    api.runAssistantThread.mockImplementation((_input, handlers) => {
      handlers.onEvent({ response: "Persisted answer", type: "completed" });
      return () => undefined;
    });

    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
        <QueryChatThread contextTruncated={false} />
      </QueryChatRuntimeProvider>,
    );

    await user.type(screen.getByRole("textbox", { name: "Message input" }), "Persist this question");
    await user.click(screen.getByRole("button", { name: "Send message" }));

    expect(await screen.findByText("Persisted answer")).toBeTruthy();
    expect(api.createAssistantThread).toHaveBeenCalledWith("analyst@example.com");
    await waitFor(() => expect(api.runAssistantThread).toHaveBeenCalledTimes(1));
    const run = api.runAssistantThread.mock.calls[0]?.[0];
    expect(run).toMatchObject({
      files: [],
      parentMessageId: undefined,
      prompt: "Persist this question",
      threadId: threadMetadata.threadId,
      userEmail: "analyst@example.com",
    });
    expect(run.assistantMessageId).toBe(run.requestId);
    expect(run.userMessageId).toEqual(expect.any(String));
    expect(api.queryModel).not.toHaveBeenCalled();
  });

  it("keeps a newly created thread when the initial list resolves late", async () => {
    const user = userEvent.setup();
    const list = deferred<{ nextCursor: null; threads: [] }>();
    api.listAssistantThreads.mockReturnValue(list.promise);
    api.createAssistantThread.mockResolvedValue(threadMetadata);
    api.runAssistantThread.mockImplementation((_input, handlers) => {
      handlers.onEvent({ response: "Persisted answer", type: "completed" });
      return () => undefined;
    });

    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
        <MemoryRouter>
          <AssistantWorkspaceSidebar activeHomeSection="assistant" email="analyst@example.com" tools={[]} />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    await user.type(screen.getByRole("textbox", { name: "Message input" }), "Persist this question");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(api.runAssistantThread).toHaveBeenCalledOnce());

    list.resolve({ nextCursor: null, threads: [] });

    expect(await screen.findByRole("button", { name: threadMetadata.title })).toBeTruthy();
  });

  it("reuses the original user message when retrying a persisted answer", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [] });
    api.createAssistantThread.mockResolvedValue(threadMetadata);
    api.runAssistantThread.mockImplementation((_input, handlers) => {
      handlers.onEvent({ response: "Persisted answer", type: "completed" });
      return () => undefined;
    });

    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
        <QueryChatThread contextTruncated={false} />
      </QueryChatRuntimeProvider>,
    );

    await user.type(screen.getByRole("textbox", { name: "Message input" }), "Persist this question");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByText("Persisted answer");
    await user.click(screen.getByRole("button", { name: "Retry response" }));

    await waitFor(() => expect(api.runAssistantThread).toHaveBeenCalledTimes(2));
    const firstRun = api.runAssistantThread.mock.calls[0]?.[0];
    const retryRun = api.runAssistantThread.mock.calls[1]?.[0];
    expect(retryRun.userMessageId).toBe(firstRun.userMessageId);
    expect(retryRun.assistantMessageId).not.toBe(firstRun.assistantMessageId);
    expect(retryRun.requestId).toBe(retryRun.assistantMessageId);
  });

  it("shows a load failure and allows the thread to be selected again", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [threadMetadata] });
    api.getAssistantThread.mockRejectedValueOnce(new Error("network unavailable"));
    api.getAssistantThread.mockResolvedValueOnce(threadDetail);

    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
        <MemoryRouter>
          <AssistantWorkspaceSidebar activeHomeSection="assistant" email="analyst@example.com" tools={[]} />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    const threadButton = await screen.findByRole("button", { name: threadMetadata.title });
    await user.click(threadButton);
    expect(await screen.findByText("Couldn’t load this chat. Please select it again.")).toBeTruthy();

    await user.click(threadButton);
    expect(await screen.findByText("Earlier assistant answer")).toBeTruthy();
    expect(api.getAssistantThread).toHaveBeenCalledTimes(2);
  });

  it("deletes a previous chat from its options menu and clears the selected transcript", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [threadMetadata] });
    api.getAssistantThread.mockResolvedValue(threadDetail);
    api.deleteAssistantThread.mockResolvedValue(undefined);

    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
        <MemoryRouter>
          <AssistantWorkspaceSidebar activeHomeSection="assistant" email="analyst@example.com" tools={[]} />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    await user.click(await screen.findByRole("button", { name: threadMetadata.title }));
    expect(await screen.findByText("Earlier assistant answer")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: `Chat options for ${threadMetadata.title}` }));
    expect(screen.getByRole("menuitem", { name: "Archive chat" }).getAttribute("data-disabled"))
      .not.toBeNull();
    await user.click(screen.getByRole("menuitem", { name: "Delete chat" }));

    await waitFor(() => expect(api.deleteAssistantThread).toHaveBeenCalledWith(
      threadMetadata.threadId,
      "analyst@example.com",
    ));
    await waitFor(() => expect(screen.queryByRole("button", { name: threadMetadata.title })).toBeNull());
    expect(screen.queryByText("Earlier assistant answer")).toBeNull();
  });

  it("keeps the thread and reports a sanitized error when deletion fails", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [threadMetadata] });
    api.deleteAssistantThread.mockRejectedValue(new Error("network unavailable"));

    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
        <MemoryRouter>
          <AssistantWorkspaceSidebar activeHomeSection="assistant" email="analyst@example.com" tools={[]} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    await user.click(await screen.findByRole("button", { name: `Chat options for ${threadMetadata.title}` }));
    await user.click(screen.getByRole("menuitem", { name: "Delete chat" }));

    expect(await screen.findByText("Couldn’t delete chat. Please try again.")).toBeTruthy();
    expect(screen.getByRole("button", { name: threadMetadata.title })).toBeTruthy();
  });
});

const threadMetadata = {
  createdAt: "2026-09-21 10:00:00",
  lastMessageAt: "2026-09-21 10:01:00",
  status: "regular" as const,
  threadId: "thread-1",
  title: "Previous chat",
  updatedAt: "2026-09-21 10:01:00",
};

const threadDetail = {
  ...threadMetadata,
  messages: [
    {
      completedAt: "2026-09-21 10:00:01",
      content: "Earlier user question",
      createdAt: "2026-09-21 10:00:00",
      messageId: "message-1",
      model: null,
      parentMessageId: null,
      role: "user" as const,
      sequence: 1,
      status: "completed" as const,
      threadId: "thread-1",
    },
    {
      completedAt: "2026-09-21 10:01:01",
      content: "Earlier assistant answer",
      createdAt: "2026-09-21 10:01:00",
      messageId: "message-2",
      model: "gpt-5.5",
      parentMessageId: "message-1",
      role: "assistant" as const,
      sequence: 2,
      status: "completed" as const,
      threadId: "thread-1",
    },
  ],
};

function deferred<T>() {
  let settle: ((value: T) => void) | undefined;
  const promise = new Promise<T>((resolvePromise) => {
    settle = resolvePromise;
  });
  return {
    promise,
    resolve(value: T) {
      settle?.(value);
    },
  };
}

// @vitest-environment happy-dom

import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, beforeAll, afterEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";
import { useAgentRuntime } from "@/components/chat/AgentRuntime";
import { acceptDealDeletion } from "@/data/dealDeletion/lifecycle";
import { QueryChatRuntimeProvider } from "@/components/chat/QueryChatRuntimeProvider";
import { QueryChatThread } from "@/components/chat/QueryChat";
import { AssistantWorkspaceSidebar } from "@/components/hub/sidebar/AssistantWorkspaceSidebar";

const { api } = vi.hoisted(() => ({
  api: {
    createAssistantThread: vi.fn(),
    deleteAssistantThread: vi.fn(),
    getAssistantThread: vi.fn(),
    listAssistantThreads: vi.fn(),
    listDeals: vi.fn(),
    listDealDocuments: vi.fn(),
    runAssistantThread: vi.fn(),
  },
}));

vi.mock("@quarry/runtime", () => ({ runtime: { api } }));

// Happy DOM rejects canceled Web Animation promises; exercise Motion’s JS fallback.
const animationDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
beforeAll(() => { Reflect.deleteProperty(Element.prototype, "animate"); });
afterAll(() => {
  if (animationDescriptor) Object.defineProperty(Element.prototype, "animate", animationDescriptor);
});

afterEach(() => {
  cleanup();
  api.createAssistantThread.mockReset();
  api.deleteAssistantThread.mockReset();
  api.getAssistantThread.mockReset();
  api.listAssistantThreads.mockReset();
  api.listDeals.mockReset();
  api.listDealDocuments.mockReset();
  api.runAssistantThread.mockReset();
});

describe("QueryChatRuntimeProvider", () => {
  it("keeps a signed-out draft and attachments without starting a temporary chat", async () => {
    const user = userEvent.setup();
    render(
      <QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="">
        <QueryChatThread contextTruncated={false} />
      </QueryChatRuntimeProvider>,
    );
    const input = screen.getByRole("textbox", { name: "Message input" });
    await user.type(input, "Keep this question");
    await user.upload(screen.getByLabelText("Choose attachments"), new File(["Notes"], "notes.txt", { type: "text/plain" }));
    await user.click(screen.getByRole("button", { name: "Send message" }));
    expect(await screen.findByText("Sign in to save this chat before sending a message.")).toBeTruthy();
    expect((input as HTMLTextAreaElement).value).toBe("Keep this question");
    expect(screen.getByRole("button", { name: "Remove notes.txt" })).toBeTruthy();
    expect(api.createAssistantThread).not.toHaveBeenCalled();
    expect(api.runAssistantThread).not.toHaveBeenCalled();
  });

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
      <QueryChatRuntimeProvider
        onContextTruncated={vi.fn()}
        userEmail="analyst@example.com"
      >
        <MemoryRouter>
          <AssistantWorkspaceSidebar
            activeHomeSection="assistant"
            email="analyst@example.com"
            tools={[]}
          />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    await user.click(
      await screen.findByRole("button", { name: threadMetadata.title }),
    );

    expect(await screen.findByTestId("thread-history-skeleton")).toBeTruthy();
    resolveThread?.(threadDetail);
    const userMessage = await screen.findByText("Earlier user question");
    const assistantMessage = screen.getByText("Earlier assistant answer");
    expect(userMessage.closest('[data-role="user"]')).toBeTruthy();
    expect(assistantMessage.closest('[data-role="assistant"]')).toBeTruthy();
    expect(api.getAssistantThread).toHaveBeenCalledWith(
      threadMetadata.threadId,
    );
  });

  it("retains the prompt and attachments when thread creation fails", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [] });
    api.createAssistantThread.mockRejectedValueOnce(new Error("unavailable"));
    render(<QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com">
      <QueryChatThread contextTruncated={false} />
    </QueryChatRuntimeProvider>);
    const file = new File(["Notes"], "retained.txt", { type: "text/plain" });
    await user.upload(screen.getByLabelText("Choose attachments"), file);
    const input = screen.getByRole("textbox", { name: "Message input" });
    await user.type(input, "Keep my draft");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Your message and files are still here");
    expect((input as HTMLTextAreaElement).value).toBe("Keep my draft");
    expect(screen.getByRole("button", { name: "Remove retained.txt" })).toBeTruthy();
    expect(document.activeElement).toBe(input);
    expect(api.runAssistantThread).not.toHaveBeenCalled();
  });

  it("creates a persistent thread before sending its first run", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({
      nextCursor: null,
      threads: [],
    });
    api.createAssistantThread.mockResolvedValue(threadMetadata);
    api.runAssistantThread.mockImplementation((_input, handlers) => {
      handlers.onEvent({ response: "Persisted answer", type: "completed" });
      return () => undefined;
    });

    render(
      <QueryChatRuntimeProvider
        onContextTruncated={vi.fn()}
        userEmail="analyst@example.com"
      >
        <QueryChatThread contextTruncated={false} />
      </QueryChatRuntimeProvider>,
    );

    const attachment = new File(["Persisted file"], "persisted.txt", { type: "text/plain" });
    await user.upload(screen.getByLabelText("Choose attachments"), attachment);
    await user.click(screen.getByRole("combobox", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /GPT-5.5/ }));
    await user.type(
      screen.getByRole("textbox", { name: "Message input" }),
      "Persist this question",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));

    expect(await screen.findByText("Persisted answer")).toBeTruthy();
    expect(api.createAssistantThread).toHaveBeenCalledWith(undefined, null);
    await waitFor(() =>
      expect(api.runAssistantThread).toHaveBeenCalledTimes(1),
    );
    const run = api.runAssistantThread.mock.calls[0]?.[0];
    expect(run).toMatchObject({
      files: [attachment],
      model: "gpt-5.5",
      parentMessageId: undefined,
      prompt: "Persist this question",
      threadId: threadMetadata.threadId,
    });
    expect(run.assistantMessageId).toBe(run.requestId);
    expect(run.userMessageId).toEqual(expect.any(String));
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
      <QueryChatRuntimeProvider
        onContextTruncated={vi.fn()}
        userEmail="analyst@example.com"
      >
        <MemoryRouter>
          <AssistantWorkspaceSidebar
            activeHomeSection="assistant"
            email="analyst@example.com"
            tools={[]}
          />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    const attachment = new File(["Persisted file"], "persisted.txt", { type: "text/plain" });
    await user.upload(screen.getByLabelText("Choose attachments"), attachment);
    await user.click(screen.getByRole("combobox", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /GPT-5.5/ }));
    await user.type(
      screen.getByRole("textbox", { name: "Message input" }),
      "Persist this question",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(api.runAssistantThread).toHaveBeenCalledOnce());

    list.resolve({ nextCursor: null, threads: [] });

    expect(
      await screen.findByRole("button", { name: threadMetadata.title }),
    ).toBeTruthy();
  });

  it("reuses the original user message when retrying a persisted answer", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({
      nextCursor: null,
      threads: [],
    });
    api.createAssistantThread.mockResolvedValue(threadMetadata);
    api.runAssistantThread.mockImplementation((_input, handlers) => {
      handlers.onEvent({ response: "Persisted answer", type: "completed" });
      return () => undefined;
    });

    render(
      <QueryChatRuntimeProvider
        onContextTruncated={vi.fn()}
        userEmail="analyst@example.com"
      >
        <QueryChatThread contextTruncated={false} />
      </QueryChatRuntimeProvider>,
    );

    const attachment = new File(["Persisted file"], "persisted.txt", { type: "text/plain" });
    await user.upload(screen.getByLabelText("Choose attachments"), attachment);
    await user.click(screen.getByRole("combobox", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /GPT-5.5/ }));
    await user.type(
      screen.getByRole("textbox", { name: "Message input" }),
      "Persist this question",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await screen.findByText("Persisted answer");
    await user.click(screen.getByRole("button", { name: "Retry response" }));

    await waitFor(() =>
      expect(api.runAssistantThread).toHaveBeenCalledTimes(2),
    );
    const firstRun = api.runAssistantThread.mock.calls[0]?.[0];
    const retryRun = api.runAssistantThread.mock.calls[1]?.[0];
    expect(retryRun.userMessageId).toBe(firstRun.userMessageId);
    expect(retryRun.assistantMessageId).not.toBe(firstRun.assistantMessageId);
    expect(retryRun.requestId).toBe(retryRun.assistantMessageId);
  });

  it("shows a load failure and allows the thread to be selected again", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({
      nextCursor: null,
      threads: [threadMetadata],
    });
    api.getAssistantThread.mockRejectedValueOnce(
      new Error("network unavailable"),
    );
    api.getAssistantThread.mockResolvedValueOnce(threadDetail);

    render(
      <QueryChatRuntimeProvider
        onContextTruncated={vi.fn()}
        userEmail="analyst@example.com"
      >
        <MemoryRouter>
          <AssistantWorkspaceSidebar
            activeHomeSection="assistant"
            email="analyst@example.com"
            tools={[]}
          />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    const threadButton = await screen.findByRole("button", {
      name: threadMetadata.title,
    });
    await user.click(threadButton);
    expect(
      await screen.findByText(
        "Couldn’t load this chat. Please select it again.",
      ),
    ).toBeTruthy();

    await user.click(threadButton);
    expect(await screen.findByText("Earlier assistant answer")).toBeTruthy();
    expect(api.getAssistantThread).toHaveBeenCalledTimes(2);
  });

  it("deletes a previous chat from its options menu and clears the selected transcript", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({
      nextCursor: null,
      threads: [threadMetadata],
    });
    api.getAssistantThread.mockResolvedValue(threadDetail);
    api.deleteAssistantThread.mockResolvedValue(undefined);

    render(
      <QueryChatRuntimeProvider
        onContextTruncated={vi.fn()}
        userEmail="analyst@example.com"
      >
        <MemoryRouter>
          <AssistantWorkspaceSidebar
            activeHomeSection="assistant"
            email="analyst@example.com"
            tools={[]}
          />
          <QueryChatThread contextTruncated={false} />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    await user.click(
      await screen.findByRole("button", { name: threadMetadata.title }),
    );
    expect(await screen.findByText("Earlier assistant answer")).toBeTruthy();

    await user.click(
      screen.getByRole("button", {
        name: `Chat options for ${threadMetadata.title}`,
      }),
    );
    expect(
      screen
        .getByRole("menuitem", { name: "Archive chat" })
        .getAttribute("data-disabled"),
    ).not.toBeNull();
    await user.click(screen.getByRole("menuitem", { name: "Delete chat" }));

    await waitFor(() =>
      expect(api.deleteAssistantThread).toHaveBeenCalledWith(
        threadMetadata.threadId,
      ),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: threadMetadata.title }),
      ).toBeNull(),
    );
    expect(screen.queryByText("Earlier assistant answer")).toBeNull();
  });

  it("keeps the thread and reports a sanitized error when deletion fails", async () => {
    const user = userEvent.setup();
    api.listAssistantThreads.mockResolvedValue({
      nextCursor: null,
      threads: [threadMetadata],
    });
    api.deleteAssistantThread.mockRejectedValue(
      new Error("network unavailable"),
    );

    render(
      <QueryChatRuntimeProvider
        onContextTruncated={vi.fn()}
        userEmail="analyst@example.com"
      >
        <MemoryRouter>
          <AssistantWorkspaceSidebar
            activeHomeSection="assistant"
            email="analyst@example.com"
            tools={[]}
          />
        </MemoryRouter>
      </QueryChatRuntimeProvider>,
    );

    await user.click(
      await screen.findByRole("button", {
        name: `Chat options for ${threadMetadata.title}`,
      }),
    );
    await user.click(screen.getByRole("menuitem", { name: "Delete chat" }));

    expect(
      await screen.findByText("Couldn’t delete chat. Please try again."),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: threadMetadata.title }),
    ).toBeTruthy();
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

it("keys drafts, pending lists, and new threads by the active deal", async () => {
  const user = userEvent.setup();
  const pending = deferred<{ nextCursor: null; threads: [] }>();
  api.listAssistantThreads
    .mockReturnValueOnce(pending.promise)
    .mockResolvedValue({ nextCursor: null, threads: [] });
  api.createAssistantThread.mockResolvedValue({
    ...threadMetadata,
    dealId: "DEAL-B",
  });
  api.runAssistantThread.mockImplementation((_input, handlers) => {
    queueMicrotask(() =>
      handlers.onEvent({ type: "completed", response: "Answer" }),
    );
    return () => {};
  });
  const view = (dealId: string) => (
    <MemoryRouter>
      <QueryChatRuntimeProvider
        dealId={dealId}
        scope="deal"
        onContextTruncated={vi.fn()}
        userEmail="owner@example.com"
      >
        <QueryChatThread contextTruncated={false} />
      </QueryChatRuntimeProvider>
    </MemoryRouter>
  );
  const { rerender } = render(view("DEAL-A"));
  await user.type(
    screen.getByRole("textbox", { name: "Message input" }),
    "Draft for A",
  );
  rerender(view("DEAL-B"));
  expect(
    (
      screen.getByRole("textbox", {
        name: "Message input",
      }) as HTMLTextAreaElement
    ).value,
  ).toBe("");
  pending.resolve({ nextCursor: null, threads: [] });
  await waitFor(() =>
    expect(api.listAssistantThreads).toHaveBeenLastCalledWith({
      scope: "deal",
      dealId: "DEAL-B",
    }),
  );
  await user.type(
    screen.getByRole("textbox", { name: "Message input" }),
    "Question for B",
  );
  await user.click(screen.getByRole("button", { name: "Send message" }));
  await waitFor(() =>
    expect(api.createAssistantThread).toHaveBeenCalledWith(undefined, "DEAL-B"),
  );
});

it("snapshots cross-deal mentions with uploads for retry and preserves the next draft", async () => {
  const user = userEvent.setup();
  api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [] });
  api.createAssistantThread.mockResolvedValue(threadMetadata);
  api.listDeals.mockResolvedValue([{ dealId: "DEAL-A", dealName: "Alpha" }, { dealId: "DEAL-B", dealName: "Beta" }]);
  api.listDealDocuments.mockImplementation(async (id: string) => [{ fileId: `file-${id}`, displayName: id === "DEAL-A" ? "Architecture.pdf" : "Budget.xlsx" }]);
  api.runAssistantThread.mockImplementation((_input, handlers) => {
    handlers.onEvent({ type: "failed", error: "test failure" });
    return () => undefined;
  });
  render(<QueryChatRuntimeProvider onContextTruncated={vi.fn()} userEmail="analyst@example.com"><QueryChatThread contextTruncated={false} /></QueryChatRuntimeProvider>);
  const input = screen.getByRole("textbox", { name: "Message input" });
  await user.type(input, "Review @arch");
  await user.click(await screen.findByRole("option", { name: /Architecture/ }));
  await user.type(input, "with @budget");
  await user.click(await screen.findByRole("option", { name: /Budget/ }));
  await user.upload(screen.getByLabelText("Choose attachments"), new File(["Notes"], "manual.txt", { type: "text/plain" }));
  await user.click(screen.getByRole("button", { name: "Send message" }));
  await screen.findByRole("button", { name: "Try again" });
  const original = api.runAssistantThread.mock.calls[0][0];
  expect(original.fileReferences).toEqual([
    { dealId: "DEAL-A", fileId: "file-DEAL-A", name: "Architecture.pdf" },
    { dealId: "DEAL-B", fileId: "file-DEAL-B", name: "Budget.xlsx" },
  ]);
  expect(original.files[0].name).toBe("manual.txt");
  expect(original.fileScopeDealId).toBeUndefined();
  expect(screen.queryByRole("button", { name: "Remove reference Architecture.pdf" })).toBeNull();
  await user.type(input, "Next question @budget");
  await user.click(await screen.findByRole("option", { name: /Budget/ }));
  await user.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(api.runAssistantThread).toHaveBeenCalledTimes(2));
  expect(api.runAssistantThread.mock.calls[1][0].fileReferences).toEqual(original.fileReferences);
  expect((input as HTMLTextAreaElement).value).toBe("Next question ");
  expect(screen.getByRole("button", { name: "Remove reference Budget.xlsx" })).toBeTruthy();
});

it("sends the active data room scope with references", async () => {
  const user = userEvent.setup();
  api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [] });
  api.createAssistantThread.mockResolvedValue({ ...threadMetadata, dealId: "DEAL-A" });
  api.listDealDocuments.mockResolvedValue([{ fileId: "file-a", displayName: "Architecture.pdf" }]);
  api.runAssistantThread.mockImplementation((_input, handlers) => { handlers.onEvent({ type: "completed", response: "Done" }); return () => undefined; });
  render(<QueryChatRuntimeProvider scope="deal" dealId="DEAL-A" onContextTruncated={vi.fn()} userEmail="analyst@example.com"><QueryChatThread contextTruncated={false} /></QueryChatRuntimeProvider>);
  await user.type(screen.getByRole("textbox", { name: "Message input" }), "Review @");
  await user.click(await screen.findByRole("option", { name: /Architecture/ }));
  await user.click(screen.getByRole("button", { name: "Send message" }));
  await waitFor(() => expect(api.runAssistantThread).toHaveBeenCalledOnce());
  expect(api.runAssistantThread.mock.calls[0][0]).toMatchObject({ fileScopeDealId: "DEAL-A", fileReferences: [{ dealId: "DEAL-A", fileId: "file-a", name: "Architecture.pdf" }] });
  expect(api.listDeals).not.toHaveBeenCalled();
});

it("clears a deleted selection and ignores its pending history response", async () => {
  const dealId = "DEAL-RUNTIME-HISTORY";
  const thread = { ...threadMetadata, dealId };
  const history = deferred<typeof threadDetail>();
  api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [thread] });
  api.getAssistantThread.mockReturnValue(history.promise);
  const { result } = renderHook(() => useAgentRuntime({ api, onContextTruncated: vi.fn(), userEmail: "analyst@example.com" }));
  await waitFor(() => expect(result.current.state.threads).toHaveLength(1));
  let loading: Promise<void> | undefined;
  act(() => { loading = result.current.selectThread(thread.threadId); });
  expect(result.current.state.isThreadLoading).toBe(true);
  act(() => result.current.setDraft("Private draft"));
  await act(async () => { await acceptDealDeletion(dealId, async () => null); });
  expect(result.current.state).toMatchObject({ selectedThreadId: null, messages: [], draft: "", draftFiles: [], draftReferences: [], threads: [], isThreadLoading: false });
  await act(async () => { history.resolve(threadDetail); await loading; });
  expect(result.current.state.messages).toEqual([]);
  expect(result.current.state.selectedThreadId).toBeNull();
});

it("cancels a running deleted-deal stream through the store subscription", async () => {
  const dealId = "DEAL-RUNTIME-STREAM";
  const cancel = vi.fn();
  api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [] });
  api.createAssistantThread.mockResolvedValue({ ...threadMetadata, dealId });
  api.runAssistantThread.mockReturnValue(cancel);
  const { result } = renderHook(() => useAgentRuntime({ api, onContextTruncated: vi.fn(), userEmail: "analyst@example.com", scope: "deal", dealId }));
  act(() => result.current.send("Private question"));
  await waitFor(() => expect(result.current.state.isRunning).toBe(true));
  await act(async () => { await acceptDealDeletion(dealId, async () => null); });
  await waitFor(() => expect(cancel).toHaveBeenCalledTimes(1));
  expect(result.current.state).toMatchObject({ selectedThreadId: null, messages: [], draft: "", isRunning: false, isPreparingThread: false });
});

it("does not start a run when thread creation resolves after deal deletion", async () => {
  const dealId = "DEAL-RUNTIME-CREATION";
  const creation = deferred<typeof threadMetadata & { dealId: string }>();
  api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [] });
  api.createAssistantThread.mockReturnValue(creation.promise);
  const { result } = renderHook(() => useAgentRuntime({ api, onContextTruncated: vi.fn(), userEmail: "analyst@example.com", scope: "deal", dealId }));
  act(() => result.current.send("Private question"));
  await act(async () => { await acceptDealDeletion(dealId, async () => null); });
  await act(async () => { creation.resolve({ ...threadMetadata, dealId }); await creation.promise; });
  expect(api.runAssistantThread).not.toHaveBeenCalled();
  expect(result.current.state.messages).toEqual([]);
  expect(result.current.state.selectedThreadId).toBeNull();
});

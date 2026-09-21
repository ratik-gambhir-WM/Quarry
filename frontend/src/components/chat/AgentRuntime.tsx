import type { ReactNode } from "react";
import { createContext, useCallback, useContext, useEffect, useReducer, useRef } from "react";
import { runtime } from "@quarry/runtime";
import type {
  AssistantMessage as PersistedAssistantMessage,
  AssistantThread,
  QuarryApi,
} from "../../contracts/quarryApi";
import type { AssistantMessage, Message, UserMessage } from "./chatModel";
import { createModelAdapter } from "./threadModelAdapter";

export type ThreadRuntimeState = {
  copiedMessageId: string | null;
  draft: string;
  isPreparingThread: boolean;
  isRunning: boolean;
  isThreadListLoading: boolean;
  isThreadLoading: boolean;
  messages: readonly Message[];
  selectedThreadId: string | null;
  threadListError: boolean;
  threads: readonly AssistantThread[];
};

type AgentRuntimeOptions = {
  api: Pick<QuarryApi, "queryModel"> & Partial<Pick<
    QuarryApi,
    "createAssistantThread" | "getAssistantThread" | "listAssistantThreads" | "renameAssistantThread" | "runAssistantThread"
  >>;
  onContextTruncated: (truncated: boolean) => void;
  userEmail: string;
};

type AgentRuntimeValue = {
  copy(messageId: string): Promise<void>;
  newChat(): void;
  retry(messageId: string): void;
  selectThread(threadId: string): Promise<void>;
  send(prompt?: string): void;
  setDraft(draft: string): void;
  state: ThreadRuntimeState;
  stop(): void;
};

const emptyState: ThreadRuntimeState = {
  copiedMessageId: null,
  draft: "",
  isPreparingThread: false,
  isRunning: false,
  isThreadListLoading: false,
  isThreadLoading: false,
  messages: [],
  selectedThreadId: null,
  threadListError: false,
  threads: [],
};

export function useAgentRuntime({ api, onContextTruncated, userEmail }: AgentRuntimeOptions): AgentRuntimeValue {
  const [state, dispatch] = useReducer(
    (_current: ThreadRuntimeState, next: ThreadRuntimeState) => next,
    userEmail,
    (email) => ({ ...emptyState, isThreadListLoading: Boolean(email) }),
  );
  const stateRef = useRef(state);
  const abortControllerRef = useRef<AbortController | null>(null);
  const loadTokenRef = useRef(0);
  const mountedRef = useRef(true);
  stateRef.current = state;

  const update = useCallback((change: Partial<ThreadRuntimeState>) => {
    if (!mountedRef.current) return;
    const next = { ...stateRef.current, ...change };
    stateRef.current = next;
    dispatch(next);
  }, []);

  const replaceAssistantMessage = useCallback((
    id: string,
    text: string | undefined,
    status: AssistantMessage["status"],
  ) => {
    update({
      messages: stateRef.current.messages.map((message) => (
        message.role === "assistant" && message.id === id
          ? {
              ...message,
              ...(text === undefined ? {} : { content: [{ text, type: "text" as const }] }),
              status,
            }
          : message
      )),
    });
  }, [update]);

  const startRun = useCallback(async (text: string, threadId: string) => {
    const userMessage = userMessageFor(text);
    const assistantMessage = assistantMessageFor();
    const messages = [...stateRef.current.messages, userMessage, assistantMessage];
    const controller = new AbortController();
    abortControllerRef.current = controller;
    onContextTruncated(false);
    update({ draft: "", isRunning: true, messages });

    try {
      const adapter = createModelAdapter(api, { onContextTruncated, userEmail: userEmail || undefined });
      for await (const result of adapter.run({
        abortSignal: controller.signal,
        assistantMessageId: assistantMessage.id,
        messages,
        threadId: threadId || undefined,
      })) {
        if (controller.signal.aborted) return;
        replaceAssistantMessage(assistantMessage.id, result.content[0].text, { type: "running" });
      }
      if (!controller.signal.aborted) {
        replaceAssistantMessage(assistantMessage.id, undefined, { reason: "stop", type: "complete" });
      }
    } catch {
      if (!controller.signal.aborted) {
        replaceAssistantMessage(assistantMessage.id, undefined, { reason: "error", type: "incomplete" });
      }
    } finally {
      if (abortControllerRef.current === controller) abortControllerRef.current = null;
      update({ isRunning: false });
    }
  }, [api, onContextTruncated, replaceAssistantMessage, update, userEmail]);

  const send = useCallback((prompt = stateRef.current.draft) => {
    const text = prompt.trim();
    const current = stateRef.current;
    if (!text || current.isPreparingThread || current.isRunning || current.isThreadLoading) return;
    if (!userEmail) {
      void startRun(text, "");
      return;
    }

    update({ isPreparingThread: true });
    const selectedThreadId = stateRef.current.selectedThreadId;
    const createAssistantThread = api.createAssistantThread;
    const threadId = selectedThreadId
      ? Promise.resolve(selectedThreadId)
      : createAssistantThread
        ? createAssistantThread(userEmail).then((thread) => {
            update({
              selectedThreadId: thread.threadId,
              threads: [thread, ...stateRef.current.threads],
            });
            return thread.threadId;
          })
        : Promise.resolve(null);
    void threadId
      .then((id) => { if (id) void startRun(text, id); })
      .catch(() => undefined)
      .finally(() => update({ isPreparingThread: false }));
  }, [api.createAssistantThread, startRun, update, userEmail]);

  const setDraft = useCallback((draft: string) => update({ draft }), [update]);

  const newChat = useCallback(() => {
    const current = stateRef.current;
    if (current.isPreparingThread || current.isRunning) return;
    loadTokenRef.current += 1;
    onContextTruncated(false);
    update({
      copiedMessageId: null,
      draft: "",
      isThreadLoading: false,
      messages: [],
      selectedThreadId: null,
    });
  }, [onContextTruncated, update]);

  const selectThread = useCallback(async (threadId: string) => {
    const current = stateRef.current;
    const getAssistantThread = api.getAssistantThread;
    if (
      !userEmail
      || current.isPreparingThread
      || current.isRunning
      || threadId === current.selectedThreadId
      || !getAssistantThread
    ) return;

    const token = ++loadTokenRef.current;
    onContextTruncated(false);
    update({
      copiedMessageId: null,
      draft: "",
      isThreadLoading: true,
      messages: [],
      selectedThreadId: threadId,
    });
    try {
      const thread = await getAssistantThread(threadId, userEmail);
      if (token === loadTokenRef.current) {
        update({ isThreadLoading: false, messages: activeMessageBranch(thread.messages) });
      }
    } catch {
      if (token === loadTokenRef.current) update({ isThreadLoading: false });
    }
  }, [api.getAssistantThread, onContextTruncated, update, userEmail]);

  const stop = useCallback(() => {
    const controller = abortControllerRef.current;
    if (!controller) return;
    controller.abort();
    const runningMessage = [...stateRef.current.messages]
      .reverse()
      .find((message): message is AssistantMessage => (
        message.role === "assistant" && message.status.type === "running"
      ));
    if (runningMessage) {
      replaceAssistantMessage(runningMessage.id, undefined, { reason: "cancelled", type: "incomplete" });
    }
  }, [replaceAssistantMessage]);

  const retry = useCallback((messageId: string) => {
    const current = stateRef.current;
    if (current.isRunning) return;
    const failedIndex = current.messages.findIndex((message) => message.id === messageId);
    const previous = current.messages[failedIndex - 1];
    if (failedIndex < 1 || previous?.role !== "user") return;
    update({ messages: current.messages.slice(0, failedIndex) });
    send(textContent(previous));
  }, [send, update]);

  const copy = useCallback(async (messageId: string) => {
    const message = stateRef.current.messages.find((candidate) => candidate.id === messageId);
    if (message?.role !== "assistant" || !navigator.clipboard) return;
    try {
      await navigator.clipboard.writeText(textContent(message));
      update({ copiedMessageId: messageId });
    } catch {
      // Clipboard access is browser-policy controlled. The response remains selectable if denied.
    }
  }, [update]);

  useEffect(() => {
    if (!userEmail || !api.listAssistantThreads) return;
    let active = true;
    void api.listAssistantThreads(userEmail)
      .then((page) => {
        if (active) update({ isThreadListLoading: false, threads: page.threads });
      })
      .catch(() => {
        if (active) update({ isThreadListLoading: false, threadListError: true });
      });
    return () => {
      active = false;
    };
  }, [api, update, userEmail]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      abortControllerRef.current?.abort();
    };
  }, []);

  return { copy, newChat, retry, selectThread, send, setDraft, state, stop };
}

const AgentRuntimeContext = createContext<AgentRuntimeValue | null>(null);

export function QueryChatRuntimeProvider({
  children,
  onContextTruncated,
  userEmail,
}: {
  children: ReactNode;
  onContextTruncated: (truncated: boolean) => void;
  userEmail: string;
}) {
  const agentRuntime = useAgentRuntime({ api: runtime.api, onContextTruncated, userEmail });
  return <AgentRuntimeContext.Provider value={agentRuntime}>{children}</AgentRuntimeContext.Provider>;
}

export function useAgentRuntimeContext() {
  const agentRuntime = useContext(AgentRuntimeContext);
  if (!agentRuntime) throw new Error("Chat controls must be rendered inside QueryChatRuntimeProvider.");
  return agentRuntime;
}

export function useThreadRuntimeState<T>(select: (state: ThreadRuntimeState) => T) {
  return select(useAgentRuntimeContext().state);
}

function activeMessageBranch(messages: readonly PersistedAssistantMessage[]): Message[] {
  if (messages.length === 0) return [];
  const byId = new Map(messages.map((message) => [message.messageId, message]));
  const branch: PersistedAssistantMessage[] = [];
  const lastMessage = messages[messages.length - 1];
  if (!lastMessage) return [];
  let current: PersistedAssistantMessage | undefined = lastMessage;
  while (current) {
    branch.push(current);
    current = current.parentMessageId ? byId.get(current.parentMessageId) : undefined;
  }
  return branch.reverse().map(fromAssistantMessage);
}

function fromAssistantMessage(message: PersistedAssistantMessage): Message {
  const content = [{ text: message.content, type: "text" as const }];
  if (message.role === "user") return { attachments: [], content, id: message.messageId, role: "user" };
  return {
    content,
    id: message.messageId,
    role: "assistant",
    status: message.status === "completed"
      ? { reason: "stop", type: "complete" }
      : message.status === "streaming"
        ? { type: "running" }
        : { reason: message.status === "cancelled" ? "cancelled" : "error", type: "incomplete" },
  };
}

function userMessageFor(text: string): UserMessage {
  return { attachments: [], content: [{ text, type: "text" }], id: createId(), role: "user" };
}

function assistantMessageFor(): AssistantMessage {
  return { content: [{ text: "", type: "text" }], id: createId(), role: "assistant", status: { type: "running" } };
}

function createId() {
  return globalThis.crypto?.randomUUID?.() ?? `chat-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function textContent(message: Message) {
  return message.content.map((part) => part.type === "text" ? part.text : "").join("");
}

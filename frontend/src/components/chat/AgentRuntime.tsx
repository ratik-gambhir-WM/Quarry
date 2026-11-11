import { createThreadRuntimeStore, type ThreadRuntimeState } from "./threadRuntimeStore";
import type { ReactNode } from "react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  useSyncExternalStore,
  useRef,
} from "react";
import { runtime } from "@quarry/runtime";
import type {
  AssistantMessage as PersistedAssistantMessage,
  AssistantThread,
  ChatFileReference,
  AssistantThreadListOptions,
  QuarryApi,
} from "../../contracts/quarryApi";
import { isChatModelValid, type AssistantMessage, type ChatModelSelection, type Message, type UserMessage } from "./chatModel";
import { addChatFiles } from "./chatAttachments";
import { createModelAdapter } from "./threadModelAdapter";

export type { ThreadRuntimeState } from "./threadRuntimeStore";

type AgentRuntimeOptions = {
  api: Pick<QuarryApi, "createAssistantThread" | "runAssistantThread"> &
    Partial<
      Pick<
        QuarryApi,
        | "deleteAssistantThread"
        | "getAssistantThread"
        | "listAssistantThreads"
        | "renameAssistantThread"
      >
    >;
  onContextTruncated: (truncated: boolean) => void;
  userEmail: string;
  scope?: "all" | "freeform" | "deal";
  dealId?: string;
  initialDraft?: string;
  initialFileReference?: ChatFileReference;
};

type AgentRuntimeValue = {
  loadMoreThreads(): Promise<void>;
  copy(messageId: string): Promise<void>;
  deleteThread(threadId: string): Promise<void>;
  newChat(): void;
  retry(messageId: string): void;
  selectThread(threadId: string): Promise<void>;
  send(prompt?: string): void;
  setDraft(draft: string): void;
  setReferences(references: readonly ChatFileReference[]): void;
  addReference(reference: ChatFileReference): void;
  addFiles(files: readonly File[]): void;
  removeFile(index: number): void;
  setModel(model: ChatModelSelection): void;
  state: ThreadRuntimeState;
  stop(): void;
};

const emptyState: ThreadRuntimeState = {
  nextCursor: null,
  scopeDealId: null,
  copiedMessageId: null,
  deletingThreadId: null,
  draft: "",
  draftReferences: [],
  draftFiles: [],
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

export function useAgentRuntime({
  api,
  onContextTruncated,
  userEmail,
  scope = "all",
  dealId,
  initialDraft = "",
  initialFileReference,
}: AgentRuntimeOptions): AgentRuntimeValue {
  const abortControllerRef = useRef<AbortController | null>(null);
  const loadTokenRef = useRef(0);
  const mountedRef = useRef(true);
  const [store] = useState(() => createThreadRuntimeStore({
    ...emptyState,
    draft: initialDraft,
    draftReferences: initialFileReference && scope === "deal" && initialFileReference.dealId === dealId
      ? [initialFileReference] : [],
    isThreadListLoading: Boolean(userEmail),
    scopeDealId: scope === "deal" ? (dealId ?? null) : null,
  }, () => {
    loadTokenRef.current++;
    abortControllerRef.current?.abort();
  }));
  const state = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);

  const update = useCallback((change: Partial<ThreadRuntimeState>) => {
    if (mountedRef.current) store.update(change);
  }, [store]);

  const replaceAssistantMessage = useCallback(
    (
      id: string,
      text: string | undefined,
      status: AssistantMessage["status"],
      error?: string,
    ) => {
      update({
        messages: store.current.messages.map((message) =>
          message.role === "assistant" && message.id === id
            ? {
                ...message,
                ...(text === undefined
                  ? {}
                  : { content: [{ text, type: "text" as const }] }),
                status,
                error,
              }
            : message,
        ),
      });
    },
    [store, update],
  );

  const startRun = useCallback(
    async (
      userMessage: UserMessage,
      threadId: string,
      reusesUserMessage = false,
      model?: string,
    ) => {
      if (!mountedRef.current) return;
      const assistantMessage = assistantMessageFor();
      const messages = reusesUserMessage
        ? [...store.current.messages, assistantMessage]
        : [...store.current.messages, userMessage, assistantMessage];
      const text = textContent(userMessage);
      const controller = new AbortController();
      abortControllerRef.current = controller;
      logChat("run.started", {
        assistantMessageId: assistantMessage.id,
        messageCount: messages.length,
        persisted: Boolean(threadId),
        promptChars: [...text].length,
        threadId: threadId || undefined,
      });
      onContextTruncated(false);
      update({
        ...(!reusesUserMessage ? { draft: "", draftFiles: [], draftReferences: [], attachmentError: null } : {}),
        isRunning: true,
        messages,
        sendError: null,
        threadLoadError: false,
      });

      try {
        const adapter = createModelAdapter(api, {
          onContextTruncated,
        });
        for await (const result of adapter.run({
          abortSignal: controller.signal,
          assistantMessageId: assistantMessage.id,
          messages,
          model,
          threadId: threadId || undefined,
        })) {
          if (controller.signal.aborted) return;
          replaceAssistantMessage(assistantMessage.id, result.content[0].text, {
            type: "running",
          });
        }
        if (!controller.signal.aborted) {
          replaceAssistantMessage(assistantMessage.id, undefined, {
            reason: "stop",
            type: "complete",
          });
          logChat("run.completed", {
            assistantMessageId: assistantMessage.id,
            threadId: threadId || undefined,
          });
        }
      } catch (error) {
        if (!controller.signal.aborted) {
          replaceAssistantMessage(assistantMessage.id, undefined, {
            reason: "error",
            type: "incomplete",
          }, userMessage.fileReferences?.length ? "Couldn’t complete this response with the selected files. Check access, file types and size limits, then retry or start a new message with different references." : undefined);
          logChat("run.failed", {
            assistantMessageId: assistantMessage.id,
            error: errorMessage(error),
            threadId: threadId || undefined,
          });
        }
      } finally {
        if (abortControllerRef.current === controller)
          abortControllerRef.current = null;
        update({ isRunning: false });
      }
    },
    [api, onContextTruncated, replaceAssistantMessage, store, update],
  );

  const send = useCallback(
    (prompt = store.current.draft) => {
      const text = prompt.trim();
      const current = store.current;
      if (
        !text ||
        !isChatModelValid(current.selectedModel) ||
        current.isPreparingThread ||
        current.isRunning ||
        current.isThreadLoading
      ) {
        logChat("send.ignored", {
          hasText: Boolean(text),
          isPreparingThread: current.isPreparingThread,
          isRunning: current.isRunning,
          isThreadLoading: current.isThreadLoading,
        });
        return;
      }
      logChat("send.requested", {
        persisted: Boolean(userEmail),
        promptChars: [...text].length,
      });
      const message = userMessageFor(text, current.draftFiles, current.draftReferences, current.scopeDealId);
      const model = current.selectedModel.type === "model" ? current.selectedModel.id.trim() : undefined;
      if (!userEmail) {
        update({ sendError: "Sign in to save this chat before sending a message." });
        return;
      }

      update({ isPreparingThread: true, sendError: null });
      const selectedThreadId = store.current.selectedThreadId;
      const createAssistantThread = api.createAssistantThread;
      const threadId = selectedThreadId
        ? Promise.resolve(selectedThreadId)
        : createAssistantThread(undefined, scope === "deal" ? dealId : null).then((thread) => {
            if (!mountedRef.current) return null;
            update({
              selectedThreadId: thread.threadId,
              threads: [thread, ...store.current.threads],
            });
            return store.current.selectedThreadId === thread.threadId ? thread.threadId : null;
          });
      void threadId
        .then((id) => {
          if (id) void startRun(message, id, false, model);
          else update({ sendError: "Couldn’t start this chat. Your message and files are still here. Try sending again." });
        })
        .catch((error) => {
          logChat("thread.create_failed", { error: errorMessage(error) });
          update({ sendError: "Couldn’t start this chat. Your message and files are still here. Try sending again." });
        })
        .finally(() => update({ isPreparingThread: false }));
    },
    [api.createAssistantThread, startRun, store, update, userEmail, scope, dealId],
  );

  const setReferences = useCallback((draftReferences: readonly ChatFileReference[]) => {
    const current = store.current;
    if (current.isPreparingThread || current.isThreadLoading) return;
    if (draftReferences.length + current.draftFiles.length > 20) {
      update({ attachmentError: "Choose up to 20 uploads and file references in total." });
      return;
    }
    update({ draftReferences, attachmentError: null });
  }, [store, update]);

  const addReference = useCallback((reference: ChatFileReference) => {
    const current = store.current;
    if (current.scopeDealId !== reference.dealId || current.draftReferences.some(
      (file) => file.dealId === reference.dealId && file.fileId === reference.fileId,
    )) return;
    setReferences([...current.draftReferences, reference]);
  }, [setReferences]);

  const setDraft = useCallback((draft: string) => update({ draft }), [update]);

  const addFiles = useCallback((incoming: readonly File[]) => {
    if (store.current.isPreparingThread || store.current.isThreadLoading) return;
    if (store.current.draftFiles.length + store.current.draftReferences.length + incoming.length > 20) {
      update({ attachmentError: "Choose up to 20 uploads and file references in total." });
      return;
    }
    const result = addChatFiles(store.current.draftFiles, incoming);
    update({ draftFiles: result.files, attachmentError: result.error });
  }, [store, update]);

  const removeFile = useCallback((index: number) => {
    if (store.current.isPreparingThread || store.current.isThreadLoading) return;
    update({
      draftFiles: store.current.draftFiles.filter((_, fileIndex) => fileIndex !== index),
      attachmentError: null,
    });
  }, [store, update]);

  const setModel = useCallback((selectedModel: ChatModelSelection) => {
    if (store.current.isPreparingThread || store.current.isThreadLoading) return;
    update({ selectedModel });
  }, [store, update]);

  const deleteThread = useCallback(
    async (threadId: string) => {
      const current = store.current;
      const deleteAssistantThread = api.deleteAssistantThread;
      if (
        !userEmail ||
        !deleteAssistantThread ||
        current.isPreparingThread ||
        current.isRunning ||
        current.deletingThreadId
      )
        return;

      update({ deletingThreadId: threadId, threadDeleteError: false });
      try {
        await deleteAssistantThread(threadId);
        const wasSelected = store.current.selectedThreadId === threadId;
        if (wasSelected) loadTokenRef.current += 1;
        update({
          copiedMessageId: wasSelected
            ? null
            : store.current.copiedMessageId,
          deletingThreadId: null,
          draft: wasSelected ? "" : store.current.draft,
          draftFiles: wasSelected ? [] : store.current.draftFiles,
          draftReferences: wasSelected ? [] : store.current.draftReferences,
          attachmentError: wasSelected ? null : store.current.attachmentError,
          sendError: wasSelected ? null : store.current.sendError,
          isThreadLoading: wasSelected
            ? false
            : store.current.isThreadLoading,
          messages: wasSelected ? [] : store.current.messages,
          selectedThreadId: wasSelected
            ? null
            : store.current.selectedThreadId,
          threads: store.current.threads.filter(
            (thread) => thread.threadId !== threadId,
          ),
        });
        logChat("thread.deleted", { threadId });
      } catch (error) {
        logChat("thread.delete_failed", {
          error: errorMessage(error),
          threadId,
        });
        update({ deletingThreadId: null, threadDeleteError: true });
      }
    },
    [api.deleteAssistantThread, store, update, userEmail],
  );

  const newChat = useCallback(() => {
    const current = store.current;
    if (current.isPreparingThread || current.isRunning) return;
    loadTokenRef.current += 1;
    onContextTruncated(false);
    update({
      copiedMessageId: null,
      draft: "",
      draftReferences: [],
      draftFiles: [],
      attachmentError: null,
      sendError: null,
      isThreadLoading: false,
      messages: [],
      selectedThreadId: null,
      threadDeleteError: false,
      threadLoadError: false,
    });
  }, [onContextTruncated, store, update]);

  const selectThread = useCallback(
    async (threadId: string) => {
      const current = store.current;
      const getAssistantThread = api.getAssistantThread;
      if (
        !userEmail ||
        current.isPreparingThread ||
        current.isRunning ||
        threadId === current.selectedThreadId ||
        !getAssistantThread
      )
        return;

      const token = ++loadTokenRef.current;
      onContextTruncated(false);
      update({
        copiedMessageId: null,
        draft: "",
        draftReferences: [],
        draftFiles: [],
        attachmentError: null,
        sendError: null,
        isThreadLoading: true,
        messages: [],
        selectedThreadId: threadId,
        threadLoadError: false,
      });
      try {
        const thread = await getAssistantThread(threadId);
        if (token === loadTokenRef.current) {
          update({
            isThreadLoading: false,
            messages: activeMessageBranch(thread.messages),
          });
        }
      } catch (error) {
        logChat("thread.load_failed", { error: errorMessage(error), threadId });
        if (token === loadTokenRef.current) {
          update({
            isThreadLoading: false,
            selectedThreadId: null,
            threadLoadError: true,
          });
        }
      }
    },
    [api.getAssistantThread, onContextTruncated, store, update, userEmail],
  );

  const stop = useCallback(() => {
    const controller = abortControllerRef.current;
    if (!controller) return;
    controller.abort();
    const runningMessage = [...store.current.messages]
      .reverse()
      .find(
        (message): message is AssistantMessage =>
          message.role === "assistant" && message.status.type === "running",
      );
    if (runningMessage) {
      logChat("run.cancelled", { assistantMessageId: runningMessage.id });
      replaceAssistantMessage(runningMessage.id, undefined, {
        reason: "cancelled",
        type: "incomplete",
      });
    }
  }, [replaceAssistantMessage, store]);

  const retry = useCallback(
    (messageId: string) => {
      const current = store.current;
      if (current.isRunning || current.isPreparingThread || !isChatModelValid(current.selectedModel)) return;
      const failedIndex = current.messages.findIndex(
        (message) => message.id === messageId,
      );
      const previous = current.messages[failedIndex - 1];
      if (failedIndex < 1 || previous?.role !== "user") return;
      update({ messages: current.messages.slice(0, failedIndex) });
      void startRun(previous, current.selectedThreadId ?? "", true,
        current.selectedModel.type === "model" ? current.selectedModel.id.trim() : undefined);
    },
    [startRun, store, update],
  );

  const copy = useCallback(
    async (messageId: string) => {
      const message = store.current.messages.find(
        (candidate) => candidate.id === messageId,
      );
      if (message?.role !== "assistant" || !navigator.clipboard) return;
      try {
        await navigator.clipboard.writeText(textContent(message));
        update({ copiedMessageId: messageId });
      } catch {
        // Clipboard access is browser-policy controlled. The response remains selectable if denied.
      }
    },
    [store, update],
  );

  useEffect(() => {
    if (!userEmail || !api.listAssistantThreads) return;
    let active = true;
    void api
      .listAssistantThreads({
        scope,
        ...(scope === "deal" && dealId ? { dealId } : {}),
      })
      .then((page) => {
        logChat("thread_list.loaded", { threadCount: page.threads.length });
        if (active) {
          update({
            isThreadListLoading: false,
            threadListError: false,
            threads: mergeThreadPage(page.threads, store.current.threads),
            nextCursor: page.nextCursor,
          });
        }
      })
      .catch((error) => {
        logChat("thread_list.load_failed", { error: errorMessage(error) });
        if (active)
          update({ isThreadListLoading: false, threadListError: true });
      });
    return () => {
      active = false;
    };
  }, [api, store, update, userEmail, scope, dealId]);


  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      abortControllerRef.current?.abort();
    };
  }, []);

  const loadMoreThreads = useCallback(async () => {
    const cursor = store.current.nextCursor;
    if (
      !api.listAssistantThreads ||
      !cursor ||
      store.current.isThreadListLoading
    )
      return;
    update({ isThreadListLoading: true });
    const options: AssistantThreadListOptions = {
      after: cursor,
      scope,
      ...(scope === "deal" && dealId ? { dealId } : {}),
    };
    try {
      const page = await api.listAssistantThreads(options);
      update({
        isThreadListLoading: false,
        threadListError: false,
        nextCursor: page.nextCursor,
        threads: mergeThreadPage(page.threads, store.current.threads),
      });
    } catch {
      update({ isThreadListLoading: false, threadListError: true });
    }
  }, [api, userEmail, scope, dealId, store, update]);
  return {
    loadMoreThreads,
    copy,
    deleteThread,
    newChat,
    retry,
    selectThread,
    send,
    setDraft,
    setReferences,
    addReference,
    addFiles,
    removeFile,
    setModel,
    state,
    stop,
  };
}

const AgentRuntimeContext = createContext<AgentRuntimeValue | null>(null);

type ProviderProps = Omit<AgentRuntimeOptions, "api"> & { children: ReactNode };
export function QueryChatRuntimeProvider(props: ProviderProps) {
  return (
    <ScopedChatRuntimeProvider
      key={`${props.userEmail}:${props.scope ?? "all"}:${props.dealId ?? ""}`}
      {...props}
    />
  );
}
function ScopedChatRuntimeProvider({ children, ...options }: ProviderProps) {
  const agentRuntime = useAgentRuntime({ api: runtime.api, ...options });
  return (
    <AgentRuntimeContext.Provider value={agentRuntime}>
      {children}
    </AgentRuntimeContext.Provider>
  );
}

export function useAgentRuntimeContext() {
  const agentRuntime = useContext(AgentRuntimeContext);
  if (!agentRuntime)
    throw new Error(
      "Chat controls must be rendered inside QueryChatRuntimeProvider.",
    );
  return agentRuntime;
}

export function useThreadRuntimeState<T>(
  select: (state: ThreadRuntimeState) => T,
) {
  return select(useAgentRuntimeContext().state);
}

function activeMessageBranch(
  messages: readonly PersistedAssistantMessage[],
): Message[] {
  if (messages.length === 0) return [];
  const byId = new Map(messages.map((message) => [message.messageId, message]));
  const branch: PersistedAssistantMessage[] = [];
  const lastMessage = messages[messages.length - 1];
  if (!lastMessage) return [];
  let current: PersistedAssistantMessage | undefined = lastMessage;
  while (current) {
    branch.push(current);
    current = current.parentMessageId
      ? byId.get(current.parentMessageId)
      : undefined;
  }
  return branch.reverse().map(fromAssistantMessage);
}

function mergeThreadPage(
  remoteThreads: readonly AssistantThread[],
  localThreads: readonly AssistantThread[],
) {
  const remoteThreadIds = new Set(
    remoteThreads.map((thread) => thread.threadId),
  );
  return [
    ...localThreads.filter((thread) => !remoteThreadIds.has(thread.threadId)),
    ...remoteThreads,
  ];
}

function fromAssistantMessage(message: PersistedAssistantMessage): Message {
  const content = [{ text: message.content, type: "text" as const }];
  if (message.role === "user")
    return { attachments: [], content, id: message.messageId, role: "user" };
  return {
    content,
    id: message.messageId,
    role: "assistant",
    status:
      message.status === "completed"
        ? { reason: "stop", type: "complete" }
        : message.status === "streaming"
          ? { type: "running" }
          : {
              reason: message.status === "cancelled" ? "cancelled" : "error",
              type: "incomplete",
            },
  };
}

function userMessageFor(text: string, files: readonly File[], references: readonly ChatFileReference[], scopeDealId: string | null): UserMessage {
  return {
    attachments: [...files],
    fileReferences: [...references],
    ...(scopeDealId ? { fileScopeDealId: scopeDealId } : {}),
    content: [{ text, type: "text" }],
    id: createId(),
    role: "user",
  };
}

function assistantMessageFor(): AssistantMessage {
  return {
    content: [{ text: "", type: "text" }],
    id: createId(),
    role: "assistant",
    status: { type: "running" },
  };
}

function createId() {
  return (
    globalThis.crypto?.randomUUID?.() ??
    `chat-${Date.now()}-${Math.random().toString(36).slice(2)}`
  );
}

function textContent(message: Message) {
  return message.content
    .map((part) => (part.type === "text" ? part.text : ""))
    .join("");
}

function logChat(event: string, details: Record<string, unknown>) {
  console.info(`[chat] ${event}`, details);
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

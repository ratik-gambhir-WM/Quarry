import {
  ExportedMessageRepository,
  useAuiState,
  type RemoteThreadListAdapter,
  type ThreadHistoryAdapter,
  type ThreadMessage,
} from "@assistant-ui/react";
import { useMemo } from "react";
import type { AssistantMessage, QuarryApi } from "../../contracts/quarryApi";

export function createAssistantThreadListAdapter(
  api: QuarryApi,
  userEmail: string,
): RemoteThreadListAdapter {
  function useThreadAdapters() {
    const threadId = useAuiState((state) => state.threadListItem.remoteId);
    return useMemo(
      () => ({ history: createThreadHistoryAdapter(api, userEmail, threadId) }),
      [threadId],
    );
  }

  return {
    async archive(remoteId) {
      await api.archiveAssistantThread(remoteId, userEmail);
    },
    async delete(remoteId) {
      await api.deleteAssistantThread(remoteId, userEmail);
    },
    fetch: async (threadId) => {
      return toRemoteThread(await api.getAssistantThread(threadId, userEmail));
    },
    async generateTitle(remoteId, messages) {
      const title = titleFromMessages(messages);
      if (title) await api.renameAssistantThread(remoteId, userEmail, title);
      return new ReadableStream({ start: (controller) => controller.close() });
    },
    async initialize(threadId) {
      const thread = await api.createAssistantThread(userEmail, threadId);
      return { remoteId: thread.threadId };
    },
    async list(params) {
      const page = await api.listAssistantThreads(userEmail, { after: params?.after });
      return {
        threads: page.threads.map(toRemoteThread),
        ...(page.nextCursor ? { nextCursor: page.nextCursor } : {}),
      };
    },
    async rename(remoteId, newTitle) {
      await api.renameAssistantThread(remoteId, userEmail, newTitle);
    },
    async unarchive(remoteId) {
      await api.unarchiveAssistantThread(remoteId, userEmail);
    },
    unstable_useAdapters: useThreadAdapters,
  };
}

function createThreadHistoryAdapter(
  api: QuarryApi,
  userEmail: string,
  threadId: string | undefined,
): ThreadHistoryAdapter {
  return {
    // The run endpoint atomically owns user/assistant writes. These hooks intentionally avoid
    // writing the same runtime messages a second time through the vendor persistence lifecycle.
    append: async () => undefined,
    update: async () => undefined,
    async load() {
      if (!threadId) return { messages: [] };
      const thread = await api.getAssistantThread(threadId, userEmail);
      return ExportedMessageRepository.fromBranchableArray(
        thread.messages.map((message) => ({
          message: toThreadMessage(message),
          parentId: message.parentMessageId,
        })),
        { headId: thread.messages[thread.messages.length - 1]?.messageId ?? null },
      );
    },
  };
}

function toRemoteThread(thread: {
  lastMessageAt: string;
  status: "regular" | "archived";
  threadId: string;
  title: string;
}) {
  return {
    lastMessageAt: new Date(`${thread.lastMessageAt.replace(" ", "T")}Z`),
    remoteId: thread.threadId,
    status: thread.status,
    title: thread.title,
  } as const;
}

function toThreadMessage(message: AssistantMessage) {
  const common = {
    content: [{ text: message.content, type: "text" as const }],
    createdAt: new Date(`${message.createdAt.replace(" ", "T")}Z`),
    id: message.messageId,
    role: message.role,
  };
  if (message.role === "user") return common;
  return {
    ...common,
    status: message.status === "completed"
      ? { type: "complete" as const, reason: "stop" as const }
      : {
          type: "incomplete" as const,
          reason: message.status === "cancelled" ? "cancelled" as const : "error" as const,
        },
  };
}

function titleFromMessages(messages: readonly ThreadMessage[]) {
  const firstUser = messages.find((message) => message.role === "user");
  if (!firstUser) return undefined;
  const text = firstUser.content
    .filter((part) => part.type === "text")
    .map((part) => part.type === "text" ? part.text : "")
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
  if (!text) return undefined;
  return [...text].slice(0, 80).join("");
}

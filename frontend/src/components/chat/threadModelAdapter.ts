import type {
  QuarryApi,
  SendQueryEventHandlers,
} from "../../contracts/quarryApi";
import type { Message, ModelAdapter } from "./chatModel";

const MAX_CONTEXT_MESSAGES = 64;
const MAX_CONTEXT_CHARS = 400_000;
const MAX_MESSAGE_CHARS = 100_000;

const SERVER_ERROR_MESSAGE =
  "The assistant could not complete the response. Please try again.";
const CONNECTION_ERROR_MESSAGE =
  "The assistant connection was interrupted. Please try again.";

type ModelAdapterOptions = {
  onContextTruncated?: (truncated: boolean) => void;
};

type QueueItem =
  | { kind: "snapshot"; text: string }
  | { kind: "done" }
  | { error: Error; kind: "error" };

type CompletedPair = readonly [string, string];

export function createModelAdapter(
  api: Pick<QuarryApi, "runAssistantThread">,
  options: ModelAdapterOptions = {},
): ModelAdapter {
  return {
    async *run(runOptions) {
      const prompt = readCurrentPrompt(runOptions.messages);
      const { messageCount, truncated } = estimateHistory(runOptions.messages);
      options.onContextTruncated?.(truncated);
      console.info("[chat] adapter.prepared", {
        estimatedContextMessageCount: messageCount,
        truncated,
        persisted: Boolean(runOptions.threadId),
        promptChars: [...prompt].length,
      });

      if (runOptions.abortSignal.aborted) return;

      const queue = createQueue();
      let buffer = "";
      let cleanup: (() => void) | undefined;
      let cleanupPending = false;
      let cleanupCalled = false;

      const dispose = () => {
        if (cleanupCalled) return;
        if (!cleanup) {
          cleanupPending = true;
          return;
        }
        cleanupCalled = true;
        const stop = cleanup;
        cleanup = undefined;
        stop();
      };

      const abort = () => {
        queue.finish({ kind: "done" });
        dispose();
      };
      runOptions.abortSignal.addEventListener("abort", abort, { once: true });

      try {
        try {
          const currentUser = findCurrentUserMessage(runOptions.messages);
          const currentUserIndex = findCurrentUserIndex(runOptions.messages);
          const previousMessage = runOptions.messages[currentUserIndex - 1];
          const parentMessageId =
            previousMessage?.role === "assistant"
              ? previousMessage.id
              : undefined;
          const handlers: SendQueryEventHandlers = {
            onConnectionError: (message) => {
              console.error("[chat] stream.connection_error", { message });
              queue.finish({
                error: new Error(CONNECTION_ERROR_MESSAGE),
                kind: "error",
              });
              dispose();
            },
            onEvent: (event) => {
              if (!queue.accepting()) return;
              switch (event.type) {
                case "started":
                  console.info("[chat] stream.started", { model: event.model });
                  return;
                case "delta":
                  console.debug("[chat] stream.delta", {
                    characterCount: [...event.delta].length,
                  });
                  buffer += event.delta;
                  if (buffer.length > 0)
                    queue.push({ kind: "snapshot", text: buffer });
                  return;
                case "completed":
                  console.info("[chat] stream.completed", {
                    characterCount: [...event.response].length,
                  });
                  if (event.response !== buffer) {
                    buffer = event.response;
                    queue.push({ kind: "snapshot", text: buffer });
                  }
                  queue.finish({ kind: "done" });
                  dispose();
                  return;
                case "failed":
                  console.error("[chat] stream.failed", { error: event.error });
                  queue.finish({
                    error: new Error(SERVER_ERROR_MESSAGE),
                    kind: "error",
                  });
                  dispose();
              }
            },
          };
          const { threadId, assistantMessageId } = runOptions;
          if (!threadId || !assistantMessageId) {
            throw new Error("A saved thread and assistant message ID are required.");
          }
          cleanup = api.runAssistantThread(
            {
              assistantMessageId,
              files: [...currentUser.attachments],
              ...(currentUser.fileReferences?.length ? { fileReferences: [...currentUser.fileReferences] } : {}),
              ...(currentUser.fileScopeDealId ? { fileScopeDealId: currentUser.fileScopeDealId } : {}),
              ...(runOptions.model ? { model: runOptions.model } : {}),
              parentMessageId,
              prompt,
              requestId: assistantMessageId,
              threadId,
              userMessageId: currentUser.id,
            },
            handlers,
          );
        } catch (error) {
          console.error("[chat] adapter.transport_error", error);
          queue.finish({
            error: new Error(CONNECTION_ERROR_MESSAGE),
            kind: "error",
          });
        }
        if (
          cleanupPending ||
          !queue.accepting() ||
          runOptions.abortSignal.aborted
        )
          dispose();

        while (true) {
          const item = await queue.next();
          if (item.kind === "done") return;
          if (item.kind === "error") throw item.error;
          yield { content: [{ text: item.text, type: "text" }] };
        }
      } finally {
        runOptions.abortSignal.removeEventListener("abort", abort);
        queue.finish({ kind: "done" });
        dispose();
      }
    },
  };
}

function readCurrentPrompt(messages: readonly Message[]) {
  const current = findCurrentUserMessage(messages);
  const text = textOnlyContent(current);
  if (
    text === null || text.trim().length === 0
  ) {
    throw new Error("Messages must contain a non-empty text prompt.");
  }
  return text;
}

// This estimate drives the local truncation notice; the server owns actual history selection.
function estimateHistory(messages: readonly Message[]) {
  const currentUserIndex = findCurrentUserIndex(messages);
  const pairs: CompletedPair[] = [];

  for (let index = 0; index < currentUserIndex; index += 1) {
    const user = messages[index];
    const assistant = messages[index + 1];
    if (user?.role !== "user" || assistant?.role !== "assistant") continue;

    const userText = textOnlyContent(user);
    const assistantText = textOnlyContent(assistant);
    if (
      assistant.status.type === "complete" &&
      userText !== null &&
      assistantText !== null &&
      userText.trim().length > 0 &&
      assistantText.trim().length > 0
    ) {
      pairs.push([userText, assistantText]);
      index += 1;
    }
  }

  let selectedPairs = 0;
  let characterCount = 0;
  let truncated = false;
  const maxPairs = MAX_CONTEXT_MESSAGES / 2;

  for (let index = pairs.length - 1; index >= 0; index -= 1) {
    const pair = pairs[index];
    if (!pair) continue;
    const pairCounts = pair.map(countCharacters);
    const pairCharacterCount = pairCounts[0] + pairCounts[1];
    if (
      selectedPairs >= maxPairs ||
      pairCounts.some((count) => count > MAX_MESSAGE_CHARS) ||
      characterCount + pairCharacterCount > MAX_CONTEXT_CHARS
    ) {
      truncated = true;
      break;
    }
    selectedPairs += 1;
    characterCount += pairCharacterCount;
  }

  return {
    messageCount: selectedPairs * 2,
    truncated,
  };
}

function findCurrentUserIndex(messages: readonly Message[]) {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    if (messages[index]?.role === "user") return index;
  }
  throw new Error("A user message is required to run the assistant.");
}

function findCurrentUserMessage(messages: readonly Message[]) {
  const message = messages[findCurrentUserIndex(messages)];
  if (!message || message.role !== "user") {
    throw new Error("A user message is required to run the assistant.");
  }
  return message;
}

function textOnlyContent(message: Message) {
  if (
    message.content.length === 0 ||
    message.content.some((part) => part.type !== "text")
  ) {
    return null;
  }
  return message.content
    .map((part) => (part.type === "text" ? part.text : ""))
    .join("");
}

function countCharacters(value: string) {
  return [...value].length;
}

function createQueue() {
  const items: QueueItem[] = [];
  let isAccepting = true;
  let resolveNext: ((item: QueueItem) => void) | undefined;

  const deliver = (item: QueueItem) => {
    if (resolveNext) {
      const resolve = resolveNext;
      resolveNext = undefined;
      resolve(item);
    } else {
      items.push(item);
    }
  };

  return {
    accepting: () => isAccepting,
    finish(item: Extract<QueueItem, { kind: "done" | "error" }>) {
      if (!isAccepting) return;
      isAccepting = false;
      deliver(item);
    },
    next() {
      const item = items.shift();
      if (item) return Promise.resolve(item);
      return new Promise<QueueItem>((resolve) => {
        resolveNext = resolve;
      });
    },
    push(item: Extract<QueueItem, { kind: "snapshot" }>) {
      if (isAccepting) deliver(item);
    },
  };
}

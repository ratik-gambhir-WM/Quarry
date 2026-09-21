import type {
  ChatModelAdapter,
  ChatModelRunOptions,
  ChatModelRunResult,
  ThreadAssistantMessage,
  ThreadMessage,
  ThreadUserMessage,
} from "@assistant-ui/react";
import { describe, expect, it, vi } from "vitest";
import type { QueryModelInput, SendQueryEventHandlers } from "@/contracts/quarryApi";
import { createQueryModelAdapter } from "@/components/chat/queryModelAdapter";

describe("createQueryModelAdapter", () => {
  it("converts ordered deltas to cumulative snapshots and makes completion authoritative", async () => {
    let handlers: SendQueryEventHandlers | undefined;
    const cleanup = vi.fn();
    const queryModel = vi.fn((_input: QueryModelInput, nextHandlers: SendQueryEventHandlers) => {
      handlers = nextHandlers;
      return cleanup;
    });
    const iterator = stream(createQueryModelAdapter({ queryModel }), runOptions([user("Draft a brief")]));

    const first = iterator.next();
    handlers?.onEvent({ model: "server-default", type: "started" });
    handlers?.onEvent({ delta: "Part", type: "delta" });
    expect(await first).toEqual(snapshot("Part"));

    const second = iterator.next();
    handlers?.onEvent({ delta: "ial", type: "delta" });
    expect(await second).toEqual(snapshot("Partial"));

    const final = iterator.next();
    handlers?.onEvent({ response: "Authoritative final", type: "completed" });
    expect(await final).toEqual(snapshot("Authoritative final"));
    expect(await iterator.next()).toEqual({ done: true, value: undefined });
    expect(cleanup).toHaveBeenCalledTimes(1);
    expect(queryModel).toHaveBeenCalledWith(
      { context: [], files: [], prompt: "Draft a brief" },
      expect.any(Object),
    );
  });

  it("supports a synchronous no-delta completion and disposes exactly once", async () => {
    const cleanup = vi.fn();
    const queryModel = vi.fn((_input: QueryModelInput, handlers: SendQueryEventHandlers) => {
      handlers.onEvent({ response: "Immediate answer", type: "completed" });
      return cleanup;
    });

    const output = await collect(
      createQueryModelAdapter({ queryModel }),
      runOptions([user("Answer now")]),
    );

    expect(output).toEqual([[{ text: "Immediate answer", type: "text" }]]);
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it("maps server and connection failures to distinct sanitized errors", async () => {
    const serverAdapter = createQueryModelAdapter({
      queryModel: (_input, handlers) => {
        handlers.onEvent({ error: "provider body that must not leak", type: "failed" });
        return () => undefined;
      },
    });
    const connectionAdapter = createQueryModelAdapter({
      queryModel: (_input, handlers) => {
        handlers.onConnectionError?.("raw transport detail");
        return () => undefined;
      },
    });

    await expect(collect(serverAdapter, runOptions([user("Question")]))).rejects.toThrow(
      "The assistant could not complete the response. Please try again.",
    );
    await expect(collect(connectionAdapter, runOptions([user("Question")]))).rejects.toThrow(
      "The assistant connection was interrupted. Please try again.",
    );
    await expect(collect(createQueryModelAdapter({
      queryModel: () => {
        throw new Error("raw synchronous transport detail");
      },
    }), runOptions([user("Question")]))).rejects.toThrow(
      "The assistant connection was interrupted. Please try again.",
    );
  });

  it("aborts quietly, preserves the last yielded snapshot, and ignores late events", async () => {
    let handlers: SendQueryEventHandlers | undefined;
    const cleanup = vi.fn();
    const controller = new AbortController();
    const adapter = createQueryModelAdapter({
      queryModel: (_input, nextHandlers) => {
        handlers = nextHandlers;
        return cleanup;
      },
    });
    const iterator = stream(adapter, runOptions([user("Stream")], controller.signal));

    const first = iterator.next();
    handlers?.onEvent({ delta: "Keep this", type: "delta" });
    expect(await first).toEqual(snapshot("Keep this"));

    const done = iterator.next();
    controller.abort();
    handlers?.onEvent({ delta: " but not this", type: "delta" });
    handlers?.onEvent({ response: "Late completion", type: "completed" });

    expect(await done).toEqual({ done: true, value: undefined });
    expect(cleanup).toHaveBeenCalledTimes(1);
  });

  it("replays only completed text pairs and omits optional server-owned defaults", async () => {
    let captured: QueryModelInput | undefined;
    const queryModel = vi.fn((input: QueryModelInput, handlers: SendQueryEventHandlers) => {
      captured = input;
      handlers.onEvent({ response: "Next", type: "completed" });
      return () => undefined;
    });
    const messages: ThreadMessage[] = [
      user("First", "u1"),
      assistant("First answer", { type: "complete", reason: "stop" }, "a1"),
      user("Failed question", "u2"),
      assistant("Partial", { type: "incomplete", reason: "error" }, "a2"),
      user("Current prompt", "u3"),
    ];

    await collect(createQueryModelAdapter({ queryModel }), runOptions(messages));

    expect(captured).toEqual({
      context: [
        { content: "First", role: "user" },
        { content: "First answer", role: "assistant" },
      ],
      files: [],
      prompt: "Current prompt",
    });
    expect(captured).not.toHaveProperty("model");
    expect(captured).not.toHaveProperty("systemInstructions");
  });

  it("keeps the newest 32 whole pairs and reports omitted older context", async () => {
    let captured: QueryModelInput | undefined;
    const onContextTruncated = vi.fn();
    const history: ThreadMessage[] = [];
    for (let index = 0; index < 33; index += 1) {
      history.push(user(`Question ${index}`, `u${index}`));
      history.push(
        assistant(`Answer ${index}`, { type: "complete", reason: "stop" }, `a${index}`),
      );
    }
    history.push(user("Current", "current"));

    await collect(
      createQueryModelAdapter({
        queryModel: (input, handlers) => {
          captured = input;
          handlers.onEvent({ response: "Done", type: "completed" });
          return () => undefined;
        },
      }, { onContextTruncated }),
      runOptions(history),
    );

    expect(captured?.context).toHaveLength(64);
    expect(captured?.context[0]).toEqual({ content: "Question 1", role: "user" });
    expect(captured?.context[(captured?.context.length ?? 0) - 1]).toEqual({
      content: "Answer 32",
      role: "assistant",
    });
    expect(onContextTruncated).toHaveBeenCalledWith(true);
  });

  it("rejects blank and non-text current messages before starting transport", async () => {
    const queryModel = vi.fn();
    const adapter = createQueryModelAdapter({ queryModel });

    await expect(collect(adapter, runOptions([user("   ")]))).rejects.toThrow(
      "Messages must contain non-empty text only.",
    );
    await expect(
      collect(adapter, runOptions([{
        ...user("ignored"),
        content: [{ image: "data:image/png;base64,AA==", type: "image" }],
      }])),
    ).rejects.toThrow("Messages must contain non-empty text only.");
    expect(queryModel).not.toHaveBeenCalled();
  });
});

function runOptions(
  messages: readonly ThreadMessage[],
  abortSignal = new AbortController().signal,
): ChatModelRunOptions {
  return {
    abortSignal,
    context: {},
    messages,
    runConfig: {},
    unstable_getMessage: () => messages[messages.length - 1] ?? user("fallback"),
  };
}

function user(text: string, id = "user"): ThreadUserMessage {
  return {
    attachments: [],
    content: [{ text, type: "text" }],
    createdAt: new Date(0),
    id,
    metadata: { custom: {} },
    role: "user",
  };
}

function assistant(
  text: string,
  status: ThreadAssistantMessage["status"],
  id = "assistant",
): ThreadAssistantMessage {
  return {
    content: [{ text, type: "text" }],
    createdAt: new Date(0),
    id,
    metadata: {
      custom: {},
      steps: [],
      unstable_annotations: [],
      unstable_data: [],
      unstable_state: null,
    },
    role: "assistant",
    status,
  };
}

function stream(adapter: ChatModelAdapter, options: ChatModelRunOptions) {
  const result = adapter.run(options);
  if (!(Symbol.asyncIterator in result)) throw new Error("Expected a streaming adapter.");
  return result;
}

async function collect(adapter: ChatModelAdapter, options: ChatModelRunOptions) {
  const output: ChatModelRunResult["content"][] = [];
  for await (const result of stream(adapter, options)) output.push(result.content);
  return output;
}

function snapshot(text: string) {
  return {
    done: false,
    value: { content: [{ text, type: "text" }] },
  };
}

export type ContentPart =
  { text: string; type: "text" } | { image: string; type: "image" };

export type UserMessage = {
  attachments: readonly File[];
  content: readonly ContentPart[];
  id: string;
  role: "user";
};

export type AssistantStatus =
  | { reason: "stop"; type: "complete" }
  | { reason: "cancelled" | "error"; type: "incomplete" }
  | { type: "running" };

export type AssistantMessage = {
  content: readonly ContentPart[];
  id: string;
  role: "assistant";
  status: AssistantStatus;
};

export type Message = UserMessage | AssistantMessage;

export type ModelRunOptions = {
  abortSignal: AbortSignal;
  assistantMessageId?: string;
  messages: readonly Message[];
  model?: string;
  threadId?: string;
};

export type ModelRunResult = {
  content: readonly [{ text: string; type: "text" }];
};

export type ModelAdapter = {
  run(options: ModelRunOptions): AsyncGenerator<ModelRunResult>;
};

export type ChatModelSelection =
  | { type: "default" }
  | { type: "model"; id: string };

export function isChatModelValid(model: ChatModelSelection) {
  return model.type === "default" || (
    model.id.trim().length > 0 &&
    [...model.id.trim()].length <= 128 &&
    !/[\u0000-\u001f\u007f-\u009f]/u.test(model.id)
  );
}

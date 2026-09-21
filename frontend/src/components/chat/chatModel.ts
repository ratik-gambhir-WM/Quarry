export type ContentPart =
  | { text: string; type: "text" }
  | { image: string; type: "image" };

export type UserMessage = {
  attachments: readonly unknown[];
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
  threadId?: string;
};

export type ModelRunResult = {
  content: readonly [{ text: string; type: "text" }];
};

export type ModelAdapter = {
  run(options: ModelRunOptions): AsyncGenerator<ModelRunResult>;
};

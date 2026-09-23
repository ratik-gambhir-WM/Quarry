import type { SendQueryEvent } from "../contracts/quarryApi";

const MAX_EVENT_BYTES = 1_048_576;

export class QuerySseParser {
  private buffer = "";
  private readonly decoder = new TextDecoder("utf-8", { fatal: true });
  private started = false;
  private terminal = false;

  constructor(private readonly onEvent: (event: SendQueryEvent) => void) {}

  push(bytes: Uint8Array) {
    this.buffer += this.decoder.decode(bytes, { stream: true });
    this.drain();
    if (utf8ByteLength(this.buffer) > MAX_EVENT_BYTES) {
      throw new Error("The query stream returned an oversized event.");
    }
  }

  finish() {
    this.buffer += this.decoder.decode();
    this.drain();
    if (this.buffer.trim()) {
      throw new Error("The query stream ended with an incomplete event.");
    }
    if (!this.terminal) {
      throw new Error("The query stream ended before a terminal event.");
    }
  }

  private drain() {
    for (;;) {
      const boundary = findBoundary(this.buffer);
      if (!boundary) return;
      if (utf8ByteLength(this.buffer, boundary.index) > MAX_EVENT_BYTES) {
        throw new Error("The query stream returned an oversized event.");
      }
      const frame = this.buffer.slice(0, boundary.index);
      this.buffer = this.buffer.slice(boundary.index + boundary.length);
      this.processFrame(frame);
    }
  }

  private processFrame(frame: string) {
    let eventName: string | undefined;
    const data: string[] = [];
    for (const sourceLine of frame.split("\n")) {
      const line = sourceLine.endsWith("\r") ? sourceLine.slice(0, -1) : sourceLine;
      if (line.startsWith(":")) continue;
      if (line.startsWith("event:")) eventName = line.slice(6).trim();
      if (line.startsWith("data:")) {
        const value = line.slice(5);
        data.push(value.startsWith(" ") ? value.slice(1) : value);
      }
    }
    if (!eventName && data.length === 0) return;
    if (!eventName || data.length === 0 || this.terminal) {
      throw new Error("The query stream returned an invalid event frame.");
    }
    let value: unknown;
    try {
      value = JSON.parse(data.join("\n"));
    } catch {
      throw new Error("The query stream returned malformed JSON.");
    }
    const event = parseQueryEvent(value);
    if (event.type !== eventName) {
      throw new Error("The query stream event name did not match its payload.");
    }
    if (!this.started && event.type !== "started") {
      throw new Error("The query stream did not start correctly.");
    }
    if (this.started && event.type === "started") {
      throw new Error("The query stream started more than once.");
    }
    if (event.type === "started") this.started = true;
    if (event.type === "completed" || event.type === "failed") this.terminal = true;
    this.onEvent(event);
  }
}

export function parseQueryEvent(value: unknown): SendQueryEvent {
  if (!isRecord(value) || typeof value.type !== "string") {
    throw new Error("The query stream returned an invalid event.");
  }
  switch (value.type) {
    case "started":
      requireOptionalKeys(value, ["model", "type"], [
        "assistantMessageId",
        "threadId",
        "userMessageId",
      ]);
      if (
        typeof value.model === "string"
        && value.model.length > 0
        && [value.assistantMessageId, value.threadId, value.userMessageId]
          .every((item) => item === undefined || typeof item === "string")
      ) return value as SendQueryEvent;
      break;
    case "delta":
      requireKeys(value, ["delta", "type"]);
      if (typeof value.delta === "string") return value as SendQueryEvent;
      break;
    case "completed":
      requireKeys(value, ["response", "type"]);
      if (typeof value.response === "string" && value.response.trim()) return value as SendQueryEvent;
      break;
    case "failed":
      requireKeys(value, ["error", "type"]);
      if (typeof value.error === "string" && value.error.length > 0) return value as SendQueryEvent;
      break;
  }
  throw new Error("The query stream returned an invalid event.");
}

function findBoundary(value: string) {
  const lf = value.indexOf("\n\n");
  const crlf = value.indexOf("\r\n\r\n");
  if (lf < 0 && crlf < 0) return undefined;
  if (crlf >= 0 && (lf < 0 || crlf < lf)) return { index: crlf, length: 4 };
  return { index: lf, length: 2 };
}

function utf8ByteLength(value: string, end = value.length) {
  let bytes = 0;
  for (let index = 0; index < end; index += 1) {
    const codeUnit = value.charCodeAt(index);
    if (codeUnit <= 0x7f) {
      bytes += 1;
    } else if (codeUnit <= 0x7ff) {
      bytes += 2;
    } else if (
      codeUnit >= 0xd800
      && codeUnit <= 0xdbff
      && index + 1 < end
      && value.charCodeAt(index + 1) >= 0xdc00
      && value.charCodeAt(index + 1) <= 0xdfff
    ) {
      bytes += 4;
      index += 1;
    } else {
      bytes += 3;
    }
  }
  return bytes;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function requireKeys(value: Record<string, unknown>, expected: string[]) {
  const keys = Object.keys(value).sort();
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) {
    throw new Error("The query stream returned an invalid event shape.");
  }
}

function requireOptionalKeys(
  value: Record<string, unknown>,
  required: string[],
  optional: string[],
) {
  const keys = Object.keys(value);
  if (
    required.some((key) => !keys.includes(key))
    || keys.some((key) => !required.includes(key) && !optional.includes(key))
  ) {
    throw new Error("The query stream returned an invalid event shape.");
  }
}

import { describe, expect, it, vi } from "vitest";
import { QuerySseParser } from "@/api/querySse";

describe("QuerySseParser", () => {
  it("parses chunked UTF-8, CRLF, comments, and repeated data lines", () => {
    const onEvent = vi.fn();
    const parser = new QuerySseParser(onEvent);
    const bytes = new TextEncoder().encode(
      ": keep-alive\r\n\r\nevent: started\r\ndata: {\"type\":\"started\",\r\ndata: \"model\":\"gpt-5.5\"}\r\n\r\nevent: delta\ndata: {\"type\":\"delta\",\"delta\":\"hé\"}\n\nevent: completed\ndata: {\"type\":\"completed\",\"response\":\"hello\"}\n\n",
    );
    const split = bytes.findIndex((byte) => byte === 0xc3) + 1;
    parser.push(bytes.slice(0, split));
    parser.push(bytes.slice(split));
    parser.finish();
    expect(onEvent.mock.calls.map(([event]) => event.type)).toEqual([
      "started", "delta", "completed",
    ]);
  });

  it("rejects malformed and unterminated streams", () => {
    const malformed = new QuerySseParser(vi.fn());
    expect(() => malformed.push(new TextEncoder().encode("event: started\ndata: {}\n\n"))).toThrow();
    expect(() => new QuerySseParser(vi.fn()).finish()).toThrow("terminal event");
  });

  it("enforces the event limit in UTF-8 bytes", () => {
    const parser = new QuerySseParser(vi.fn());
    const oversized = `event: delta\ndata: {"type":"delta","delta":"${"😀".repeat(262_145)}"}`;

    expect(() => parser.push(new TextEncoder().encode(oversized))).toThrow("oversized event");
  });

  it("rejects an oversized event that includes its terminating blank line", () => {
    const parser = new QuerySseParser(vi.fn());
    const oversized = `event: started\ndata: {"type":"started","model":"${"x".repeat(1_048_576)}"}\n\n`;

    expect(() => parser.push(new TextEncoder().encode(oversized))).toThrow("oversized event");
  });
});

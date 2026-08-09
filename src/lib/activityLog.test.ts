import { beforeEach, describe, expect, it } from "vitest";
import {
  beginIpcRequest,
  clearActivityLog,
  finishIpcRequest,
  getActivityLogEntries,
} from "./activityLog";

describe("activityLog", () => {
  beforeEach(() => {
    clearActivityLog();
  });

  it("redacts secrets, email addresses, and absolute paths recursively", () => {
    const id = beginIpcRequest("create_user", {
      apiKey: "sk-secret",
      email: "person@example.com",
      nested: { path: "/Users/person/Documents/private.pdf" },
    });
    finishIpcRequest(id, {
      details: { token: "bearer-secret" },
      durationMs: 12,
      status: "success",
    });

    const [entry] = getActivityLogEntries();
    expect(entry.details).not.toContain("sk-secret");
    expect(entry.details).not.toContain("person@example.com");
    expect(entry.details).not.toContain("/Users/person");
    expect(entry.details).not.toContain("bearer-secret");
    expect(entry.details).toContain("[REDACTED]");
  });

  it("bounds nested arrays and long strings", () => {
    beginIpcRequest("bounded", {
      items: Array.from({ length: 40 }, (_, index) => ({ index, value: "x".repeat(2_100) })),
    });

    const [entry] = getActivityLogEntries();
    expect(entry.details).toContain("[10 more items]");
    expect(entry.details).toContain("[truncated 100 characters]");
    expect(entry.details?.length).toBeLessThan(70_000);
  });
});

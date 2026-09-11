import { describe, expect, it } from "vitest";
import { runtime } from "@quarry/runtime";

describe("selected Quarry runtime", () => {
  it("composes the web target for the test mode", () => {
    expect(runtime.target).toBe("web");
    expect(runtime.api.listDeals).toBeTypeOf("function");
    expect(runtime.api.startProcessFile).toBeTypeOf("function");
    expect(runtime.platform.saveFile).toBeTypeOf("function");
  });
});

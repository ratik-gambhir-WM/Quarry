import { describe, expect, it } from "vitest";
import { formatFileSize, formatShortUtcDate } from "./formatters";

describe("UI formatters", () => {
  it("formats file sizes at stable unit boundaries", () => {
    expect(formatFileSize(42)).toBe("42 B");
    expect(formatFileSize(1024)).toBe("1.0 KB");
    expect(formatFileSize(1024 * 1024)).toBe("1.0 MB");
  });

  it("returns the UI placeholder for missing or invalid dates", () => {
    expect(formatShortUtcDate()).toBe("—");
    expect(formatShortUtcDate("not-a-date")).toBe("—");
  });
});

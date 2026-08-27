import { describe, expect, it } from "vitest";
import { normalizeSource } from "./normalize-source";

describe("normalizeSource", () => {
  it("gives pdf.js a copy of a Uint8Array", () => {
    const original = new Uint8Array([1, 2, 3]);
    const normalized = normalizeSource(original);

    expect(normalized).not.toBeNull();
    expect(typeof normalized).toBe("object");
    expect(normalized).toHaveProperty("data");
    const copied = (normalized as { data: Uint8Array }).data;
    expect(copied).not.toBe(original);
    expect(copied.buffer).not.toBe(original.buffer);
    copied[0] = 9;
    expect(original[0]).toBe(1);
  });

  it("gives pdf.js a copy of an ArrayBuffer", () => {
    const original = new Uint8Array([4, 5, 6]).buffer;
    const normalized = normalizeSource(original) as { data: ArrayBuffer };

    expect(normalized.data).not.toBe(original);
    new Uint8Array(normalized.data)[0] = 9;
    expect(new Uint8Array(original)[0]).toBe(4);
  });
});

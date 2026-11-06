import { describe, expect, it } from "vitest";
import { keywordSearchExcerpt } from "@/data/documents/keywordSearch";

describe("keyword search excerpts", () => {
  it("centers the excerpt on a match beyond the start of a long chunk", () => {
    const text = `${"Introductory background. ".repeat(40)}The application uses Ruby on Rails. ${"More context. ".repeat(40)}`;
    const parts = keywordSearchExcerpt(text, "ruby on rails");
    const excerpt = parts.map((part) => part.text).join("");
    expect(excerpt).toContain("Ruby on Rails");
    expect(excerpt.length).toBeLessThanOrEqual(322);
    expect(excerpt.startsWith("…")).toBe(true);
    expect(excerpt.endsWith("…")).toBe(true);
    expect(parts.filter((part) => part.matched).map((part) => part.text)).toEqual([
      "Ruby", "on", "Rails",
    ]);
  });

  it("highlights whole words and distinguishes C, C++ and C#", () => {
    const text = "Confidential provisions, C, C++ and C#; RUBY (on Rails), café.";
    for (const [query, matches] of [
      ["on", ["on"]],
      ["C++", ["C++"]],
      ["C#", ["C#"]],
      ["C", ["C"]],
      ["ruby, CAFÉ", ["RUBY", "café"]],
    ] as const) {
      expect(keywordSearchExcerpt(text, query).filter((part) => part.matched).map((part) => part.text)).toEqual(matches);
    }
  });

  it("normalizes line breaks and handles empty or unmatched queries", () => {
    expect(keywordSearchExcerpt("Ruby\n\n on\tRails", "Ruby").map((part) => part.text).join("")).toBe("Ruby on Rails");
    expect(keywordSearchExcerpt("", "ruby")).toEqual([]);
    expect(keywordSearchExcerpt("No match here.", "ruby")).toEqual([
      { text: "No match here.", matched: false },
    ]);
    expect(keywordSearchExcerpt("Literal + symbols", "+++#").every((part) => !part.matched)).toBe(true);
  });
});

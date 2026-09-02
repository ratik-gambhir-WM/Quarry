import { describe, expect, it } from "vitest";
import { buildDocumentSearchFixtureResults } from "../../../fixtures/data-room/documentSearch";
import { isDocumentSearchResultAvailable } from "./documentSearchModel";

describe("documentSearch", () => {
  it("builds deterministic current-document targets ahead of related mock results", () => {
    const results = buildDocumentSearchFixtureResults("Synthetic_Terms.pdf");

    expect(results.slice(0, 2).map((result) => result.target)).toEqual([
      { kind: "pdf-page", page: 1 },
      { kind: "pdf-page", page: 2 },
    ]);
    expect(results[0]?.fileName).toBe("Synthetic_Terms.pdf");
    expect(results[results.length - 1]?.id).toBe("master-agreement");
  });

  it("enables only valid current-document page targets", () => {
    const [pageOne, pageTwo, external] =
      buildDocumentSearchFixtureResults("Synthetic.pdf");

    expect(
      isDocumentSearchResultAvailable({
        currentFileName: "Synthetic.pdf",
        numPages: 1,
        result: pageOne!,
      }),
    ).toBe(true);
    expect(
      isDocumentSearchResultAvailable({
        currentFileName: "Synthetic.pdf",
        numPages: 1,
        result: pageTwo!,
      }),
    ).toBe(false);
    expect(
      isDocumentSearchResultAvailable({
        currentFileName: "Synthetic.pdf",
        numPages: 99,
        result: external!,
      }),
    ).toBe(false);
  });
});

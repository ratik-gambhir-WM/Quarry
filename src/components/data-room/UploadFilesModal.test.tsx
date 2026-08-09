import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import {
  UploadFileRow,
  UploadFooter,
  type UploadEntry,
} from "./UploadFilesModal";

const baseEntry: UploadEntry = {
  id: "/selected/example.pdf",
  name: "example.pdf",
  path: "/selected/example.pdf",
  selected: true,
  sizeBytes: 506_778,
  status: "processing",
};

describe("UploadFilesModal Quarry-web parity", () => {
  it("renders the Quarry-web spinner row while a native document job is processing", () => {
    const markup = renderToStaticMarkup(
      <UploadFileRow entry={baseEntry} onRemove={vi.fn()} onToggle={vi.fn()} />,
    );

    expect(markup).toContain("Processing");
    expect(markup).toContain('aria-label="Processing"');
    expect(markup).toContain("motion-safe:animate-spin");
    expect(markup).toContain('checked=""');
    expect(markup).toContain('disabled=""');
  });

  it("renders the green completion check, final duration, and Done action", () => {
    const rowMarkup = renderToStaticMarkup(
      <UploadFileRow
        entry={{ ...baseEntry, chunkCount: 12, status: "completed" }}
        onRemove={vi.fn()}
        onToggle={vi.fn()}
      />,
    );
    const footerMarkup = renderToStaticMarkup(
      <UploadFooter
        activeCount={0}
        elapsedSeconds={69}
        hasCompletedEntries
        isProcessing={false}
        onClose={vi.fn()}
        onStart={vi.fn()}
        processedSeconds={69}
        retrying={false}
        uploadableCount={0}
      />,
    );

    expect(rowMarkup).toContain("Complete");
    expect(rowMarkup).toContain('aria-label="Processing complete"');
    expect(rowMarkup).toContain("bg-[#dff5e8]");
    expect(rowMarkup).toContain("text-[#168447]");
    expect(footerMarkup).toContain("Processed in 69 seconds");
    expect(footerMarkup).toContain(">Done</button>");
    expect(footerMarkup).toContain('disabled=""');
  });

  it("renders the active footer count and elapsed-time pill", () => {
    const markup = renderToStaticMarkup(
      <UploadFooter
        activeCount={3}
        elapsedSeconds={5}
        hasCompletedEntries={false}
        isProcessing
        onClose={vi.fn()}
        onStart={vi.fn()}
        processedSeconds={null}
        retrying={false}
        uploadableCount={0}
      />,
    );

    expect(markup).toContain("Processing 3 files…");
    expect(markup).toContain("5 seconds");
    expect(markup).toContain("tabular-nums");
  });
});

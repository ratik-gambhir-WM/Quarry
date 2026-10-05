// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { UploadFilesModal } from "@/components/data-room/UploadFilesModal";

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: {
      startProcessFile: vi.fn(),
      subscribeToProcessFileJob: vi.fn(),
    },
  },
}));

afterEach(cleanup);

describe("UploadFilesModal", () => {
  it("accepts supported image files for document ingestion", () => {
    render(
      <UploadFilesModal
        dealId="DEAL-IMAGE"
        onClose={vi.fn()}
        userId="analyst@example.com"
      />,
    );
    const input = document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("expected the upload file input");
    expect(input.accept).toContain("image/png");
    expect(input.accept).toContain("image/jpeg");
    expect(input.accept).toContain("image/webp");
    expect(input.accept).toContain("image/gif");

    const file = new File([new Uint8Array([1, 2, 3])], "evidence.PNG", {
      type: "image/png",
    });
    fireEvent.change(input, { target: { files: [file] } });

    expect(screen.getByText("evidence.PNG")).toBeDefined();
    expect(screen.getByRole("checkbox", { name: "Select evidence.PNG" })).toBeDefined();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("accepts modern Office files and rejects legacy Office files", () => {
    render(
      <UploadFilesModal
        dealId="DEAL-OFFICE"
        onClose={vi.fn()}
        userId="analyst@example.com"
      />,
    );
    const input = document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("expected the upload file input");
    expect(input.accept).toContain(".xlsx");
    expect(input.accept).toContain(".pptx");
    expect(input.accept).toContain("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
    expect(input.accept).toContain("application/vnd.openxmlformats-officedocument.presentationml.presentation");

    const workbook = new File([new Uint8Array([1, 2, 3])], "model.XLSX", { type: "" });
    const presentation = new File([new Uint8Array([4, 5, 6])], "board.pptx", { type: "" });
    const legacy = new File([new Uint8Array([7])], "legacy.xls", { type: "" });
    fireEvent.change(input, { target: { files: [workbook, presentation, legacy] } });

    expect(screen.getByText("model.XLSX")).toBeDefined();
    expect(screen.getByText("board.pptx")).toBeDefined();
    expect(screen.getByRole("alert").textContent).toContain(
      "legacy.xls is not a supported document or image file.",
    );
  });
});

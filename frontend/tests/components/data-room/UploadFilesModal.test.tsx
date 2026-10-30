// @vitest-environment happy-dom

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { UploadFilesModal } from "@/components/data-room/UploadFilesModal";

const { startProcessFile, subscribeToProcessFileJob } = vi.hoisted(() => ({
  startProcessFile: vi.fn(),
  subscribeToProcessFileJob: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: {
      startProcessFile,
      subscribeToProcessFileJob,
    },
  },
}));

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("UploadFilesModal", () => {
  it("accepts supported image files for document ingestion", () => {
    render(
      <UploadFilesModal
        dealId="DEAL-IMAGE"
        onClose={vi.fn()}
        userId="analyst@example.com"
      />,
    );
    const input =
      document.querySelector<HTMLInputElement>('input[type="file"]');
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
    expect(
      screen.getByRole("checkbox", { name: "Select evidence.PNG" }),
    ).toBeDefined();
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
    const input =
      document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("expected the upload file input");
    expect(input.accept).toContain(".xlsx");
    expect(input.accept).toContain(".pptx");
    expect(input.accept).toContain(
      "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    );
    expect(input.accept).toContain(
      "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    );

    const workbook = new File([new Uint8Array([1, 2, 3])], "model.XLSX", {
      type: "",
    });
    const presentation = new File([new Uint8Array([4, 5, 6])], "board.pptx", {
      type: "",
    });
    const legacy = new File([new Uint8Array([7])], "legacy.xls", { type: "" });
    fireEvent.change(input, {
      target: { files: [workbook, presentation, legacy] },
    });

    expect(screen.getByText("model.XLSX")).toBeDefined();
    expect(screen.getByText("board.pptx")).toBeDefined();
    expect(screen.getByRole("alert").textContent).toContain(
      "legacy.xls is not a supported document or image file.",
    );
  });

  it("highlights the dropzone and adds dropped files", () => {
    render(<UploadFilesModal dealId="deal-1" onClose={vi.fn()} userId="user-1" />);
    const dropzone = screen.getByRole("button", { name: "Browse files to upload" }).parentElement;
    if (!dropzone) throw new Error("expected the upload dropzone");
    const file = new File(["contents"], "notes.pdf");
    fireEvent.dragEnter(dropzone, { dataTransfer: { types: ["Files"] } });
    expect(screen.getByText("Drop files to add them")).toBeDefined();
    fireEvent.drop(dropzone, { dataTransfer: { files: [file] } });
    expect(screen.getByText("notes.pdf")).toBeDefined();
    expect(screen.getByText(/Drop files here or/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: "Remove notes.pdf" }));
    expect(screen.queryByText("notes.pdf")).toBeNull();
  });

  it("retains the processing circle, completion check and close protection", async () => {
    const unsubscribe = vi.fn();
    startProcessFile.mockResolvedValue({ jobId: "job-1", filename: "notes.pdf" });
    subscribeToProcessFileJob.mockReturnValue(unsubscribe);
    const onClose = vi.fn();
    render(<UploadFilesModal dealId="deal-1" onClose={onClose} userId="user-1" />);
    const input = document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("expected the upload file input");
    fireEvent.change(input, { target: { files: [new File(["contents"], "notes.pdf")] } });
    fireEvent.click(screen.getByRole("button", { name: "Upload 1" }));
    await waitFor(() => expect(subscribeToProcessFileJob).toHaveBeenCalledOnce());
    expect(screen.getByLabelText("Processing")).toBeDefined();
    expect(screen.getByRole("button", { name: "Close upload files dialog" })).toHaveProperty("disabled", true);
    const handlers = subscribeToProcessFileJob.mock.calls[0][1];
    act(() => handlers.onEvent({ jobId: "job-1", filename: "notes.pdf", status: "completed" }));
    expect(screen.getByLabelText("Processing complete")).toBeDefined();
    expect(screen.getByText("Complete")).toBeDefined();
    expect(unsubscribe).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("keeps failed uploads visible and allows retry", async () => {
    startProcessFile.mockRejectedValueOnce(new Error("Invalid workbook"));
    render(<UploadFilesModal dealId="deal-1" onClose={vi.fn()} userId="user-1" />);
    const input = document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!input) throw new Error("expected the upload file input");
    fireEvent.change(input, { target: { files: [new File(["contents"], "model.xlsx")] } });
    fireEvent.click(screen.getByRole("button", { name: "Upload 1" }));
    expect(await screen.findByText("Invalid workbook")).toBeDefined();
    expect(screen.getByRole("button", { name: "Retry 1" })).toBeDefined();
  });

});

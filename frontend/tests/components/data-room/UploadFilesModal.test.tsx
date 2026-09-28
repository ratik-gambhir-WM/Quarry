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
});

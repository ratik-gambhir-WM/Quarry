// @vitest-environment happy-dom

import type { ReactNode } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SummarizePage } from "@/pages/SummarizePage";

const { saveFile, summarizePath } = vi.hoisted(() => ({
  saveFile: vi.fn(),
  summarizePath: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: {
      summarizePath,
      summarizeSelected: vi.fn(),
      summarizeUpload: vi.fn(),
    },
    platform: { saveFile },
  },
}));

vi.mock("@/components/hub/WorkspaceHomeShell", () => ({
  WorkspaceHomeShell: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

afterEach(cleanup);

describe("SummarizePage", () => {
  beforeEach(() => {
    saveFile.mockReset();
    saveFile.mockResolvedValue(undefined);
    summarizePath.mockReset();
    summarizePath.mockResolvedValue("# Generated summary\n\nUseful detail.");
  });

  it("loads and renders the Markdown result after a successful request", async () => {
    render(<SummarizePage />);

    expect(screen.queryByRole("heading", { name: "Generated summary" })).toBeNull();
    fireEvent.change(screen.getByPlaceholderText("Search or browse files in Finder..."), {
      target: { value: "/synthetic/report.pdf" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    expect(await screen.findByRole("heading", { name: "Generated summary" })).toBeTruthy();
    expect(summarizePath).toHaveBeenCalledWith("/synthetic/report.pdf");

    fireEvent.click(screen.getByRole("button", { name: "Save markdown summary" }));
    await waitFor(() => expect(saveFile).toHaveBeenCalledTimes(1));
  });

  it("clears a prior export error before retrying the save", async () => {
    saveFile.mockRejectedValueOnce(new Error("Save unavailable")).mockResolvedValueOnce(undefined);
    render(<SummarizePage />);
    fireEvent.change(screen.getByPlaceholderText("Search or browse files in Finder..."), {
      target: { value: "/synthetic/report.pdf" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Submit" }));
    await screen.findByRole("heading", { name: "Generated summary" });

    fireEvent.click(screen.getByRole("button", { name: "Save markdown summary" }));
    expect(await screen.findByText("Save unavailable")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save markdown summary" }));

    await waitFor(() => expect(saveFile).toHaveBeenCalledTimes(2));
    expect(screen.queryByText("Save unavailable")).toBeNull();
  });
});

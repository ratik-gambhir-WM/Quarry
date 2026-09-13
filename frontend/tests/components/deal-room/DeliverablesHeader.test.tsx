// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DeliverableTemplatesView } from "@/components/deal-room/DeliverableTemplatesView";
import { DeliverablesHeader } from "@/components/deal-room/DeliverablesHeader";
import { TemplatePreviewProvider } from "@/components/deal-room/TemplatePreviewStore";

const { importPptxTemplate, listTemplatePreviews } = vi.hoisted(() => ({
  importPptxTemplate: vi.fn(),
  listTemplatePreviews: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: { api: { importPptxTemplate, listTemplatePreviews } },
}));

beforeEach(() => {
  importPptxTemplate.mockReset();
  listTemplatePreviews.mockReset();
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("DeliverablesHeader", () => {
  it("renders the navy templates action and forwards navigation", async () => {
    const user = userEvent.setup();
    const onViewTemplates = vi.fn();
    render(<DeliverablesHeader mode="deliverables" onViewTemplates={onViewTemplates} />);

    expect(screen.getByRole("heading", { level: 1, name: "Deliverables" })).not.toBeNull();
    const viewTemplatesButton = screen.getByRole("button", { name: "View Templates" });
    expect(viewTemplatesButton.className).toContain("bg-primary-container");
    expect(viewTemplatesButton.className).toContain("text-white");
    expect(viewTemplatesButton.dataset.size).toBe("sm");

    await user.click(viewTemplatesButton);
    expect(onViewTemplates).toHaveBeenCalledOnce();
  });

  it.each([
    ["Import Slide Template", "single", 0],
    ["Import Deck Template", "batch", 1],
  ] as const)("imports from the %s header action", async (label, mode, inputIndex) => {
    listTemplatePreviews.mockResolvedValue(templatePage(["template-1"]));
    importPptxTemplate.mockResolvedValue({ importMode: mode, importedCount: 1, warningCount: 0 });
    const user = userEvent.setup();
    const onBack = vi.fn();
    const inputClick = vi.spyOn(HTMLInputElement.prototype, "click");
    const view = render(
      <TemplatePreviewProvider requestKey="request-1">
        <DeliverablesHeader mode="templates" onBack={onBack} />
      </TemplatePreviewProvider>,
    );

    expect(await screen.findByRole("button", { name: label })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Import Slide Template" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Import Deck Template" })).not.toBeNull();

    const input = view.container.querySelectorAll<HTMLInputElement>('input[type="file"]')[inputIndex];
    expect(input).not.toBeNull();
    if (!input) throw new Error("Expected the template file input to render.");
    expect(input.accept).toBe(".pptx");
    expect(input.multiple).toBe(false);

    await user.click(screen.getByRole("button", { name: label }));
    expect(inputClick).toHaveBeenCalledOnce();

    fireEvent.change(input, {
      target: {
        files: [
          new File(
            ["presentation"],
            "template.pptx",
            { type: "application/vnd.openxmlformats-officedocument.presentationml.presentation" },
          ),
        ],
      },
    });

    await waitFor(() => expect(importPptxTemplate).toHaveBeenCalledWith(expect.any(File), mode));
    expect(listTemplatePreviews).toHaveBeenCalledTimes(2);
    await user.click(screen.getByRole("button", { name: "Back to Deliverables" }));
    expect(onBack).toHaveBeenCalledOnce();
  });

  it("keeps the import action only in the gallery for an empty catalog", async () => {
    listTemplatePreviews.mockResolvedValue(templatePage([]));
    render(
      <TemplatePreviewProvider requestKey="request-1">
        <DeliverablesHeader mode="templates" onBack={vi.fn()} />
        <DeliverableTemplatesView onRetry={vi.fn()} />
      </TemplatePreviewProvider>,
    );

    expect(await screen.findByRole("heading", { level: 3, name: "No slide templates yet" })).not.toBeNull();
    expect(screen.getAllByRole("button", { name: "Import Slide Template" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "Import Deck Template" })).toHaveLength(1);
    expect(listTemplatePreviews).toHaveBeenCalledOnce();
  });
});

function templatePage(ids: string[]) {
  return {
    pagination: {
      hasNextPage: false,
      hasPreviousPage: false,
      page: 1,
      pageSize: 10,
      totalItems: ids.length,
      totalPages: ids.length === 0 ? 0 : 1,
    },
    previews: ids.map((templateId) => ({
      contentType: "image/png" as const,
      dataUrl: `data:image/png;base64,${btoa(templateId)}`,
      height: 900,
      previewUrl: `/templates/${templateId}/preview`,
      templateId,
      width: 1600,
    })),
  };
}

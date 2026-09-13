// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MAX_PPTX_TEMPLATE_IMPORT_BYTES, type TemplatePreviewPage } from "@/contracts/quarryApi";

const { deleteTemplate, importPptxTemplate, listTemplatePreviews } = vi.hoisted(() => ({
  deleteTemplate: vi.fn(),
  importPptxTemplate: vi.fn(),
  listTemplatePreviews: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: { api: { deleteTemplate, importPptxTemplate, listTemplatePreviews } },
}));

import { DeliverableTemplatesView } from "@/components/deal-room/DeliverableTemplatesView";
import { DeliverablesHeader } from "@/components/deal-room/DeliverablesHeader";
import { TemplatePreviewProvider } from "@/components/deal-room/TemplatePreviewStore";

beforeEach(() => {
  deleteTemplate.mockReset();
  importPptxTemplate.mockReset();
  listTemplatePreviews.mockReset();
});
afterEach(cleanup);

describe("DeliverableTemplatesView", () => {
  it("shows a labelled carousel skeleton while loading", async () => {
    const request = deferred<TemplatePreviewPage>();
    listTemplatePreviews.mockReturnValue(request.promise);
    renderView();

    expect(screen.getAllByRole("status")).toHaveLength(1);
    expect(screen.getByText("Loading template previews")).not.toBeNull();
    expect(listTemplatePreviews).toHaveBeenCalledTimes(1);

    request.resolve(page(1, 0, 0, []));
    expect(await screen.findByRole("heading", { level: 3, name: "No slide templates yet" })).not.toBeNull();
  });

  it("loads every page sequentially and renders templates in stable order", async () => {
    listTemplatePreviews
      .mockResolvedValueOnce(page(1, 2, 2, ["first-template"]))
      .mockResolvedValueOnce(page(2, 2, 2, ["second_template"]));

    renderView();

    const carousel = await screen.findByRole("region", { name: "Template preview carousel" });
    const images = within(carousel).getAllByRole("img");
    expect(images.map((image) => image.getAttribute("alt"))).toEqual([
      "Template preview: first template",
      "Template preview: second template",
    ]);
    expect(images[0]).toHaveProperty("loading", "lazy");
    expect(images[0]).toHaveProperty("decoding", "async");
    expect(images[0].getAttribute("src")).toBe("data:image/png;base64,Zmlyc3QtdGVtcGxhdGU=");
    expect(listTemplatePreviews.mock.calls).toEqual([[1], [2]]);
    expect(screen.queryByRole("heading", { name: "No slide templates yet" })).toBeNull();
  });

  it("renders the empty state only for a successful empty catalog", async () => {
    listTemplatePreviews.mockResolvedValue(page(1, 0, 0, []));
    const view = renderView();

    expect(await screen.findByRole("heading", { level: 3, name: "No slide templates yet" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Import Slide Template" })).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();

    const input = view.container.querySelector<HTMLInputElement>('input[type="file"]');
    expect(input?.accept).toBe(".pptx");
    expect(input?.multiple).toBe(false);
  });

  it("rejects a failed later page without rendering partial results and requests a retry", async () => {
    const onRetry = vi.fn();
    listTemplatePreviews
      .mockResolvedValueOnce(page(1, 2, 2, ["partial-template"]))
      .mockRejectedValueOnce(new Error("upstream details"));
    renderView({ onRetry });

    expect(await screen.findByText("Template previews could not be loaded.")).not.toBeNull();
    expect(screen.queryByAltText("Template preview: partial template")).toBeNull();
    expect(screen.queryByText("upstream details")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(onRetry).toHaveBeenCalledOnce();
  });

  it("starts a fresh request when the memoization key changes", async () => {
    listTemplatePreviews
      .mockRejectedValueOnce(new Error("temporarily unavailable"))
      .mockResolvedValueOnce(page(1, 0, 0, []));
    const view = renderView();

    expect(await screen.findByText("Template previews could not be loaded.")).not.toBeNull();

    view.rerender(
      <TemplatePreviewProvider requestKey="request-2">
        <DeliverableTemplatesView onRetry={vi.fn()} />
      </TemplatePreviewProvider>,
    );

    expect(await screen.findByRole("heading", { level: 3, name: "No slide templates yet" })).not.toBeNull();
    expect(listTemplatePreviews.mock.calls).toEqual([[1], [1]]);
  });

  it("confirms and removes a template after the delete API succeeds", async () => {
    listTemplatePreviews.mockResolvedValue(page(1, 2, 1, ["first-template", "second-template"], 10));
    deleteTemplate.mockResolvedValue(undefined);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    renderView();

    await userEvent.click(await screen.findByRole("button", { name: "Delete first template" }));

    expect(window.confirm).toHaveBeenCalledWith(
      "Delete “first template”? This cannot be undone.",
    );
    expect(deleteTemplate).toHaveBeenCalledWith("first-template");
    await waitFor(() => {
      expect(screen.queryByAltText("Template preview: first template")).toBeNull();
    });
    expect(screen.getByAltText("Template preview: second template")).not.toBeNull();
  });

  it("keeps the template and shows a sanitized message when deletion fails", async () => {
    listTemplatePreviews.mockResolvedValue(page(1, 1, 1, ["first-template"]));
    deleteTemplate.mockRejectedValue(new Error("private upstream details"));
    vi.spyOn(window, "confirm").mockReturnValue(true);
    renderView();

    await userEvent.click(await screen.findByRole("button", { name: "Delete first template" }));

    expect((await screen.findByRole("alert")).textContent).toBe(
      "The template could not be deleted.",
    );
    expect(screen.getByAltText("Template preview: first template")).not.toBeNull();
    expect(screen.queryByText("private upstream details")).toBeNull();
    expect(screen.getByRole("button", { name: "Delete first template" })).toHaveProperty(
      "disabled",
      false,
    );
  });

  it.each([
    ["changed pagination", [page(1, 2, 2, ["one"]), page(2, 3, 2, ["two"], 2)]],
    ["duplicate IDs", [page(1, 2, 2, ["same"]), page(2, 2, 2, ["same"])]],
    ["final total mismatch", [page(1, 2, 1, ["one"], 10)]],
    ["excessive page count", [page(1, 101, 101, ["one"])]],
  ])("rejects %s", async (_name, responses) => {
    for (const response of responses) listTemplatePreviews.mockResolvedValueOnce(response);
    renderView();

    expect(await screen.findByText("Template previews could not be loaded.")).not.toBeNull();
    expect(screen.queryByRole("region", { name: "Template preview carousel" })).toBeNull();
  });

  it("does not render a completed request after unmount", async () => {
    const request = deferred<TemplatePreviewPage>();
    listTemplatePreviews.mockReturnValue(request.promise);
    const view = renderView();
    view.unmount();

    request.resolve(page(1, 0, 0, []));
    await request.promise;
    await waitFor(() => expect(screen.queryByRole("heading", { name: "No slide templates yet" })).toBeNull());
  });

  it("imports a deck from the empty state, disables both actions, and renders the refreshed catalog", async () => {
    const request = deferred<{ importMode: "batch"; importedCount: number; warningCount: number }>();
    listTemplatePreviews
      .mockResolvedValueOnce(page(1, 0, 0, []))
      .mockResolvedValueOnce(page(1, 1, 1, ["new-template"]));
    importPptxTemplate.mockReturnValue(request.promise);
    const view = renderView();
    await screen.findByRole("heading", { name: "No slide templates yet" });
    const inputs = view.container.querySelectorAll<HTMLInputElement>('input[type="file"]');

    fireEvent.change(inputs[1], {
      target: { files: [powerPointFile("deck.pptx")] },
    });

    expect(importPptxTemplate).toHaveBeenCalledWith(expect.any(File), "batch");
    expect(screen.getByText("Importing deck…")).not.toBeNull();
    expect(screen.getByRole("button", { name: "Import Slide Template" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Import Deck Template" })).toHaveProperty("disabled", true);

    request.resolve({ importMode: "batch", importedCount: 2, warningCount: 1 });
    expect(await screen.findByAltText("Template preview: new template")).not.toBeNull();
    expect(screen.getByText(/Imported 2 templates\..*completed with 1 warning/)).not.toBeNull();
    expect(screen.getByText(/Templates without generated previews may not appear/)).not.toBeNull();
  });

  it("rejects an empty PowerPoint locally without calling the API", async () => {
    listTemplatePreviews.mockResolvedValue(page(1, 0, 0, []));
    const view = renderView();
    await screen.findByRole("heading", { name: "No slide templates yet" });

    fireEvent.change(view.container.querySelectorAll<HTMLInputElement>('input[type="file"]')[0], {
      target: { files: [new File([], "empty.pptx")] },
    });

    expect((await screen.findByRole("alert")).textContent).toContain("Choose a non-empty PowerPoint file.");
    expect(importPptxTemplate).not.toHaveBeenCalled();
  });

  it.each([
    ["notes.txt", 12, "not an accepted file type"],
    ["oversized.pptx", MAX_PPTX_TEMPLATE_IMPORT_BYTES + 1, "maximum size of 25MB"],
  ] as const)("rejects invalid local upload %s", async (name, size, message) => {
    listTemplatePreviews.mockResolvedValue(page(1, 0, 0, []));
    const view = renderView();
    await screen.findByRole("heading", { name: "No slide templates yet" });
    const file = powerPointFile(name);
    Object.defineProperty(file, "size", { value: size });

    fireEvent.change(view.container.querySelectorAll<HTMLInputElement>('input[type="file"]')[0], {
      target: { files: [file] },
    });

    expect((await screen.findByRole("alert")).textContent).toContain(message);
    expect(importPptxTemplate).not.toHaveBeenCalled();
  });

  it("keeps existing previews and offers reload-only recovery when refresh fails after import", async () => {
    listTemplatePreviews
      .mockResolvedValueOnce(page(1, 1, 1, ["old-template"]))
      .mockRejectedValueOnce(new Error("refresh failed"))
      .mockResolvedValueOnce(page(1, 1, 1, ["new-template"]));
    importPptxTemplate.mockResolvedValue({ importMode: "single", importedCount: 1, warningCount: 0 });
    const view = renderHeaderAndView();
    await screen.findByAltText("Template preview: old template");

    fireEvent.change(view.container.querySelectorAll<HTMLInputElement>('input[type="file"]')[0], {
      target: { files: [powerPointFile("slide.pptx")] },
    });

    expect((await screen.findByRole("alert")).textContent).toContain(
      "The import completed, but the template gallery could not be refreshed.",
    );
    expect(screen.getByAltText("Template preview: old template")).not.toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Refresh templates" }));
    expect(await screen.findByAltText("Template preview: new template")).not.toBeNull();
    expect(importPptxTemplate).toHaveBeenCalledTimes(1);
  });

  it("directs a rejected multi-slide single import to the deck action without retrying", async () => {
    listTemplatePreviews.mockResolvedValue(page(1, 1, 1, ["old-template"]));
    importPptxTemplate.mockRejectedValue(
      new Error("This PowerPoint contains multiple slides. Use Import Deck Template instead."),
    );
    const view = renderHeaderAndView();
    await screen.findByAltText("Template preview: old template");

    fireEvent.change(view.container.querySelectorAll<HTMLInputElement>('input[type="file"]')[0], {
      target: { files: [powerPointFile("deck.pptx")] },
    });

    expect((await screen.findByRole("alert")).textContent).toContain("Use Import Deck Template instead.");
    expect(importPptxTemplate).toHaveBeenCalledOnce();
    expect(importPptxTemplate).toHaveBeenCalledWith(expect.any(File), "single");
    expect(listTemplatePreviews).toHaveBeenCalledOnce();
  });
});

function renderView({ onRetry = vi.fn() }: { onRetry?: () => void } = {}) {
  return render(
    <TemplatePreviewProvider requestKey="request-1">
      <DeliverableTemplatesView onRetry={onRetry} />
    </TemplatePreviewProvider>,
  );
}

function renderHeaderAndView() {
  return render(
    <TemplatePreviewProvider requestKey="request-with-header">
      <DeliverablesHeader mode="templates" onBack={vi.fn()} />
      <DeliverableTemplatesView onRetry={vi.fn()} />
    </TemplatePreviewProvider>,
  );
}

function powerPointFile(name: string) {
  return new File(["presentation"], name, {
    type: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  });
}

function page(
  pageNumber: number,
  totalItems: number,
  totalPages: number,
  ids: string[],
  pageSize = totalItems === 0 ? 10 : 1,
): TemplatePreviewPage {
  return {
    pagination: {
      hasNextPage: pageNumber < totalPages,
      hasPreviousPage: pageNumber > 1,
      page: pageNumber,
      pageSize,
      totalItems,
      totalPages,
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

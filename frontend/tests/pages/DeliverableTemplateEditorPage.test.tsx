// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Outlet, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DiligenceCanvasDocument } from "@/contracts/diligenceCanvas";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";
import { DeliverableTemplateEditorPage } from "@/pages/deal-room/DeliverableTemplateEditorPage";

const { exportPowerPoint, getTemplate, savePowerPoint } = vi.hoisted(() => ({
  exportPowerPoint: vi.fn(),
  getTemplate: vi.fn(),
  savePowerPoint: vi.fn(),
}));
vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { exportPowerPoint, getTemplate },
    platform: { savePowerPoint },
  },
}));
vi.mock("@/lib/diligence-canvas/DiligenceCanvas", () => ({
  DiligenceCanvas: ({
    isJsonOpen,
    jsonPanelId,
    onChange,
    showPresentationHeader,
    value,
  }: {
    isJsonOpen?: boolean;
    jsonPanelId?: string;
    onChange: (document: DiligenceCanvasDocument) => void;
    showPresentationHeader?: boolean;
    value: DiligenceCanvasDocument;
  }) => (
    <div>
      <p>Canvas: {value.presentation.title}</p>
      <p>Canvas header: {showPresentationHeader === false ? "hidden" : "visible"}</p>
      <p id={jsonPanelId}>JSON panel: {isJsonOpen ? "open" : "closed"}</p>
      <button
        onClick={() => onChange({
          ...value,
          presentation: { ...value.presentation, title: "Edited" },
        })}
        type="button"
      >
        Edit canvas
      </button>
    </div>
  ),
}));

beforeEach(() => {
  exportPowerPoint.mockReset();
  getTemplate.mockReset();
  savePowerPoint.mockReset();
});
afterEach(cleanup);

describe("DeliverableTemplateEditorPage", () => {
  it("loads the decoded route ID and keeps the editable name local to the header", async () => {
    const request = deferred<DiligenceCanvasDocument>();
    getTemplate.mockReturnValue(request.promise);
    renderEditor("template%2Fone");

    expect(screen.getByText("Loading template document")).not.toBeNull();
    request.resolve(makeDocument("Template One"));

    expect(await screen.findByText("Canvas: Template One")).not.toBeNull();
    expect(getTemplate).toHaveBeenCalledWith("template/one");
    expect(screen.queryByText("Edits are local to this view and are not saved.")).toBeNull();
    const nameText = screen.getByRole("button", { name: "Edit deliverable name: Unnamed" });
    expect(nameText.textContent).toBe("Unnamed");
    await userEvent.dblClick(nameText);
    const nameInput = screen.getByRole("textbox", { name: "Deliverable name" });
    expect(nameInput.getAttribute("value")).toBe("Unnamed");
    await userEvent.clear(nameInput);
    await userEvent.type(nameInput, "Technology diligence{Enter}");
    expect(screen.getByRole("button", { name: "Edit deliverable name: Technology diligence" }).textContent)
      .toBe("Technology diligence");
    expect(getTemplate).toHaveBeenCalledOnce();
    expect(exportPowerPoint).not.toHaveBeenCalled();
    expect(screen.getByText("Canvas header: hidden")).not.toBeNull();
    expect(screen.getByText("JSON panel: closed")).not.toBeNull();

    const showJsonButton = screen.getByRole("button", { name: "Show JSON" });
    expect(showJsonButton.dataset.size).toBe("sm");
    await userEvent.click(showJsonButton);
    expect(screen.getByText("JSON panel: open")).not.toBeNull();
    expect(screen.getByRole("button", { name: "Hide JSON" })).not.toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Edit canvas" }));
    expect(screen.getByText("Canvas: Edited")).not.toBeNull();
    expect(screen.queryByText(/Locally changed.*discarded/)).toBeNull();

    exportPowerPoint.mockResolvedValue({
      dataBase64: "UEsDBA==",
      fileName: "Edited.pptx",
      mimeType: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
      warningCount: 1,
    });
    savePowerPoint.mockResolvedValue(true);
    const exportButton = screen.getByRole("button", { name: "Export" });
    expect(exportButton.dataset.size).toBe("sm");
    await userEvent.click(exportButton);

    expect(exportPowerPoint).toHaveBeenCalledWith(expect.objectContaining({
      presentation: expect.objectContaining({ title: "Edited" }),
    }));
    expect(savePowerPoint).toHaveBeenCalledWith({
      dataBase64: "UEsDBA==",
      suggestedName: "Edited.pptx",
      title: "Save PowerPoint presentation",
    });
    expect(await screen.findByText("PowerPoint exported with 1 export warning.")).not.toBeNull();
  });

  it("returns explicitly to the template gallery", async () => {
    getTemplate.mockResolvedValue(makeDocument("Example"));
    renderEditor("example");

    await screen.findByText("Canvas: Example");
    await userEvent.click(screen.getByRole("button", { name: "Back to Templates" }));
    expect(screen.getByText("Template gallery")).not.toBeNull();
  });

  it("prevents duplicate exports and reports a failed request", async () => {
    getTemplate.mockResolvedValue(makeDocument("Example"));
    const request = deferred<never>();
    exportPowerPoint.mockReturnValue(request.promise);
    renderEditor("example");

    await screen.findByText("Canvas: Example");
    const exportButton = screen.getByRole("button", { name: "Export" });
    await userEvent.click(exportButton);

    const pendingButton = screen.getByRole("button", { name: "Exporting…" });
    expect(pendingButton.hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("Building PowerPoint…")).not.toBeNull();
    await userEvent.click(pendingButton);
    expect(exportPowerPoint).toHaveBeenCalledOnce();

    request.reject(new Error("upstream unavailable"));
    expect((await screen.findByRole("alert")).textContent).toContain(
      "The PowerPoint could not be exported. Try again.",
    );
  });
});

function renderEditor(templateId: string) {
  const deal = workspaceDeals[0];
  return render(
    <MemoryRouter initialEntries={[`/hub/deals/${deal.room.id}/deliverables/templates/${templateId}`]}>
      <Routes>
        <Route
          element={<Outlet context={{ deal, navigationState: {} }} />}
          path="/hub/deals/:dealId"
        >
          <Route
            element={<DeliverableTemplateEditorPage />}
            path="deliverables/templates/:templateId"
          />
          <Route element={<div>Template gallery</div>} path="deliverables/templates" />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

function makeDocument(title: string): DiligenceCanvasDocument {
  return {
    presentation: {
      preserveElementOrder: true,
      showBranding: false,
      slides: [{
        backgroundColor: "FFFFFF",
        elements: [],
        height: 720,
        id: "slide-1",
        name: "Slide 1",
        width: 1280,
      }],
      title,
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((fulfill, fail) => {
    resolve = fulfill;
    reject = fail;
  });
  return { promise, reject, resolve };
}

import { describe, expect, it, vi } from "vitest";
import {
  createTauriQuarryApi,
  type TauriQueryPayload,
} from "@/api/tauriQuarryApi";

describe("createTauriQuarryApi", () => {
  it("uses the versioned template preview path and preserves its response", async () => {
    const payload = {
      pagination: {
        hasNextPage: false,
        hasPreviousPage: false,
        page: 2,
        pageSize: 10,
        totalItems: 11,
        totalPages: 2,
      },
      previews: [],
    };
    const get = vi.fn().mockResolvedValue(payload);
    const api = createTauriQuarryApi({
      delete: vi.fn(),
      get,
      getPdf: vi.fn(),
      post: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    });

    await expect(api.listTemplatePreviews(2)).resolves.toEqual(payload);
    expect(get).toHaveBeenCalledWith("/api/v1/templates/previews?page=2");
  });

  it("deletes an encoded template through the desktop relay", async () => {
    const deleteRequest = vi.fn().mockResolvedValue(undefined);
    const api = createTauriQuarryApi({
      delete: deleteRequest,
      get: vi.fn(),
      getPdf: vi.fn(),
      post: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    });

    await expect(api.deleteTemplate("template/one")).resolves.toBeUndefined();
    expect(deleteRequest).toHaveBeenCalledWith("/api/v1/templates/template%2Fone");
  });

  it("loads and validates an encoded template through the desktop relay", async () => {
    const payload = templateDocument();
    const get = vi.fn().mockResolvedValue(payload);
    const api = createTauriQuarryApi({
      delete: vi.fn(),
      get,
      getPdf: vi.fn(),
      post: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    });

    await expect(api.getTemplate("template/one")).resolves.toBe(payload);
    expect(get).toHaveBeenCalledWith("/api/v1/templates/template%2Fone");
  });

  it("exports presentation JSON through the explicit desktop PowerPoint relay", async () => {
    const result = {
      dataBase64: "UEsDBA==",
      fileName: "Example.pptx",
      mimeType: "application/vnd.openxmlformats-officedocument.presentationml.presentation" as const,
      warningCount: 0,
    };
    const postPowerPoint = vi.fn().mockResolvedValue(result);
    const api = createTauriQuarryApi({
      delete: vi.fn(),
      get: vi.fn(),
      getPdf: vi.fn(),
      post: vi.fn(),
      postMultipart: vi.fn(),
      postPowerPoint,
      subscribeJob: vi.fn(),
    });
    const document = templateDocument();

    await expect(api.exportPowerPoint(document)).resolves.toEqual(result);
    expect(postPowerPoint).toHaveBeenCalledWith("/api/v1/templates/export", document);
  });

  it("rejects malformed template documents returned by the desktop relay", async () => {
    const api = createTauriQuarryApi({
      delete: vi.fn(),
      get: vi.fn().mockResolvedValue({ presentation: {} }),
      getPdf: vi.fn(),
      post: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    });

    await expect(api.getTemplate("example")).rejects.toThrow(
      "presentation.title must be a string",
    );
  });

  it.each(["single", "batch"] as const)(
    "imports one PPTX through the desktop %s route and normalizes an empty MIME type",
    async (importMode) => {
      const result = { importMode, importedCount: importMode === "single" ? 1 : 2, warningCount: 0 };
      const postMultipart = vi.fn().mockResolvedValue(result);
      const api = createTauriQuarryApi({
        delete: vi.fn(),
        get: vi.fn(),
        getPdf: vi.fn(),
        post: vi.fn(),
        postPowerPoint: vi.fn(),
        postMultipart,
        subscribeJob: vi.fn(),
      });
      const file = new File([new Uint8Array([1, 2, 3])], "Template.PPTX", { type: "" });

      await expect(api.importPptxTemplate(file, importMode)).resolves.toEqual(result);

      expect(postMultipart).toHaveBeenCalledWith({
        fields: [],
        files: [{
          dataBase64: "AQID",
          fieldName: "files",
          filename: "Template.PPTX",
          mimeType: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        }],
        path: `/api/v1/templates/import?mode=${importMode}`,
      });
    },
  );

  it("sends multipart document uploads to the deal-scoped path", async () => {
    const postMultipart = vi.fn().mockResolvedValue({
      documents: [],
      failed: 0,
      skipped: 0,
      succeeded: 0,
      total: 0,
    });
    const api = createTauriQuarryApi({
      delete: vi.fn(),
      get: vi.fn(),
      getPdf: vi.fn(),
      post: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart,
      subscribeJob: vi.fn(),
    });

    await api.processDocuments("DEAL / 1", " analyst@example.com ", []);

    expect(postMultipart).toHaveBeenCalledWith({
      fields: [{ name: "userId", value: "analyst@example.com" }],
      files: [],
      path: "/api/v1/deals/DEAL%20%2F%201/documents/process",
    });
  });

  it("sends deal metadata links through the desktop multipart relay", async () => {
    const postMultipart = vi.fn().mockResolvedValue({});
    const api = createTauriQuarryApi({
      delete: vi.fn(),
      get: vi.fn(),
      getPdf: vi.fn(),
      post: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart,
      subscribeJob: vi.fn(),
    });

    await api.saveDealMetadata("DEAL / 1", {
      factSheetLink: "https://example.com/fact-sheet",
      files: [],
      rlLink: "https://example.com/request-list",
      sharepointLink: null,
      sowLink: "https://example.com/sow",
    });

    expect(postMultipart).toHaveBeenCalledWith({
      fields: [
        { name: "sharepointLink", value: "" },
        { name: "sowLink", value: "https://example.com/sow" },
        { name: "factSheetLink", value: "https://example.com/fact-sheet" },
        { name: "rlLink", value: "https://example.com/request-list" },
      ],
      files: [],
      path: "/api/v1/deals/DEAL%20%2F%201/metadata",
    });
  });

  it("uses the deal-scoped document routes for lists, PDF bytes, and raw text", async () => {
    const get = vi
      .fn()
      .mockResolvedValueOnce([{ displayName: "Report.pdf", fileId: "file / 1" }])
      .mockResolvedValueOnce({
        fileName: "Report.pdf",
        sourceKind: "pdf",
        text: "Raw report text",
      });
    const getPdf = vi.fn().mockResolvedValue(new Uint8Array([37, 80, 68, 70, 45]).buffer);
    const api = createTauriQuarryApi({
      delete: vi.fn(),
      get,
      getPdf,
      post: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    });

    const documents = await api.listDealDocuments("DEAL / 1");
    const pdf = await api.getDealDocumentPdf("DEAL / 1", documents[0].fileId);
    const rawText = await api.getDealDocumentText("DEAL / 1", documents[0].fileId);

    expect(get).toHaveBeenCalledWith("/api/v1/deals/DEAL%20%2F%201/documents");
    expect(getPdf).toHaveBeenCalledWith(
      "/api/v1/deals/DEAL%20%2F%201/documents/file%20%2F%201/pdf",
    );
    expect(Array.from(pdf.bytes)).toEqual([37, 80, 68, 70, 45]);
    expect(get).toHaveBeenNthCalledWith(
      2,
      "/api/v1/deals/DEAL%20%2F%201/documents/file%20%2F%201/text",
    );
    expect(rawText.text).toBe("Raw report text");
  });

  it("maps query files and isolates server events behind the desktop transport", async () => {
    const startQuery = vi.fn(async (
      _request: unknown,
      onPayload: (payload: TauriQueryPayload) => void,
    ) => {
      onPayload({
        event: { model: "gpt-5.5", type: "started" },
        kind: "serverEvent",
        subscriptionId: "subscription",
      });
      return vi.fn();
    });
    const api = createTauriQuarryApi({
      delete: vi.fn(), get: vi.fn(), getPdf: vi.fn(), post: vi.fn(),
      postMultipart: vi.fn(), postPowerPoint: vi.fn(), startQuery, subscribeJob: vi.fn(),
    });
    const events: string[] = [];
    api.queryModel({
      context: [],
      files: [new File([new Uint8Array([1, 2, 3])], "image.png", { type: "image/png" })],
      prompt: "hello",
    }, { onEvent: (event) => events.push(event.type) });
    await vi.waitFor(() => expect(events).toEqual(["started"]));
    expect(startQuery.mock.calls[0][0]).toMatchObject({
      context: [],
      files: [{ dataBase64: "AQID", filename: "image.png", mimeType: "image/png" }],
      prompt: "hello",
    });
  });

  it("disposes a desktop query whose terminal callback fires before startup resolves", async () => {
    const cleanup = vi.fn();
    const startQuery = vi.fn(async (
      _request: unknown,
      onPayload: (payload: TauriQueryPayload) => void,
    ) => {
      onPayload({
        event: { model: "gpt-5.5", type: "started" },
        kind: "serverEvent",
        subscriptionId: "subscription",
      });
      onPayload({
        event: { response: "done", type: "completed" },
        kind: "serverEvent",
        subscriptionId: "subscription",
      });
      return cleanup;
    });
    const api = createTauriQuarryApi({
      delete: vi.fn(), get: vi.fn(), getPdf: vi.fn(), post: vi.fn(),
      postMultipart: vi.fn(), postPowerPoint: vi.fn(), startQuery, subscribeJob: vi.fn(),
    });

    const cancel = api.queryModel(
      { context: [], files: [], prompt: "hello" },
      { onEvent: vi.fn() },
    );
    await vi.waitFor(() => expect(cleanup).toHaveBeenCalledOnce());
    cancel();
    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("disposes an active desktop query after a connection error", async () => {
    const cleanup = vi.fn();
    let deliver: ((payload: TauriQueryPayload) => void) | undefined;
    const startQuery = vi.fn(async (
      _request: unknown,
      onPayload: (payload: TauriQueryPayload) => void,
    ) => {
      deliver = onPayload;
      return cleanup;
    });
    const api = createTauriQuarryApi({
      delete: vi.fn(), get: vi.fn(), getPdf: vi.fn(), post: vi.fn(),
      postMultipart: vi.fn(), postPowerPoint: vi.fn(), startQuery, subscribeJob: vi.fn(),
    });
    const onConnectionError = vi.fn();

    api.queryModel(
      { context: [], files: [], prompt: "hello" },
      { onConnectionError, onEvent: vi.fn() },
    );
    await vi.waitFor(() => expect(deliver).toBeTypeOf("function"));
    deliver?.({
      kind: "connectionError",
      message: "connection closed",
      subscriptionId: "subscription",
    });

    expect(onConnectionError).toHaveBeenCalledWith("connection closed");
    expect(cleanup).toHaveBeenCalledOnce();
  });
});

function templateDocument() {
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
      title: "Example",
    },
  };
}

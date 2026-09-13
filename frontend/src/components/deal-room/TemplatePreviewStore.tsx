import {
  createContext,
  useContext,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { runtime } from "@quarry/runtime";

import type {
  PptxTemplateImportMode,
  PptxTemplateImportResult,
  TemplatePreviewPage,
} from "@/contracts/quarryApi";
import { MAX_PPTX_TEMPLATE_IMPORT_BYTES } from "@/contracts/quarryApi";
import {
  MAX_TEMPLATE_CATALOG_ITEMS,
  MAX_TEMPLATE_PREVIEW_PAGES,
  previewToDeliverableSlide,
  type DeliverableSlide,
} from "@/data/deliverables";

type ActiveOperation =
  | { type: "delete"; templateId: string }
  | { type: "pptxImport"; importMode: PptxTemplateImportMode }
  | { type: "reload" };

type ActionFeedback = {
  canRefresh: boolean;
  message: string;
  type: "error" | "partial" | "success";
};

export type TemplatePreviewState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | {
    activeOperation: ActiveOperation | null;
    feedback: ActionFeedback | null;
    slides: readonly DeliverableSlide[];
    status: "success";
  };

type TemplatePreviewStore = {
  deleteTemplate: (templateId: string) => Promise<void>;
  getSnapshot: () => TemplatePreviewState;
  importPptxTemplate: (file: File, importMode: PptxTemplateImportMode) => Promise<void>;
  reload: () => Promise<void>;
  reportImportError: (message: string) => void;
  subscribe: (listener: () => void) => () => void;
};

const TemplatePreviewContext = createContext<TemplatePreviewStore | null>(null);

export function TemplatePreviewProvider({
  children,
  requestKey,
}: {
  children: ReactNode;
  requestKey: string;
}) {
  const store = useMemo(() => createTemplatePreviewStore(), [requestKey]);
  return (
    <TemplatePreviewContext.Provider value={store}>
      {children}
    </TemplatePreviewContext.Provider>
  );
}

export function useTemplatePreviewState() {
  const store = useContext(TemplatePreviewContext);
  if (!store) {
    throw new Error("Template preview state must be used within TemplatePreviewProvider.");
  }
  return useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
}

export function useTemplatePreviewActions() {
  const store = useContext(TemplatePreviewContext);
  if (!store) {
    throw new Error("Template preview actions must be used within TemplatePreviewProvider.");
  }
  return {
    deleteTemplate: store.deleteTemplate,
    importPptxTemplate: store.importPptxTemplate,
    reload: store.reload,
    reportImportError: store.reportImportError,
  };
}

function createTemplatePreviewStore(): TemplatePreviewStore {
  let previewState: TemplatePreviewState = { status: "loading" };
  let started = false;
  let operationVersion = 0;
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of listeners) listener();
  };

  const startLoading = () => {
    if (started) return;
    started = true;
    void loadTemplatePreviews()
      .then((slides) => {
        previewState = {
          activeOperation: null,
          feedback: null,
          slides,
          status: "success",
        };
        notify();
      })
      .catch(() => {
        previewState = {
          message: "Template previews could not be loaded.",
          status: "error",
        };
        notify();
      });
  };

  const deleteTemplate = async (templateId: string) => {
    if (
      previewState.status !== "success"
      || previewState.activeOperation !== null
      || !previewState.slides.some((slide) => slide.id === templateId)
    ) {
      return;
    }
    previewState = {
      ...previewState,
      activeOperation: { type: "delete", templateId },
      feedback: null,
    };
    notify();

    try {
      await runtime.api.deleteTemplate(templateId);
      if (previewState.status !== "success") return;
      previewState = {
        activeOperation: null,
        feedback: null,
        slides: previewState.slides.filter((slide) => slide.id !== templateId),
        status: "success",
      };
    } catch {
      if (previewState.status !== "success") return;
      previewState = {
        ...previewState,
        activeOperation: null,
        feedback: {
          canRefresh: false,
          message: "The template could not be deleted.",
          type: "error",
        },
      };
    }
    notify();
  };

  const reportImportError = (message: string) => {
    if (previewState.status !== "success" || previewState.activeOperation !== null) return;
    previewState = {
      ...previewState,
      feedback: { canRefresh: false, message, type: "error" },
    };
    notify();
  };

  const reload = async () => {
    if (previewState.status !== "success" || previewState.activeOperation !== null) return;
    const version = ++operationVersion;
    const previousFeedback = previewState.feedback;
    previewState = { ...previewState, activeOperation: { type: "reload" } };
    notify();
    try {
      const slides = await loadTemplatePreviews();
      if (version !== operationVersion || previewState.status !== "success") return;
      previewState = { activeOperation: null, feedback: null, slides, status: "success" };
    } catch {
      if (version !== operationVersion || previewState.status !== "success") return;
      previewState = {
        ...previewState,
        activeOperation: null,
        feedback: previousFeedback?.type === "partial"
          ? previousFeedback
          : {
            canRefresh: true,
            message: "Template previews could not be refreshed.",
            type: "error",
          },
      };
    }
    notify();
  };

  const importPptxTemplate = async (file: File, importMode: PptxTemplateImportMode) => {
    if (previewState.status !== "success" || previewState.activeOperation !== null) return;
    const validationError = validatePptxImportFile(file);
    if (validationError) {
      reportImportError(validationError);
      return;
    }
    const version = ++operationVersion;
    previewState = {
      ...previewState,
      activeOperation: { type: "pptxImport", importMode },
      feedback: null,
    };
    notify();

    let result: PptxTemplateImportResult;
    try {
      result = await runtime.api.importPptxTemplate(file, importMode);
    } catch (error) {
      if (version !== operationVersion || previewState.status !== "success") return;
      previewState = {
        ...previewState,
        activeOperation: null,
        feedback: {
          canRefresh: false,
          message: pptxImportErrorMessage(error, importMode),
          type: "error",
        },
      };
      notify();
      return;
    }

    try {
      const slides = await loadTemplatePreviews();
      if (version !== operationVersion || previewState.status !== "success") return;
      previewState = {
        activeOperation: null,
        feedback: {
          canRefresh: false,
          message: pptxImportSuccessMessage(result),
          type: "success",
        },
        slides,
        status: "success",
      };
    } catch {
      if (version !== operationVersion || previewState.status !== "success") return;
      previewState = {
        ...previewState,
        activeOperation: null,
        feedback: {
          canRefresh: true,
          message: `${pptxImportCountMessage(result)} The import completed, but the template gallery could not be refreshed. Refresh templates before importing again.`,
          type: "partial",
        },
      };
    }
    notify();
  };

  return {
    deleteTemplate,
    getSnapshot: () => previewState,
    importPptxTemplate,
    reload,
    reportImportError,
    subscribe: (listener) => {
      listeners.add(listener);
      startLoading();
      return () => listeners.delete(listener);
    },
  };
}

function validatePptxImportFile(file: File): string | null {
  if (!file.name.trim() || file.name.includes("/") || file.name.includes("\\")) {
    return "Choose a PowerPoint file with a valid .pptx filename.";
  }
  if (!file.name.toLowerCase().endsWith(".pptx")) {
    return "Choose a .pptx PowerPoint file.";
  }
  if (file.size === 0) return "Choose a non-empty PowerPoint file.";
  if (file.size > MAX_PPTX_TEMPLATE_IMPORT_BYTES) {
    return "Choose a PowerPoint file no larger than 25 MB.";
  }
  return null;
}

function pptxImportCountMessage(result: PptxTemplateImportResult) {
  return `Imported ${result.importedCount} ${result.importedCount === 1 ? "template" : "templates"}.`;
}

function pptxImportSuccessMessage(result: PptxTemplateImportResult) {
  const imported = pptxImportCountMessage(result);
  if (result.warningCount === 0) return imported;
  return `${imported} The import completed with ${result.warningCount} ${result.warningCount === 1 ? "warning" : "warnings"}. Templates without generated previews may not appear in this preview-only gallery.`;
}

function pptxImportErrorMessage(error: unknown, importMode: PptxTemplateImportMode) {
  const message = error instanceof Error ? error.message : "";
  if (message.includes("Use Import Deck Template instead")) {
    return "This PowerPoint contains multiple slides. Use Import Deck Template instead.";
  }
  if (message.includes("Template import could not be confirmed")) {
    return "Template import could not be confirmed. It may have completed; refresh templates before trying again.";
  }
  return importMode === "single"
    ? "The slide template could not be imported."
    : "The deck template could not be imported.";
}

async function loadTemplatePreviews(): Promise<DeliverableSlide[]> {
  const slides: DeliverableSlide[] = [];
  const seenIds = new Set<string>();
  let expectedPage = 1;
  let baseline: Pick<
    TemplatePreviewPage["pagination"],
    "pageSize" | "totalItems" | "totalPages"
  > | null = null;

  while (true) {
    const response = await runtime.api.listTemplatePreviews(expectedPage);
    const pagination = response.pagination;
    validatePage(response, expectedPage);
    baseline ??= {
      pageSize: pagination.pageSize,
      totalItems: pagination.totalItems,
      totalPages: pagination.totalPages,
    };
    if (
      pagination.pageSize !== baseline.pageSize
      || pagination.totalItems !== baseline.totalItems
      || pagination.totalPages !== baseline.totalPages
    ) {
      throw new Error("Template preview pagination changed during loading.");
    }

    for (const preview of response.previews) {
      if (seenIds.has(preview.templateId)) {
        throw new Error("Template preview IDs must be unique.");
      }
      seenIds.add(preview.templateId);
      slides.push(previewToDeliverableSlide(preview));
      if (slides.length > MAX_TEMPLATE_CATALOG_ITEMS) {
        throw new Error("Template preview catalog is too large.");
      }
    }

    if (!pagination.hasNextPage) break;
    expectedPage += 1;
  }

  if (!baseline || slides.length !== baseline.totalItems) {
    throw new Error("Template preview total did not match the catalog.");
  }
  return slides;
}

function validatePage(response: TemplatePreviewPage, expectedPage: number) {
  const { pagination, previews } = response;
  const integer = (value: number) => Number.isSafeInteger(value);
  const emptyCatalog =
    pagination.page === 1
    && pagination.totalItems === 0
    && pagination.totalPages === 0
    && previews.length === 0
    && !pagination.hasNextPage
    && !pagination.hasPreviousPage;
  const expectedTotalPages = pagination.pageSize > 0
    ? Math.ceil(pagination.totalItems / pagination.pageSize)
    : -1;
  const valid =
    integer(pagination.page)
    && integer(pagination.pageSize)
    && integer(pagination.totalItems)
    && integer(pagination.totalPages)
    && pagination.page === expectedPage
    && pagination.pageSize >= 1
    && pagination.pageSize <= 10
    && pagination.totalItems >= 0
    && pagination.totalItems <= MAX_TEMPLATE_CATALOG_ITEMS
    && pagination.totalPages >= 0
    && pagination.totalPages <= MAX_TEMPLATE_PREVIEW_PAGES
    && previews.length <= pagination.pageSize
    && (emptyCatalog
      || (pagination.totalItems > 0
        && pagination.totalPages === expectedTotalPages
        && pagination.page <= pagination.totalPages
        && pagination.hasPreviousPage === (pagination.page > 1)
        && pagination.hasNextPage === (pagination.page < pagination.totalPages)));
  if (!valid) throw new Error("Template preview pagination is invalid.");
}

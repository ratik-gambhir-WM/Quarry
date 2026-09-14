// @vitest-environment happy-dom

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DiligenceCanvasDocument } from "@/contracts/diligenceCanvas";

const { getTemplate } = vi.hoisted(() => ({ getTemplate: vi.fn() }));
vi.mock("@quarry/runtime", () => ({ runtime: { api: { getTemplate } } }));

import {
  TemplateDocumentProvider,
  useTemplateDocumentActions,
  useTemplateDocumentState,
} from "@/components/deal-room/TemplateDocumentStore";

beforeEach(() => getTemplate.mockReset());

describe("TemplateDocumentStore", () => {
  it("loads the exact template and marks replacement documents as local changes", async () => {
    getTemplate.mockResolvedValue(makeDocument("Original"));
    renderStore("template/one");

    expect(screen.getByText("loading")).not.toBeNull();
    expect(await screen.findByText("Original:clean")).not.toBeNull();
    expect(getTemplate).toHaveBeenCalledWith("template/one");

    await userEvent.click(screen.getByRole("button", { name: "Edit" }));
    expect(screen.getByText("Edited:dirty")).not.toBeNull();
  });

  it("sanitizes failures and retries with a new request generation", async () => {
    getTemplate
      .mockRejectedValueOnce(new Error("private upstream details"))
      .mockResolvedValueOnce(makeDocument("Retried"));
    renderStore("example");

    expect(await screen.findByText("This template could not be opened.")).not.toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByText("Retried:clean")).not.toBeNull();
    expect(getTemplate).toHaveBeenCalledTimes(2);
  });

  it("ignores late completion after its last subscriber unmounts", async () => {
    const request = deferred<DiligenceCanvasDocument>();
    getTemplate.mockReturnValue(request.promise);
    const view = renderStore("example");
    view.unmount();

    request.resolve(makeDocument("Late"));
    await Promise.resolve();
    expect(screen.queryByText("Late:clean")).toBeNull();
  });
});

function renderStore(templateId: string) {
  return render(
    <TemplateDocumentProvider requestKey="request-1" templateId={templateId}>
      <Probe />
    </TemplateDocumentProvider>,
  );
}

function Probe() {
  const state = useTemplateDocumentState();
  const { retry, updateDocument } = useTemplateDocumentActions();
  if (state.status === "loading") return <p>loading</p>;
  if (state.status === "error") {
    return <><p>{state.message}</p><button onClick={retry}>Retry</button></>;
  }
  return (
    <>
      <p>{state.document.presentation.title}:{state.isDirty ? "dirty" : "clean"}</p>
      <button onClick={() => updateDocument(makeDocument("Edited"))}>Edit</button>
    </>
  );
}

function makeDocument(title: string): DiligenceCanvasDocument {
  return {
    presentation: {
      preserveElementOrder: true,
      showBranding: false,
      slides: [],
      title,
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((fulfill) => { resolve = fulfill; });
  return { promise, resolve };
}

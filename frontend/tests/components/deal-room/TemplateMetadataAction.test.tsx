// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { testTemplateRetrieval } from "@/test/fixtures/templateRetrieval";

const { getTemplateRetrieval, retryTemplateProcessing } = vi.hoisted(() => ({ getTemplateRetrieval: vi.fn(), retryTemplateProcessing: vi.fn() }));
vi.mock("@quarry/runtime", () => ({ runtime: { api: { getTemplateRetrieval, retryTemplateProcessing } } }));
import { TemplateMetadataAction } from "@/components/deal-room/TemplateMetadataAction";

beforeEach(() => { getTemplateRetrieval.mockReset(); retryTemplateProcessing.mockReset(); });
afterEach(() => { cleanup(); vi.useRealTimers(); });

async function openInspector() {
  const user = userEvent.setup();
  render(<TemplateMetadataAction templateId="template-one" disabled={false} />);
  const trigger = screen.getByRole("button", { name: "More actions for template one" });
  await user.click(trigger);
  await user.click(screen.getByRole("menuitem", { name: "View metadata" }));
  return { user, trigger };
}

describe("TemplateMetadataAction", () => {
  it.each(["classification", "embedding"] as const)("retries only the failed %s stage and resumes progress", async (stage) => {
    const data = testTemplateRetrieval();
    data[stage].status = "failed";
    if (stage === "classification") data.embedding.status = "not_ready";
    delete data.embeddings;
    getTemplateRetrieval.mockResolvedValueOnce(data);
    let completeRetry: (() => void) | undefined;
    retryTemplateProcessing.mockImplementation(() => new Promise((resolve) => {
      completeRetry = () => resolve({ templateId: "template-one", stage });
    }));
    const { user } = await openInspector();
    const retry = await screen.findByRole("button", { name: "Retry" });
    await user.dblClick(retry);
    expect(retryTemplateProcessing).toHaveBeenCalledExactlyOnceWith("template-one");
    expect(screen.getByRole("button", { name: "Retrying…" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("button", { name: "Refresh" })).toHaveProperty("disabled", true);
    data[stage].status = "pending";
    getTemplateRetrieval.mockResolvedValue(data);
    await act(async () => { completeRetry?.(); });
    expect(await screen.findByText(stage === "classification" ? "Metadata is still processing" : "Embeddings are still processing")).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
  });

  it("shows a sanitized retry error and preserves saved metadata", async () => {
    const data = testTemplateRetrieval();
    data.embedding.status = "failed";
    delete data.embeddings;
    getTemplateRetrieval.mockResolvedValue(data);
    retryTemplateProcessing.mockRejectedValue(new Error("private details"));
    const { user } = await openInspector();
    await user.click(await screen.findByRole("button", { name: "Retry" }));
    expect(await screen.findByText(/Processing could not be restarted/)).not.toBeNull();
    expect(screen.getByText("Cloud architecture overview")).not.toBeNull();
    expect(screen.queryByText("private details")).toBeNull();
    expect(screen.getByRole("button", { name: "Retry" })).toHaveProperty("disabled", false);
  });

  it("ignores a retry acknowledgement after the modal closes", async () => {
    const data = testTemplateRetrieval();
    data.classification.status = "failed";
    data.embedding.status = "not_ready";
    getTemplateRetrieval.mockResolvedValue(data);
    let completeRetry: (() => void) | undefined;
    retryTemplateProcessing.mockImplementation(() => new Promise((resolve) => {
      completeRetry = () => resolve({ templateId: "template-one", stage: "classification" });
    }));
    const { user } = await openInspector();
    await user.click(await screen.findByRole("button", { name: "Retry" }));
    await user.click(screen.getByRole("button", { name: "Close" }));
    await act(async () => { completeRetry?.(); });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(1);
  });

  it.each(["ready", "pending", "processing", "not_ready"] as const)("does not offer Retry for %s jobs", async (status) => {
    const data = testTemplateRetrieval();
    data.classification.status = status;
    data.embedding.status = status;
    getTemplateRetrieval.mockResolvedValue(data);
    await openInspector();
    await waitFor(() => expect(screen.queryByText("Loading metadata…")).toBeNull());
    expect(screen.queryByRole("button", { name: "Retry" })).toBeNull();
  });

  it("loads on demand, displays full metadata and both vectors, and restores focus", async () => {
    getTemplateRetrieval.mockResolvedValue(testTemplateRetrieval());
    const { user, trigger } = await openInspector();
    const dialog = screen.getByRole("dialog", { name: "Template metadata" });
    expect(await within(dialog).findByText("Cloud architecture overview")).not.toBeNull();
    expect(within(dialog).getByText("Kubernetes")).not.toBeNull();
    expect(within(dialog).getByText("text-embedding-3-small")).not.toBeNull();
    await user.click(within(dialog).getByText("Subject vector"));
    expect(within(dialog).getByLabelText("Subject vector").textContent).toContain("-0.23");
    await user.click(within(dialog).getByText("Capability vector"));
    expect(within(dialog).getByLabelText("Capability vector").textContent).toContain("0.67");
    expect(getTemplateRetrieval.mock.calls).toEqual([["template-one"], ["template-one", true]]);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(document.activeElement).toBe(trigger);
  });

  it("shows loading while the saved metadata request is pending", async () => {
    getTemplateRetrieval.mockReturnValue(new Promise(() => {}));
    await openInspector();
    expect(screen.getByRole("status").textContent).toBe("Loading metadata…");
  });

  it("shows processing, polls until both stages complete, and stops after closing", async () => {
    vi.useFakeTimers();
    const pending = testTemplateRetrieval();
    pending.classification.status = "processing";
    pending.embedding.status = "not_ready";
    pending.metadata = null;
    delete pending.embeddings;
    getTemplateRetrieval.mockResolvedValueOnce(pending).mockResolvedValue(testTemplateRetrieval());
    render(<TemplateMetadataAction templateId="template-one" disabled={false} />);
    fireEvent.pointerDown(screen.getByRole("button", { name: /More actions/ }), { button: 0, ctrlKey: false });
    fireEvent.click(screen.getByRole("menuitem", { name: "View metadata" }));
    await act(async () => { await Promise.resolve(); });
    expect(screen.getByText("Metadata is still processing")).not.toBeNull();
    expect(screen.getByText("Embeddings are still processing")).not.toBeNull();
    await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
    expect(screen.getByText("Cloud architecture overview")).not.toBeNull();
    expect(screen.queryByText("Metadata is still processing")).toBeNull();
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(3);
    await act(async () => { await vi.advanceTimersByTimeAsync(9000); });
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(3);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(3);
  });

  it("keeps ready metadata visible while embeddings are processing", async () => {
    const partial = testTemplateRetrieval();
    partial.embedding.status = "pending";
    delete partial.embeddings;
    getTemplateRetrieval.mockResolvedValue(partial);
    await openInspector();
    expect(await screen.findByText("Cloud architecture overview")).not.toBeNull();
    expect(screen.getByText("Embeddings are still processing")).not.toBeNull();
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(1);
  });

  it("shows metadata immediately while fetching saved vector values", async () => {
    const metadata = testTemplateRetrieval();
    delete metadata.embeddings;
    getTemplateRetrieval.mockResolvedValueOnce(metadata).mockReturnValue(new Promise(() => {}));
    await openInspector();
    expect(await screen.findByText("Cloud architecture overview")).not.toBeNull();
    expect(screen.getByText("Loading embedding values…")).not.toBeNull();
  });

  it("does not fetch before opening and disables the menu during gallery operations", () => {
    render(<TemplateMetadataAction templateId="template-one" disabled />);
    expect(screen.getByRole("button", { name: /More actions/ })).toHaveProperty("disabled", true);
    expect(getTemplateRetrieval).not.toHaveBeenCalled();
  });

  it("cancels the pending processing poll when closed", async () => {
    vi.useFakeTimers();
    const data = testTemplateRetrieval();
    data.classification.status = "pending";
    data.embedding.status = "not_ready";
    data.metadata = null;
    delete data.embeddings;
    getTemplateRetrieval.mockResolvedValue(data);
    render(<TemplateMetadataAction templateId="template-one" disabled={false} />);
    fireEvent.pointerDown(screen.getByRole("button", { name: /More actions/ }), { button: 0, ctrlKey: false });
    fireEvent.click(screen.getByRole("menuitem", { name: "View metadata" }));
    await act(async () => { await Promise.resolve(); });
    expect(screen.getByText("Metadata is still processing")).not.toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(9000); });
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(1);
  });

  it("keeps metadata readable when embedding inspection is disabled", async () => {
    const metadata = testTemplateRetrieval();
    delete metadata.embeddings;
    getTemplateRetrieval.mockResolvedValueOnce(metadata).mockRejectedValue(new Error("Embedding inspection is disabled."));
    await openInspector();
    expect(await screen.findByText("Cloud architecture overview")).not.toBeNull();
    expect(screen.getByText(/their values are unavailable for inspection/)).not.toBeNull();
  });

  it("supports retry after a sanitized loading error", async () => {
    getTemplateRetrieval.mockRejectedValueOnce(new Error("private server details")).mockResolvedValue(testTemplateRetrieval());
    const { user } = await openInspector();
    expect(await screen.findByRole("alert")).not.toBeNull();
    expect(screen.queryByText("private server details")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    expect(await screen.findByText("Cloud architecture overview")).not.toBeNull();
  });

  it.each(["failed", "not_ready"] as const)("shows terminal %s without an endless processing spinner", async (status) => {
    const data = testTemplateRetrieval();
    data.classification.status = status;
    data.embedding.status = status;
    data.metadata = null;
    delete data.embeddings;
    getTemplateRetrieval.mockResolvedValue(data);
    await openInspector();
    await waitFor(() => expect(screen.queryByText("Loading metadata…")).toBeNull());
    expect(screen.queryByRole("status")).toBeNull();
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(1);
  });

  it("ignores a response after closing and fetches fresh data when reopened", async () => {
    let resolve: ((value: ReturnType<typeof testTemplateRetrieval>) => void) | undefined;
    getTemplateRetrieval.mockReturnValueOnce(new Promise((done) => { resolve = done; })).mockResolvedValue(testTemplateRetrieval());
    const { user, trigger } = await openInspector();
    await user.click(screen.getByRole("button", { name: "Close" }));
    await act(async () => { resolve?.(testTemplateRetrieval()); });
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(1);
    await user.click(trigger);
    await user.click(screen.getByRole("menuitem", { name: "View metadata" }));
    expect(await screen.findByText("Cloud architecture overview")).not.toBeNull();
    expect(getTemplateRetrieval).toHaveBeenCalledTimes(3);
  });
});

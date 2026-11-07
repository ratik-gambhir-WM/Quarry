// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DataRoomDocumentSearch } from "@/components/data-room/document-search/DataRoomDocumentSearch";
import { keywordHit } from "../../../fixtures/documents/keywordSearch";
import { hybridResponse, mixedSourceResponse } from "../../../fixtures/documents/hybridSearch";
import type { HybridDocumentResponse } from "@/contracts/quarryApi";

const { search, keyword } = vi.hoisted(() => ({ search: vi.fn(), keyword: vi.fn() }));
vi.mock("@quarry/runtime", () => ({ runtime: { api: { searchDealDocumentChunksHybrid: search, searchDealDocumentChunksByKeyword: keyword } } }));
afterEach(() => { cleanup(); vi.useRealTimers(); });
beforeEach(() => { search.mockReset().mockResolvedValue(hybridResponse()); keyword.mockReset().mockResolvedValue([keywordHit()]); });

function mount(onActivateResult = vi.fn()) {
  return render(<DataRoomDocumentSearch dealId="DEAL-1" currentFileId="file-1"
    currentFileName="Synthetic_Terms.pdf" currentPageCount={3}
    files={[{ fileId: "file-1", name: "Synthetic_Terms.pdf" }, { fileId: "file-2", name: "Other.pdf" }]}
    onActivateResult={onActivateResult} open />);
}

it.each([
  ["on", "Confidential provisions on liability", ["on"]],
  ["Ruby, Rails", "Ruby on Rails", ["Ruby", "Rails"]],
  ["C", "C, C++ and C# services", ["C"]],
  ["C++", "C, C++ and C# services", ["C++"]],
  ["C#", "C, C++ and C# services", ["C#"]],
  ["CAFÉ", "A café service", ["café"]],
  ["liability", "Background context. ".repeat(40) + "\nThe liability provision. " + "More context. ".repeat(40), ["liability"]],
] as const)("highlights whole keyword tokens for %s in results and preview", async (query, text, expected) => {
  search.mockResolvedValue(hybridResponse([keywordHit({ text })]));
  mount();
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: query } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  const option = await screen.findByRole("option");
  const preview = screen.getByRole("complementary", { name: "Preview", hidden: true });
  for (const surface of [option, preview]) {
    expect([...surface.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(expected);
  }
  expect(within(preview).getByRole("heading").querySelector("mark")).toBeNull();
});

it("uses whole-token ranges in file names without truncating or normalizing them", async () => {
  search.mockResolvedValue(hybridResponse([keywordHit({ text: "The café uses C++ and C#." })]));
  const fileName = "📄 café_C++_C#_Confidential.pdf";
  render(<DataRoomDocumentSearch dealId="DEAL-1" currentFileId="file-1"
    currentFileName={fileName} currentPageCount={3} onActivateResult={vi.fn()} open />);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "CAFÉ, C++" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  const option = await screen.findByRole("option");
  const heading = within(screen.getByRole("complementary", { name: "Preview", hidden: true })).getByRole("heading");
  expect(heading.textContent).toBe(fileName);
  expect([...heading.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(["café", "C++"]);
  expect([...option.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(["café", "C++", "café", "C++"]);
});

it("submits once on Enter, never while typing, and activates an actual page", async () => {
  const activate = vi.fn(); mount(activate);
  const input = screen.getByRole("searchbox");
  fireEvent.change(input, { target: { value: "liab" } });
  fireEvent.change(input, { target: { value: "liability" } });
  await new Promise((resolve) => setTimeout(resolve, 300));
  expect(search).not.toHaveBeenCalled();
  expect(screen.queryByText(/No results for/)).toBeNull();
  fireEvent.keyDown(input, { key: "Enter" });
  const option = await screen.findByRole("option", { name: /Synthetic_Terms/ });
  expect(search).toHaveBeenCalledExactlyOnceWith("DEAL-1", { queryText: "liability", limit: 30 }, expect.any(AbortSignal));
  expect(option.textContent).toContain("Keyword"); expect(option.textContent).toContain("Semantic");
  fireEvent.click(option);
  expect(activate).toHaveBeenCalledWith(expect.objectContaining({ target: { kind: "pdf-page", page: 2 } }));
});

it("does not refilter API matches by filename or truncated excerpt", async () => {
  search.mockResolvedValue(hybridResponse([keywordHit({ text: "First keyword here. " + "Context. ".repeat(90) + "Second keyword here." })]));
  mount();
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "First Second" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  expect(await screen.findByRole("option", { name: /Synthetic_Terms/ })).toBeTruthy();
});

it("uses file identity to disable cross-document and missing-page navigation", async () => {
  search.mockResolvedValue(hybridResponse([
    keywordHit({ file_id: "file-2" }),
    keywordHit({ chunk_id: "no-page", page_start: null, page_end: null }),
    keywordHit({ chunk_id: "out-of-bounds", page_start: 4, page_end: 4 }),
  ]));
  const activate = vi.fn();
  mount(activate);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "liability" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  const options = await screen.findAllByRole("option");
  expect(options).toHaveLength(3);
  for (const option of options) {
    expect(option.getAttribute("aria-disabled")).toBe("true");
    fireEvent.click(option);
  }
  expect(activate).not.toHaveBeenCalled();
  expect(options[0].textContent).toContain("Other.pdf");
});

it("ignores stale responses after typing or clearing and skips blank queries", async () => {
  let finish: (hits: HybridDocumentResponse) => void = () => {};
  search.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  mount();
  const input = screen.getByRole("searchbox");
  fireEvent.change(input, { target: { value: "old" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await waitFor(() => expect(search).toHaveBeenCalledTimes(1));
  fireEvent.change(input, { target: { value: "new" } });
  await act(async () => finish(hybridResponse([keywordHit({ text: "Old private passage" })])));
  expect(screen.queryByText(/Old private passage/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
  await new Promise((resolve) => setTimeout(resolve, 300));
  expect(search).toHaveBeenCalledTimes(1);
  expect(screen.queryAllByRole("option")).toHaveLength(0);
  expect(input.hasAttribute("aria-activedescendant")).toBe(false);
});

it("shows failures explicitly and retries without fixture fallback", async () => {
  const user = userEvent.setup();
  search.mockRejectedValueOnce(new Error("Keyword search unavailable"));
  mount();
  await user.type(screen.getByRole("searchbox"), "liability");
  await user.click(screen.getByRole("button", { name: "Search" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Keyword search unavailable");
  expect(screen.queryByText(/No results for/)).toBeNull();
  await user.click(screen.getByRole("button", { name: "Retry search" }));
  expect(await screen.findByRole("option", { name: /Synthetic_Terms/ })).toBeTruthy();
});

it("searches without a selected document and reports an empty API result", async () => {
  search.mockResolvedValue(hybridResponse([]));
  render(<DataRoomDocumentSearch dealId="DEAL-1" currentFileName="" currentPageCount={0} onActivateResult={vi.fn()} open />);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "missing" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  expect(await screen.findByText(/No results for/)).toBeTruthy();
  expect(search).toHaveBeenCalledWith("DEAL-1", { queryText: "missing", limit: 30 }, expect.any(AbortSignal));
  expect(screen.queryByText("Sample excerpts")).toBeNull();
});

it("discards an in-flight response when the panel closes", async () => {
  let finish: (hits: HybridDocumentResponse) => void = () => {};
  search.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const props = { dealId: "DEAL-1", currentFileName: "", currentPageCount: 0, onActivateResult: vi.fn() };
  const view = render(<DataRoomDocumentSearch {...props} open />);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "old" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await waitFor(() => expect(search).toHaveBeenCalledTimes(1));
  view.rerender(<DataRoomDocumentSearch {...props} open={false} />);
  await act(async () => finish(hybridResponse([keywordHit({ text: "Closed-panel result" })])));
  view.rerender(<DataRoomDocumentSearch {...props} open />);
  expect(screen.queryByText(/Closed-panel result/)).toBeNull();
  expect(screen.queryAllByRole("option")).toHaveLength(0);
});

it("offers keyword search explicitly after semantic unavailability", async () => {
  search.mockRejectedValueOnce(Object.assign(new Error("Semantic search unavailable"), { status: 503 }));
  mount(); fireEvent.change(screen.getByRole("searchbox"), { target: { value: "liability" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await screen.findByRole("alert"); expect(keyword).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Use keyword search" }));
  const option = await screen.findByRole("option");
  expect(option.textContent).toContain("Keyword"); expect(option.textContent).not.toContain("Semantic");
  expect(keyword).toHaveBeenCalledExactlyOnceWith("DEAL-1", { queryText: "liability", limit: 30 });
  expect(screen.queryByRole("group", { name: "Filter results by match type" })).toBeNull();
});

it("separates sources with truthful overlapping counts, preview labels and keyboard navigation without another request", async () => {
  const activate = vi.fn();
  search.mockResolvedValue(mixedSourceResponse());
  mount(activate);
  const input = screen.getByRole("searchbox");
  fireEvent.change(input, { target: { value: "liability" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await screen.findAllByRole("option");
  const filters = within(screen.getByRole("group", { name: "Filter results by match type" }));
  expect(filters.getByRole("button", { name: "All results 3" }).getAttribute("aria-pressed")).toBe("true");
  expect(filters.getByRole("button", { name: "Keyword 2" })).toBeTruthy();
  expect(filters.getByRole("button", { name: "Semantic 2" })).toBeTruthy();
  fireEvent.click(filters.getByRole("button", { name: "Keyword 2" }));
  expect(screen.getAllByRole("option").map((item) => item.id)).toEqual(["local-search-result-literal", "local-search-result-overlap"]);
  const preview = within(screen.getByRole("complementary", { name: "Preview", hidden: true }));
  expect(preview.getByText("Keyword match")).toBeTruthy();
  fireEvent.click(filters.getByRole("button", { name: "Semantic 2" }));
  expect(screen.getAllByRole("option").map((item) => item.id)).toEqual(["local-search-result-related", "local-search-result-overlap"]);
  expect(preview.getByText("Semantic match")).toBeTruthy();
  expect(screen.getByRole("status").textContent).toBe("2 semantic results");
  fireEvent.keyDown(input, { key: "ArrowDown" });
  expect(preview.getByText("Keyword + Semantic match")).toBeTruthy();
  expect(search).toHaveBeenCalledTimes(1);
  fireEvent.keyDown(input, { key: "Enter" });
  expect(activate).toHaveBeenCalledWith(expect.objectContaining({ id: "overlap", target: { kind: "pdf-page", page: 3 } }));
});

it("distinguishes an empty source view from no search results and restores All results without refetching", async () => {
  const response = hybridResponse(); response.results[0].matchSources = ["semantic"];
  search.mockResolvedValue(response); mount();
  const input = screen.getByRole("searchbox");
  fireEvent.change(input, { target: { value: "related concept" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await screen.findByRole("option");
  fireEvent.click(screen.getByRole("button", { name: "Keyword 0" }));
  expect(screen.queryAllByRole("option")).toHaveLength(0);
  expect(screen.getByText("No keyword matches for “related concept”")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Show all results" }));
  expect(screen.getAllByRole("option")).toHaveLength(1);
  expect(document.activeElement).toBe(input);
  expect(search).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "Semantic 1" }));
  fireEvent.change(input, { target: { value: "next query" } });
  expect(screen.queryAllByRole("option")).toHaveLength(0);
  expect(screen.getByRole("button", { name: "All results" }).getAttribute("aria-pressed")).toBe("true");
  expect(screen.getByRole("button", { name: "Semantic" }).hasAttribute("disabled")).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await screen.findByRole("option");
  expect(screen.getByRole("button", { name: "All results 1" }).getAttribute("aria-pressed")).toBe("true");
});
it("does not invent literal highlights for semantic-only passages", async () => {
  const response = hybridResponse([keywordHit({ text: "Contracts expire without extension." })]);
  response.results[0].matchSources = ["semantic"];
  search.mockResolvedValue(response); mount();
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "renewal risks" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  const option = await screen.findByRole("option");
  expect(option.querySelector("mark")).toBeNull(); expect(option.textContent).toContain("Semantic");
});
it("resets draft and submitted results when the deal changes", async () => {
  const props = { currentFileName: "", currentPageCount: 0, onActivateResult: vi.fn(), open: true };
  const view=render(<DataRoomDocumentSearch {...props} dealId="DEAL-1" />);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "liability" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await screen.findByRole("option");
  view.rerender(<DataRoomDocumentSearch {...props} dealId="DEAL-2" />);
  expect((screen.getByRole("searchbox") as HTMLInputElement).value).toBe("");
  expect(screen.queryAllByRole("option")).toHaveLength(0);
  expect(search).toHaveBeenCalledTimes(1);
});

it("aborts pending browser work when edited and revalidates a submitted query on focus", async () => {
  search.mockImplementationOnce((_deal: string, _input: unknown, signal: AbortSignal) => new Promise((_resolve, reject) => {
    signal.addEventListener("abort", () => reject(new DOMException("Aborted", "AbortError")));
  }));
  mount();
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "first" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await waitFor(() => expect(search).toHaveBeenCalledTimes(1));
  const signal = search.mock.calls[0][2] as AbortSignal;
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "second" } });
  expect(signal.aborted).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await screen.findByRole("option");
  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(search).toHaveBeenCalledTimes(3));
  expect(search.mock.calls[2][1]).toEqual({ queryText: "second", limit: 30 });
});

it("clears the submission and ignores late results on an actor-session change", async () => {
  const { sessionStore } = await import("@/auth/sessionStore");
  let finish: (result: HybridDocumentResponse) => void = () => {};
  search.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  mount(); fireEvent.change(screen.getByRole("searchbox"), { target: { value: "private" } });
  fireEvent.click(screen.getByRole("button", { name: "Search" }));
  await waitFor(() => expect(search).toHaveBeenCalledTimes(1));
  act(() => sessionStore.set(null));
  await act(async () => finish(hybridResponse()));
  expect((screen.getByRole("searchbox") as HTMLInputElement).value).toBe("");
  expect(screen.queryAllByRole("option")).toHaveLength(0);
});

// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { DataRoomDocumentSearch } from "@/components/data-room/document-search/DataRoomDocumentSearch";
import { keywordHit } from "../../../fixtures/documents/keywordSearch";
import type { KeywordFileChunkHit } from "@/contracts/quarryApi";

const { search } = vi.hoisted(() => ({ search: vi.fn() }));
vi.mock("@quarry/runtime", () => ({ runtime: { api: { searchDealDocumentChunksByKeyword: search } } }));
afterEach(() => { cleanup(); vi.useRealTimers(); });
beforeEach(() => search.mockReset().mockResolvedValue([keywordHit()]));

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
  search.mockResolvedValue([keywordHit({ text })]);
  mount();
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: query } });
  const option = await screen.findByRole("option");
  const preview = screen.getByRole("complementary", { name: "Preview", hidden: true });
  for (const surface of [option, preview]) {
    expect([...surface.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(expected);
  }
  expect(within(preview).getByRole("heading").querySelector("mark")).toBeNull();
});

it("uses whole-token ranges in file names without truncating or normalizing them", async () => {
  search.mockResolvedValue([keywordHit({ text: "The café uses C++ and C#." })]);
  const fileName = "📄 café_C++_C#_Confidential.pdf";
  render(<DataRoomDocumentSearch dealId="DEAL-1" currentFileId="file-1"
    currentFileName={fileName} currentPageCount={3} onActivateResult={vi.fn()} open />);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "CAFÉ, C++" } });
  const option = await screen.findByRole("option");
  const heading = within(screen.getByRole("complementary", { name: "Preview", hidden: true })).getByRole("heading");
  expect(heading.textContent).toBe(fileName);
  expect([...heading.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(["café", "C++"]);
  expect([...option.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(["café", "C++", "café", "C++"]);
});

it("debounces typing, calls the deal keyword API and activates an actual page", async () => {
  vi.useFakeTimers();
  const activate = vi.fn();
  mount(activate);
  const input = screen.getByRole("searchbox");
  fireEvent.change(input, { target: { value: "liab" } });
  await act(async () => vi.advanceTimersByTime(200));
  fireEvent.change(input, { target: { value: "liability" } });
  await act(async () => vi.advanceTimersByTime(249));
  expect(search).not.toHaveBeenCalled();
  expect(screen.queryByText(/No results for/)).toBeNull();
  await act(async () => vi.advanceTimersByTime(1));
  expect(search).toHaveBeenCalledExactlyOnceWith("DEAL-1", { queryText: "liability", limit: 30 });
  fireEvent.click(screen.getByRole("option", { name: /Synthetic_Terms/ }));
  expect(activate).toHaveBeenCalledWith(expect.objectContaining({ fileName: "Synthetic_Terms.pdf", target: { kind: "pdf-page", page: 2 } }));
});

it("does not refilter API matches by filename or truncated excerpt", async () => {
  search.mockResolvedValue([keywordHit({ text: "First keyword here. " + "Context. ".repeat(90) + "Second keyword here." })]);
  mount();
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "First Second" } });
  expect(await screen.findByRole("option", { name: /Synthetic_Terms/ })).toBeTruthy();
});

it("uses file identity to disable cross-document and missing-page navigation", async () => {
  search.mockResolvedValue([
    keywordHit({ file_id: "file-2" }),
    keywordHit({ chunk_id: "no-page", page_start: null, page_end: null }),
    keywordHit({ chunk_id: "out-of-bounds", page_start: 4, page_end: 4 }),
  ]);
  const activate = vi.fn();
  mount(activate);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "liability" } });
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
  let finish: (hits: KeywordFileChunkHit[]) => void = () => {};
  search.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  mount();
  const input = screen.getByRole("searchbox");
  fireEvent.change(input, { target: { value: "old" } });
  await waitFor(() => expect(search).toHaveBeenCalledTimes(1));
  fireEvent.change(input, { target: { value: "new" } });
  await act(async () => finish([keywordHit({ text: "Old private passage" })]));
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
  expect((await screen.findByRole("alert")).textContent).toContain("Keyword search unavailable");
  expect(screen.queryByText(/No results for/)).toBeNull();
  await user.click(screen.getByRole("button", { name: "Retry search" }));
  expect(await screen.findByRole("option", { name: /Synthetic_Terms/ })).toBeTruthy();
});

it("searches without a selected document and reports an empty API result", async () => {
  search.mockResolvedValue([]);
  render(<DataRoomDocumentSearch dealId="DEAL-1" currentFileName="" currentPageCount={0} onActivateResult={vi.fn()} open />);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "missing" } });
  expect(await screen.findByText(/No results for/)).toBeTruthy();
  expect(search).toHaveBeenCalledWith("DEAL-1", { queryText: "missing", limit: 30 });
  expect(screen.queryByText("Sample excerpts")).toBeNull();
});

it("discards an in-flight response when the panel closes", async () => {
  let finish: (hits: KeywordFileChunkHit[]) => void = () => {};
  search.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const props = { dealId: "DEAL-1", currentFileName: "", currentPageCount: 0, onActivateResult: vi.fn() };
  const view = render(<DataRoomDocumentSearch {...props} open />);
  fireEvent.change(screen.getByRole("searchbox"), { target: { value: "old" } });
  await waitFor(() => expect(search).toHaveBeenCalledTimes(1));
  view.rerender(<DataRoomDocumentSearch {...props} open={false} />);
  await act(async () => finish([keywordHit({ text: "Closed-panel result" })]));
  view.rerender(<DataRoomDocumentSearch {...props} open />);
  expect(screen.queryByText(/Closed-panel result/)).toBeNull();
  expect(screen.queryAllByRole("option")).toHaveLength(0);
});

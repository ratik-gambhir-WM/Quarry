// @vitest-environment happy-dom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { MemoryRouter } from "react-router-dom";
import { DealDocumentSearch } from "@/components/deal-room/DealDocumentSearch";
import type { KeywordFileChunkHit } from "@/contracts/quarryApi";
// Use Motion's JS fallback: happy-dom rejects canceled Web Animations promises.
const animationDescriptor = Object.getOwnPropertyDescriptor(
  Element.prototype,
  "animate",
);
beforeAll(() => {
  Reflect.deleteProperty(Element.prototype, "animate");
});
afterAll(() => {
  if (animationDescriptor)
    Object.defineProperty(Element.prototype, "animate", animationDescriptor);
});
const { scoped, global } = vi.hoisted(() => ({
  scoped: vi.fn(),
  global: vi.fn(),
}));
vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: {
      searchDealDocumentChunksByKeyword: scoped,
      searchDocumentChunksByKeyword: global,
    },
  },
}));
vi.mock("@/app/WorkspaceProvider", () => ({
  useWorkspace: () => ({ deals: [] }),
}));
afterEach(cleanup);
beforeEach(() => {
  scoped.mockReset().mockResolvedValue([]);
  global.mockReset().mockResolvedValue([]);
});
function mount() {
  return render(
    <MemoryRouter>
      <DealDocumentSearch dealId="DEAL-1" />
    </MemoryRouter>,
  );
}
function hit(text: string, overrides: Partial<KeywordFileChunkHit> = {}): KeywordFileChunkHit {
  return {
    chunk_id: "chunk-search", deal_id: "DEAL-1", file_id: "file-search",
    version_id: "version-search", index_generation: "generation-search",
    chunk_index: 1, chunk_sha256: "0".repeat(64), text, token_count: 10,
    page_start: null, page_end: null, char_start: 0, char_end: text.length,
    section_path: "Architecture", created_at: "2026-11-06T00:00:00Z", score: 1,
    ...overrides,
  };
}
describe("document search scope", () => {
  it("defaults to this deal and lets the user search across accessible deals", async () => {
    const user = userEvent.setup();
    mount();
    expect(
      screen.getByRole("combobox", { name: "Search scope" }).textContent,
    ).toContain("This deal");
    await user.type(
      screen.getByRole("searchbox", { name: "Search terms" }),
      "architecture",
    );
    await user.click(screen.getByRole("button", { name: "Search documents" }));
    await waitFor(() =>
      expect(scoped).toHaveBeenCalledWith("DEAL-1", {
        queryText: "architecture",
        limit: 30,
      }),
    );
    await screen.findByText("No matching documents.");
    await user.click(screen.getByRole("combobox", { name: "Search scope" }));
    await user.click(
      screen.getByRole("option", { name: "All accessible deals" }),
    );
    await user.click(screen.getByRole("button", { name: "Search documents" }));
    await waitFor(() =>
      expect(global).toHaveBeenCalledWith({
        queryText: "architecture",
        limit: 30,
      }),
    );
    expect(scoped).toHaveBeenCalledTimes(1);
  });
  it("ignores a late response after a scope change and clears content on focus", async () => {
    let finish: (value: unknown[]) => void = () => {};
    scoped.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const user = userEvent.setup();
    mount();
    await user.type(screen.getByRole("searchbox"), "term");
    await user.click(screen.getByRole("button", { name: "Search documents" }));
    await user.click(screen.getByRole("combobox", { name: "Search scope" }));
    await user.click(
      screen.getByRole("option", { name: "All accessible deals" }),
    );
    finish([
      hit("late private result", { chunk_id: "late" }),
    ]);
    await waitFor(() =>
      expect(screen.queryByText("late private result")).toBeNull(),
    );
    global.mockResolvedValue([
      hit("current result", { chunk_id: "current", deal_id: "DEAL-2" }),
    ]);
    await user.click(screen.getByRole("button", { name: "Search documents" }));
    await screen.findByText("current result");
    window.dispatchEvent(new Event("focus"));
    await waitFor(() =>
      expect(screen.queryByText("current result")).toBeNull(),
    );
  });
  it("shows a retryable error without rendering results or issuing an empty query", async () => {
    const user = userEvent.setup();
    mount();
    expect(
      (
        screen.getByRole("button", {
          name: "Search documents",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    scoped.mockRejectedValueOnce(new Error("Search unavailable"));
    await user.type(screen.getByRole("searchbox"), "term");
    await user.click(screen.getByRole("button", { name: "Search documents" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
      "Search unavailable",
    );
    await user.click(screen.getByRole("button", { name: "Retry search" }));
    await screen.findByText("No matching documents.");
  });
});

it("shows compact highlighted passages, section and page context", async () => {
  const user = userEvent.setup();
  scoped.mockResolvedValue([hit(
    `${"Background on the service. ".repeat(40)}The application uses Ruby on Rails. ${"Additional context. ".repeat(40)}`,
    { section_path: "Application architecture", page_start: 2, page_end: 3 },
  )]);
  const { container } = mount();
  await user.type(screen.getByRole("searchbox"), "Ruby on Rails");
  await user.click(screen.getByRole("button", { name: "Search documents" }));
  await screen.findByRole("heading", { name: "Application architecture" });
  expect(screen.getByRole("status").textContent).toBe("1 matching passage in this deal.");
  const results = screen.getByRole("list", { name: "Search results" });
  expect(results.textContent).toContain("Pages 2–3");
  expect(results.textContent).toContain("Ruby on Rails");
  expect(results.textContent?.length).toBeLessThan(450);
  expect([...container.querySelectorAll("mark")].map((mark) => mark.textContent)).toContain("Ruby");
  expect(screen.getByRole("link", { name: "Open data room for Deal" }).getAttribute("href")).toContain("DEAL-1");
});

it("clears the Stealth search field, restores focus and disables an empty search", async () => {
  const user = userEvent.setup();
  mount();
  const input = screen.getByRole("searchbox", { name: "Search terms" });
  await user.type(input, "term");
  await user.click(
    screen.getByRole("button", { name: "Clear document search" }),
  );
  expect(input).toHaveProperty("value", "");
  expect(document.activeElement).toBe(input);
  expect(
    screen.getByRole("button", { name: "Search documents" }),
  ).toHaveProperty("disabled", true);
  expect(scoped).not.toHaveBeenCalled();
});

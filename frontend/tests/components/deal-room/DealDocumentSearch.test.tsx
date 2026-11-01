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
      { chunk_id: "late", deal_id: "DEAL-1", text: "late private result" },
    ]);
    await waitFor(() =>
      expect(screen.queryByText("late private result")).toBeNull(),
    );
    global.mockResolvedValue([
      { chunk_id: "current", deal_id: "DEAL-2", text: "current result" },
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

// @vitest-environment happy-dom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Link, MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, expect, it, vi } from "vitest";
import { DealAccessBoundary } from "@/components/deal-room/DealAccessBoundary";
import { buildWorkspaceDealFromPersisted } from "@/data/dealExtraction";
import { testDeal } from "@/test/fixtures/deals";
const mocks = vi.hoisted(() => ({
  get: vi.fn(),
  cache: vi.fn(),
  forget: vi.fn(),
}));
vi.mock("@quarry/runtime", () => ({
  runtime: { api: { getDeal: mocks.get } },
}));
vi.mock("@/app/WorkspaceProvider", () => ({
  useWorkspace: () => ({
    cacheDealResult: mocks.cache,
    forgetDeal: mocks.forget,
  }),
}));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
function show() {
  const deal = buildWorkspaceDealFromPersisted(testDeal(), null);
  render(
    <MemoryRouter initialEntries={["/deal"]}>
      <Routes>
        <Route
          path="/deal"
          element={
            <DealAccessBoundary deal={deal}>
              {() => <p>Protected document content</p>}
            </DealAccessBoundary>
          }
        />
        <Route path="/hub/deals" element={<p>Accessible deals</p>} />
      </Routes>
    </MemoryRouter>,
  );
  return deal;
}
it.each([403, 404])(
  "forgets the cached deal and unmounts protected content on %s during revalidation",
  async (status) => {
    mocks.get
      .mockResolvedValueOnce({ ...testDeal(), metadata: null })
      .mockRejectedValueOnce({ status });
    const deal = show();
    await screen.findByText("Protected document content");
    window.dispatchEvent(new Event("focus"));
    await screen.findByText("Accessible deals");
    expect(mocks.forget).toHaveBeenCalledWith(deal.room.id);
    expect(screen.queryByText("Protected document content")).toBeNull();
  },
);
it("keeps content hidden while access is unknown and supports transient-error retry", async () => {
  mocks.get
    .mockRejectedValueOnce(new Error("Access service unavailable"))
    .mockResolvedValueOnce({ ...testDeal(), metadata: null });
  const user = userEvent.setup();
  show();
  expect(screen.getByRole("status").textContent).toContain(
    "Checking deal access",
  );
  expect(screen.queryByText("Protected document content")).toBeNull();
  await screen.findByRole("alert");
  expect(mocks.forget).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Retry" }));
  await screen.findByText("Protected document content");
  await waitFor(() => expect(mocks.cache).toHaveBeenCalledOnce());
});

it("keeps the workspace mounted while navigation revalidates access", async () => {
  const deal = buildWorkspaceDealFromPersisted(testDeal(), null);
  let rejectRevalidation: (error: unknown) => void = () => {};
  mocks.get.mockResolvedValueOnce({ ...testDeal(), metadata: null }).mockImplementationOnce(
    () => new Promise((_, reject) => { rejectRevalidation = reject; }),
  );
  const user = userEvent.setup();
  render(
    <MemoryRouter initialEntries={["/deal"]}>
      <Routes>
        <Route path="/deal/*" element={
          <DealAccessBoundary deal={deal}>
            {() => <><Link to="/deal/analysis">Analysis</Link><input aria-label="Workspace state" /></>}
          </DealAccessBoundary>
        } />
        <Route path="/hub/deals" element={<p>Accessible deals</p>} />
      </Routes>
    </MemoryRouter>,
  );
  const input = await screen.findByLabelText("Workspace state");
  await user.type(input, "retained");
  await user.click(screen.getByRole("link", { name: "Analysis" }));
  await waitFor(() => expect(mocks.get).toHaveBeenCalledTimes(2));
  expect(screen.getByLabelText("Workspace state")).toBe(input);
  expect(input).toHaveProperty("value", "retained");
  expect(screen.queryByText("Checking deal access…")).toBeNull();
  rejectRevalidation({ status: 403 });
  await screen.findByText("Accessible deals");
  expect(screen.queryByLabelText("Workspace state")).toBeNull();
});

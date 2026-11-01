// @vitest-environment happy-dom
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import { DealMembersPanel } from "@/components/deal-room/DealMembersPanel";
import { testDeal } from "@/test/fixtures/deals";
import type { DealRole } from "@/contracts/dealMemberships";
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
const mocks = vi.hoisted(() => ({
  api: {
    listDealMembers: vi.fn(),
    listDealTeams: vi.fn(),
    addDealMember: vi.fn(),
    changeDealMemberRole: vi.fn(),
    transferDealOwnership: vi.fn(),
    addDealTeamMember: vi.fn(),
    getDeal: vi.fn(),
  },
  cache: vi.fn(),
  forget: vi.fn(),
}));
vi.mock("@quarry/runtime", () => ({ runtime: { api: mocks.api } }));
vi.mock("@/app/WorkspaceProvider", () => ({
  useWorkspace: () => ({
    cacheDealResult: mocks.cache,
    forgetDeal: mocks.forget,
  }),
}));
vi.mock("@/auth/sessionStore", () => ({
  sessionStore: { getSnapshot: () => ({ session: { user: { id: 1 } } }) },
}));
function show(role: DealRole = "owner") {
  const deal = testDeal({ role });
  if (role !== "owner")
    deal.capabilities = {
      read: true,
      upload: role !== "viewer",
      edit: role === "manager",
      manageTeams: role === "manager",
      manageMembers: false,
      transfer: false,
      archive: false,
      localSource: false,
    };
  render(
    <MemoryRouter>
      <DealMembersPanel deal={deal} />
    </MemoryRouter>,
  );
  return deal;
}
beforeEach(() => {
  mocks.api.listDealMembers.mockResolvedValue({
    items: [
      {
        userId: 1,
        username: "owner",
        firstName: "Original",
        lastName: "Owner",
        role: "owner",
        revision: 1,
        joinedAt: "now",
        teamIds: [],
      },
      {
        userId: 2,
        username: "member",
        firstName: "Shared",
        lastName: "Member",
        role: "contributor",
        revision: 7,
        joinedAt: "now",
        teamIds: [],
      },
    ],
    nextCursor: null,
  });
  mocks.api.listDealTeams.mockResolvedValue({ items: [], nextCursor: null });
  mocks.api.getDeal.mockResolvedValue({ ...testDeal(), metadata: null });
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
it.each(["manager", "contributor", "viewer"] as const)(
  "shows %s only its permitted management controls",
  async (role) => {
    show(role);
    await screen.findByText("Shared Member");
    expect(screen.queryByLabelText("Registered username")).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Transfer ownership" }),
    ).toBeNull();
    expect(screen.queryByRole("button", { name: "Remove member" })).toBeNull();
    expect(Boolean(screen.queryByRole("button", { name: "Create team" }))).toBe(
      role === "manager",
    );
  },
);
it("adds a registered user and refreshes authoritative access and roster", async () => {
  const user = userEvent.setup();
  const deal = show();
  await screen.findByText("Shared Member");
  await user.type(screen.getByLabelText("Registered username"), "new-member");
  await user.click(screen.getByRole("button", { name: "Add member" }));
  await waitFor(() =>
    expect(mocks.api.addDealMember).toHaveBeenCalledWith(deal.dealId, {
      username: "new-member",
      role: "contributor",
    }),
  );
  await waitFor(() => expect(mocks.cache).toHaveBeenCalled());
  expect(mocks.api.listDealMembers).toHaveBeenCalledTimes(2);
});
it("uses the member revision and refreshes on a conflict", async () => {
  mocks.api.changeDealMemberRole.mockRejectedValue({ status: 409 });
  const user = userEvent.setup();
  const deal = show();
  await screen.findByText("Shared Member");
  await user.click(screen.getByRole("combobox", { name: "Role for member" }));
  await user.click(screen.getByRole("option", { name: "Viewer" }));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "This deal changed",
  );
  expect(mocks.api.changeDealMemberRole).toHaveBeenCalledWith(deal.dealId, 2, {
    role: "viewer",
    expectedRevision: 7,
  });
  await waitFor(() =>
    expect(mocks.api.listDealMembers).toHaveBeenCalledTimes(2),
  );
});
it("requires transfer confirmation, restores focus on cancellation, and sends the access revision", async () => {
  const user = userEvent.setup();
  const deal = show();
  const transfer = await screen.findByRole("button", {
    name: "Transfer ownership",
  });
  await user.click(transfer);
  await user.click(screen.getByRole("button", { name: "Cancel" }));
  expect(mocks.api.transferDealOwnership).not.toHaveBeenCalled();
  await waitFor(() => expect(document.activeElement).toBe(transfer));
  await user.click(transfer);
  await user.click(screen.getByRole("button", { name: "Confirm change" }));
  await waitFor(() =>
    expect(mocks.api.transferDealOwnership).toHaveBeenCalledWith(deal.dealId, {
      userId: 2,
      expectedAccessRevision: 1,
    }),
  );
});
it("shows a retry when roster loading fails", async () => {
  mocks.api.listDealMembers.mockRejectedValueOnce(
    new Error("Roster unavailable"),
  );
  const user = userEvent.setup();
  show();
  await screen.findByText("Roster unavailable");
  await user.click(screen.getByRole("button", { name: "Retry roster" }));
  await screen.findByText("Shared Member");
});

it("keeps a failed confirmation open and prevents dismissal while the request is pending", async () => {
  let fail: (error: Error) => void = () => {};
  mocks.api.transferDealOwnership.mockImplementation(
    () =>
      new Promise<void>((_resolve, reject) => {
        fail = reject;
      }),
  );
  const user = userEvent.setup();
  show();
  const transfer = await screen.findByRole("button", {
    name: "Transfer ownership",
  });
  await user.click(transfer);
  const dialog = screen.getByRole("alertdialog");
  await waitFor(() =>
    expect(document.activeElement).toBe(
      screen.getByRole("button", { name: "Cancel" }),
    ),
  );
  await user.click(screen.getByRole("button", { name: "Confirm change" }));
  await user.keyboard("{Escape}");
  expect(screen.getByRole("alertdialog")).toBe(dialog);
  fail(new Error("Transfer unavailable"));
  expect((await screen.findByRole("alert")).textContent).toContain(
    "Transfer unavailable",
  );
  expect(screen.getByRole("alertdialog")).toBe(dialog);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  await waitFor(() => expect(document.activeElement).toBe(transfer));
});
it("expands teams and offers only unassigned members through the Stealth picker", async () => {
  const team = {
    teamId: "00000000-0000-4000-8000-000000000001",
    name: "Technology",
    revision: 1,
    userIds: [1],
  };
  mocks.api.listDealTeams
    .mockResolvedValueOnce({ items: [team], nextCursor: null })
    .mockResolvedValue({
      items: [{ ...team, revision: 2, userIds: [1, 2] }],
      nextCursor: null,
    });
  const user = userEvent.setup();
  const deal = show();
  await screen.findByRole("heading", { name: "Technology" });
  expect(screen.queryByRole("button", { name: "Create team" })).toBeNull();
  const teamHeading = screen.getByRole("heading", { name: "Technology" });
  const memberHeading = screen.getByRole("heading", { name: "Deal members" });
  expect(teamHeading.compareDocumentPosition(memberHeading) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  await user.click(
    screen.getByRole("combobox", { name: "Assign member to Technology" }),
  );
  expect(screen.queryByRole("option", { name: "Original Owner" })).toBeNull();
  await user.click(
    screen.getByRole("option", { name: "Shared Member" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Assign member" }),
  );
  await waitFor(() =>
    expect(mocks.api.addDealTeamMember).toHaveBeenCalledWith(
      deal.dealId,
      team.teamId,
      2,
    ),
  );
  await screen.findByRole("button", { name: "Technology · 2 members" });
  expect(
    screen.getByRole("combobox", { name: "Assign member to Technology" }),
  ).toHaveProperty("disabled", true);
});

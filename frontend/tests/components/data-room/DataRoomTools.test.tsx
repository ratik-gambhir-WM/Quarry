/* @vitest-environment happy-dom */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { useMemo } from "react";
import { MemoryRouter } from "react-router-dom";
import { DataRoomToolsProvider, useDataRoomDocumentSearch } from "@/components/data-room/DataRoomToolsProvider";
import { SidebarFrame } from "@/components/hub/sidebar/SidebarFrame";
import type { DataRoomDocumentSearchProps } from "@/components/data-room/document-search/DataRoomDocumentSearch";

function SearchRegistration({ search }: { search: DataRoomDocumentSearchProps }) {
  const value = useMemo(() => search, [search]);
  useDataRoomDocumentSearch(value);
  return null;
}
function ToolsHarness({ dealId, email, documentSearch }: {
  dealId: string;
  email?: string;
  documentSearch: DataRoomDocumentSearchProps;
}) {
  return <DataRoomToolsProvider dealId={dealId} email={email}>
    <SearchRegistration search={documentSearch} />
  </DataRoomToolsProvider>;
}
async function launch(user: ReturnType<typeof userEvent.setup>, label: string) {
  await user.click(screen.getByRole("button", { name: "Data Room tools" }));
  await user.click(await screen.findByRole("menuitem", { name: label }));
}

const { api } = vi.hoisted(() => ({
  api: {
    createAssistantThread: vi.fn(),
    deleteAssistantThread: vi.fn(),
    getAssistantThread: vi.fn(),
    listAssistantThreads: vi.fn(),
    queryModel: vi.fn(),
    runAssistantThread: vi.fn(),
  },
}));

vi.mock("@quarry/runtime", () => ({ runtime: { api } }));

// Happy DOM rejects canceled Web Animation promises; use Motion's JS fallback.
const animationDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
beforeAll(() => { Reflect.deleteProperty(Element.prototype, "animate"); });
afterAll(() => {
  if (animationDescriptor) Object.defineProperty(Element.prototype, "animate", animationDescriptor);
});
afterEach(cleanup);

beforeEach(() => {
  api.createAssistantThread.mockReset();
  api.deleteAssistantThread.mockReset();
  api.getAssistantThread.mockReset();
  api.listAssistantThreads.mockReset();
  api.queryModel.mockReset();
  api.runAssistantThread.mockReset();
});

describe("DataRoomTools", () => {
  it("leaves the logo non-interactive outside the Data Room", () => {
    render(<MemoryRouter><SidebarFrame>Content</SidebarFrame></MemoryRouter>);
    expect(screen.queryByRole("button", { name: "Data Room tools" })).toBeNull();
  });

  it("offers only Assistant and Workspace Search from the logo", async () => {
    const user = userEvent.setup();
    render(<DataRoomToolsProvider dealId="DEAL-000184">Content</DataRoomToolsProvider>);
    const trigger = screen.getByRole("button", { name: "Data Room tools" });
    expect(trigger.querySelectorAll("rect")).toHaveLength(8);
    await user.click(trigger);
    expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Assistant", "Workspace Search"]);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(document.activeElement).toBe(trigger));
    expect(api.listAssistantThreads).not.toHaveBeenCalled();
  });

  it("opens the search panel directly and returns focus to its sidebar button", async () => {
    const user = userEvent.setup();

    render(
      <ToolsHarness
        dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
      />,
    );

    const searchTrigger = screen.getByRole("button", {
      name: "Data Room tools",
    });
    await launch(user, "Workspace Search");

    expect(
      await screen.findByRole("dialog", { name: "Workspace Search" }, { timeout: 5000 }),
    ).not.toBeNull();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("searchbox", { name: "Search document" })));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(searchTrigger).toBe(document.activeElement));
  });

  it("keeps selected-document page jumps wired through the dial", async () => {
    const user = userEvent.setup();
    const activate = vi.fn();
    const focusPreview = vi.fn(() => screen.getByRole("button", { name: "Document viewer" }).focus());
    render(<><button>Document viewer</button><ToolsHarness dealId="DEAL-000184" documentSearch={{
      currentFileName: "Synthetic_Terms.pdf",
      currentPageCount: 3,
      onActivateResult: activate,
      onSelectionFocus: focusPreview,
    }} /></>);
    await launch(user, "Workspace Search");
    await user.type(await screen.findByRole("searchbox", { name: "Search document" }), "Synthetic");
    await user.click(screen.getByRole("option", { name: /Open page 2/ }));
    expect(activate).toHaveBeenCalledWith(expect.objectContaining({ target: { kind: "pdf-page", page: 2 } }));
    await waitFor(() => expect(focusPreview).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Document viewer" })));
  });

  it("centers search inside the document boundary on its first lazy open", async () => {
    const user = userEvent.setup();
    const boundary = document.createElement("div");
    boundary.getBoundingClientRect = () => DOMRect.fromRect({ x: 100, y: 40, width: 900, height: 600 });
    render(<ToolsHarness dealId="DEAL-000184" documentSearch={{
      currentFileName: "Synthetic_Terms.pdf", currentPageCount: 3,
      onActivateResult: vi.fn(), boundaryElement: boundary,
    }} />);
    await launch(user, "Workspace Search");
    const panel = await screen.findByRole("dialog", { name: "Workspace Search" });
    expect(panel.parentElement?.style.getPropertyValue("--x")).toBe("190px");
    expect(panel.parentElement?.style.getPropertyValue("--y")).toBe("120px");
  });

  it("explains search availability before a document is opened", async () => {
    const user = userEvent.setup();
    render(<DataRoomToolsProvider dealId="DEAL-000184">Files</DataRoomToolsProvider>);
    await launch(user, "Workspace Search");
    expect(await screen.findByText(/Workspace-wide search is not connected yet/)).not.toBeNull();
  });

  it("shows the Assistant chat interface in the Synthesis Canvas", async () => {
    const user = userEvent.setup();

    render(
      <ToolsHarness
        dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
      />,
    );

    const trigger = screen.getByRole("button", {
      name: "Data Room tools",
    });
    await launch(user, "Assistant");

    expect(
      await screen.findByRole("heading", { name: "How can I help you today?" }),
    ).not.toBeNull();
    expect(screen.getByRole("textbox", { name: "Message input" })).toBe(
      document.activeElement,
    );
    expect(
      screen.queryByRole("button", { name: /Summarize deal risks/ }),
    ).toBeNull();
    await user.click(
      screen.getByRole("button", { name: "Close Assistant" }),
    );
    await waitFor(() => expect(trigger).toBe(document.activeElement));
  });

  it("selects a previous Assistant chat from the panel header", async () => {
    const user = userEvent.setup();
    const thread = {
      createdAt: "2026-10-12 10:00:00",
      lastMessageAt: "2026-10-12 10:01:00",
      status: "regular" as const,
      threadId: "thread-1",
      title: "Customer concentration review",
      updatedAt: "2026-10-12 10:01:00",
    };
    api.listAssistantThreads.mockResolvedValue({
      nextCursor: null,
      threads: [thread],
    });
    api.getAssistantThread.mockResolvedValue({
      ...thread,
      messages: [
        {
          completedAt: "2026-10-12 10:00:01",
          content: "What customer concentration risk do you see?",
          createdAt: "2026-10-12 10:00:00",
          messageId: "message-1",
          model: null,
          parentMessageId: null,
          role: "user" as const,
          sequence: 1,
          status: "completed" as const,
          threadId: thread.threadId,
        },
        {
          completedAt: "2026-10-12 10:01:01",
          content: "The largest customer is the primary concentration risk.",
          createdAt: "2026-10-12 10:01:00",
          messageId: "message-2",
          model: "gpt-5.5",
          parentMessageId: "message-1",
          role: "assistant" as const,
          sequence: 2,
          status: "completed" as const,
          threadId: thread.threadId,
        },
      ],
    });

    render(
      <ToolsHarness
        dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        email="analyst@example.com"
      />,
    );

    await launch(user, "Assistant");
    await user.click(
      await screen.findByRole("button", { name: "Select previous chat" }),
    );
    await user.click(
      await screen.findByRole("menuitem", { name: thread.title }),
    );

    expect(
      await screen.findByText(
        "The largest customer is the primary concentration risk.",
      ),
    ).not.toBeNull();
    expect(api.getAssistantThread).toHaveBeenCalledWith(thread.threadId);

    const composer = screen.getByRole("textbox", { name: "Message input" });
    const composerRoot = composer.closest('[data-slot="query-composer"]');
    const composerDock = composerRoot?.parentElement?.parentElement;
    expect(composerRoot?.className).toContain("min-h-[2.625rem]");
    expect(composer.className).toContain("min-h-[1.875rem]");
    expect(composerDock?.className).not.toContain(
      "bg-[var(--theme-workspace-surface-bottom)]",
    );
  });
});

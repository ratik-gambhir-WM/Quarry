/* @vitest-environment happy-dom */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DataRoomQuickActions } from "@/components/data-room/DataRoomQuickActions";

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

afterEach(cleanup);

beforeEach(() => {
  api.createAssistantThread.mockReset();
  api.deleteAssistantThread.mockReset();
  api.getAssistantThread.mockReset();
  api.listAssistantThreads.mockReset();
  api.queryModel.mockReset();
  api.runAssistantThread.mockReset();
});

describe("DataRoomQuickActions", () => {
  it("renders evenly distributed Upload, Synthesis Canvas, Notes, and Search actions", () => {
    render(
      <DataRoomQuickActions dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={vi.fn()}
      />,
    );

    const tools = screen.getByRole("toolbar", { name: "Data room tools" });
    expect(tools.querySelectorAll("button")).toHaveLength(4);
    expect(screen.getByRole("button", { name: "Upload files" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Open Synthesis Canvas" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Notes" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Search document" })).not.toBeNull();
    expect(screen.getByRole("button", { name: "Upload files" }).dataset.variant).toBe("ghost");
    expect(screen.getByRole("button", { name: "Open Synthesis Canvas" }).dataset.variant).toBe("ghost");
    expect(screen.getByRole("button", { name: "Notes" }).dataset.variant).toBe("ghost");
    expect(screen.getByRole("button", { name: "Search document" }).dataset.variant).toBe("ghost");
    expect(screen.queryByRole("menu", { name: "Data room views" })).toBeNull();
  });

  it("opens the existing file-upload flow from the Upload icon", async () => {
    const onUploadNewFile = vi.fn();
    const user = userEvent.setup();

    render(
      <DataRoomQuickActions dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={onUploadNewFile}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Upload files" }));
    expect(onUploadNewFile).toHaveBeenCalledTimes(1);
  });

  it("opens the search panel directly and returns focus to its sidebar button", async () => {
    const user = userEvent.setup();

    render(
      <DataRoomQuickActions dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={vi.fn()}
      />,
    );

    const searchTrigger = screen.getByRole("button", { name: "Search document" });
    await user.click(searchTrigger);

    expect(await screen.findByRole("dialog", { name: "Search Synthetic_Terms.pdf" })).not.toBeNull();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(searchTrigger).toBe(document.activeElement));
  });

  it("shows the Assistant chat interface in the Synthesis Canvas", async () => {
    const user = userEvent.setup();

    render(
      <DataRoomQuickActions dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        onUploadNewFile={vi.fn()}
      />,
    );

    const trigger = screen.getByRole("button", { name: "Open Synthesis Canvas" });
    await user.click(trigger);

    expect(await screen.findByRole("heading", { name: "How can I help you today?" })).not.toBeNull();
    expect(screen.getByRole("textbox", { name: "Message input" })).toBe(document.activeElement);
    expect(screen.queryByRole("button", { name: /Summarize deal risks/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Close Synthesis Canvas" }));
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
    api.listAssistantThreads.mockResolvedValue({ nextCursor: null, threads: [thread] });
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
      <DataRoomQuickActions dealId="DEAL-000184"
        documentSearch={{
          currentFileName: "Synthetic_Terms.pdf",
          currentPageCount: 3,
          onActivateResult: vi.fn(),
        }}
        email="analyst@example.com"
        onUploadNewFile={vi.fn()}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Open Synthesis Canvas" }));
    await user.click(await screen.findByRole("button", { name: "Select previous chat" }));
    await user.click(await screen.findByRole("menuitem", { name: thread.title }));

    expect(await screen.findByText("The largest customer is the primary concentration risk.")).not.toBeNull();
    expect(api.getAssistantThread).toHaveBeenCalledWith(thread.threadId);

    const composer = screen.getByRole("textbox", { name: "Message input" });
    const composerRoot = composer.closest('[data-slot="query-composer"]');
    const composerDock = composerRoot?.parentElement?.parentElement;
    expect(composerRoot?.className).toContain("min-h-[2.625rem]");
    expect(composer.className).toContain("min-h-[1.875rem]");
    expect(composerDock?.className).not.toContain("bg-[var(--theme-workspace-surface-bottom)]");
  });
});

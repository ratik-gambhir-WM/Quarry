// @vitest-environment happy-dom

import type { ButtonHTMLAttributes, ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AssistantWorkspaceSidebar } from "@/components/hub/sidebar/AssistantWorkspaceSidebar";

const { startNewChat } = vi.hoisted(() => ({ startNewChat: vi.fn() }));

vi.mock("@assistant-ui/react", () => ({
  ThreadListItemPrimitive: {
    Root: ({ children }: { children: ReactNode }) => children,
    Title: () => <span>Previous chat</span>,
    Trigger: ({ children, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { children: ReactNode }) => (
      <button {...props}>{children}</button>
    ),
  },
  ThreadListPrimitive: {
    Items: () => null,
    New: ({ children, onClick, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { children: ReactNode }) => (
      <button
        {...props}
        onClick={(event) => {
          onClick?.(event);
          startNewChat();
        }}
        type="button"
      >
        {children}
      </button>
    ),
  },
}));

afterEach(() => {
  cleanup();
  startNewChat.mockReset();
});

describe("AssistantWorkspaceSidebar", () => {
  it("starts on Chat with an empty previous-chat list", () => {
    render(
      <MemoryRouter initialEntries={["/hub/assistant"]}>
        <AssistantWorkspaceSidebar activeHomeSection="assistant" tools={[]} />
      </MemoryRouter>,
    );

    const tabs = screen.getByRole("tablist", { name: "Assistant sidebar views" });
    const dealHub = screen.getByRole("tab", { name: "Deal Hub" });
    const chat = screen.getByRole("tab", { name: "Chat" });
    const previousChats = screen.getByRole("list", { name: "Previous chats" });

    expect(tabs.querySelectorAll('[role="tab"]')).toHaveLength(2);
    expect(dealHub.getAttribute("aria-selected")).toBe("false");
    expect(chat.getAttribute("aria-selected")).toBe("true");
    expect(previousChats.children).toHaveLength(0);
  });

  it("shows Deal Hub navigation without leaving the Assistant route", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/assistant"]}>
        <AssistantWorkspaceSidebar activeHomeSection="assistant" tools={[]} />
        <LocationProbe />
      </MemoryRouter>,
    );

    await user.click(screen.getByRole("tab", { name: "Deal Hub" }));

    expect(screen.getByRole("tab", { name: "Deal Hub" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "Chat" }).getAttribute("aria-selected")).toBe("false");
    expect(screen.getByRole("link", { name: "Assistant", current: "page" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "New chat" })).toBeNull();
    expect(screen.getByTestId("location").textContent).toBe("/hub/assistant");

    await user.click(screen.getByRole("tab", { name: "Chat" }));

    expect(screen.getByRole("button", { name: "New chat" })).toBeTruthy();
    expect(screen.getByRole("list", { name: "Previous chats" }).children).toHaveLength(0);
    expect(screen.getByTestId("location").textContent).toBe("/hub/assistant");
  });

  it("switches and focuses tabs with arrow keys", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/assistant"]}>
        <AssistantWorkspaceSidebar activeHomeSection="assistant" tools={[]} />
      </MemoryRouter>,
    );

    const chat = screen.getByRole("tab", { name: "Chat" });
    chat.focus();
    await user.keyboard("{ArrowLeft}");

    const dealHub = screen.getByRole("tab", { name: "Deal Hub" });
    expect(dealHub.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(dealHub);
    expect(screen.getByRole("link", { name: "Assistant", current: "page" })).toBeTruthy();
  });

  it("exposes the new-chat action", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/assistant"]}>
        <AssistantWorkspaceSidebar activeHomeSection="assistant" tools={[]} />
      </MemoryRouter>,
    );

    await user.click(screen.getByRole("button", { name: "New chat" }));

    expect(startNewChat).toHaveBeenCalledTimes(1);
  });

  it("centers the new-chat action when the sidebar is collapsed", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/assistant"]}>
        <AssistantWorkspaceSidebar activeHomeSection="assistant" tools={[]} />
      </MemoryRouter>,
    );

    await user.click(screen.getByRole("button", { name: "Collapse sidebar" }));

    const newChat = screen.getByRole("button", { name: "New chat" });
    expect(newChat.className).toContain("justify-center");
    expect(newChat.className).toContain("px-0");
    expect(newChat.querySelectorAll("svg")).toHaveLength(1);
  });
});

function LocationProbe() {
  return <span data-testid="location">{useLocation().pathname}</span>;
}

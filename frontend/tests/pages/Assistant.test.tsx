// @vitest-environment happy-dom

import type { ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Assistant } from "@/pages/Assistant";

const { shellProps } = vi.hoisted(() => ({ shellProps: vi.fn() }));

vi.mock("@/app/WorkspaceProvider", () => ({
  useWorkspace: () => ({ email: "analyst@example.com" }),
}));
vi.mock("@/components/chat/QueryChat", () => ({
  QueryChatThread: () => <div>Assistant chat</div>,
}));
vi.mock("@/components/chat/QueryChatRuntimeProvider", () => ({
  QueryChatRuntimeProvider: ({ children }: { children: ReactNode }) => children,
}));
vi.mock("@/components/hub/WorkspaceHomeShell", () => ({
  WorkspaceHomeShell: ({ children, ...props }: {
    activeHomeSection?: string;
    children: ReactNode;
    contentMode?: string;
    header?: ReactNode;
    sidebarMode?: string;
  }) => {
    shellProps(props);
    return <>{props.header}{children}</>;
  },
}));

afterEach(cleanup);

describe("Assistant", () => {
  it("uses the Assistant chat sidebar, fill layout, and unchanged header title", () => {
    render(<Assistant />);

    expect(screen.getByText("Assistant chat")).toBeTruthy();
    expect(screen.getByText("Summarize")).toBeTruthy();
    expect(shellProps).toHaveBeenCalledWith(expect.objectContaining({
      activeHomeSection: "assistant",
      contentMode: "fill",
      sidebarMode: "assistant",
    }));
  });
});

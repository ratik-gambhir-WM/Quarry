// @vitest-environment happy-dom

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { QueryModelInput, SendQueryEventHandlers } from "@/contracts/quarryApi";
import { QueryChat } from "@/components/chat/QueryChat";

const { queryModel, useReducedMotion } = vi.hoisted(() => ({
  queryModel: vi.fn(),
  useReducedMotion: vi.fn(() => false),
}));

vi.mock("@quarry/runtime", () => ({ runtime: { api: { queryModel } } }));
vi.mock("motion/react", () => ({
  motion: {
    div: ({ children, layout: _layout, layoutId, transition, ...props }: {
      children: React.ReactNode;
      layout?: boolean;
      layoutId?: string;
      transition?: { duration?: number };
    }) => (
      <div
        data-layout-duration={transition?.duration}
        data-layout-id={layoutId}
        {...props}
      >
        {children}
      </div>
    ),
  },
  useReducedMotion,
}));

afterEach(cleanup);

describe("QueryChat", () => {
  beforeEach(() => {
    queryModel.mockReset();
    useReducedMotion.mockReturnValue(false);
  });

  it("uses one composer for the empty-to-streaming transition and renders authoritative Markdown", async () => {
    const user = userEvent.setup();
    const cleanupStream = vi.fn();
    let handlers: SendQueryEventHandlers | undefined;
    queryModel.mockImplementation((_input: QueryModelInput, nextHandlers: SendQueryEventHandlers) => {
      handlers = nextHandlers;
      return cleanupStream;
    });
    render(<QueryChat />);

    expect(screen.getByRole("heading", { name: "How can I help you today?" })).toBeTruthy();
    expect(screen.getAllByRole("textbox", { name: "Message input" })).toHaveLength(1);
    expect(screen.getByRole("button", { name: /Summarize deal risks/ })).toBeTruthy();
    const composer = screen.getByRole("textbox", { name: "Message input" });
    const composerRoot = composer.closest('[data-slot="query-composer"]');
    expect(composerRoot?.hasAttribute("data-compact")).toBe(false);
    expect(composer.getAttribute("rows")).toBe("2");

    await user.click(screen.getByRole("button", { name: /Summarize deal risks/ }));

    expect(queryModel).toHaveBeenCalledWith(
      {
        context: [],
        files: [],
        prompt: "Summarize the most important deal risks and open questions.",
      },
      expect.any(Object),
    );
    expect(screen.getByText("Summarize the most important deal risks and open questions.")).toBeTruthy();
    expect(screen.getByRole("status", { name: "Assistant is typing" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Stop generating" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Message input" })).toBe(composer);
    expect(composerRoot?.hasAttribute("data-compact")).toBe(true);
    expect(composer.getAttribute("rows")).toBe("1");

    await act(async () => handlers?.onEvent({ delta: "# Finding", type: "delta" }));
    expect(await screen.findByRole("heading", { name: "Finding" })).toBeTruthy();
    await act(async () => handlers?.onEvent({
      response: "<script>alert('unsafe')</script>\n\n# Final answer",
      type: "completed",
    }));

    expect(await screen.findByRole("heading", { name: "Final answer" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Finding" })).toBeNull();
    expect(document.querySelector("script")).toBeNull();
    expect(screen.getByRole("button", { name: "Copy response" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retry response" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Send message" })).toBeTruthy();
    expect(cleanupStream).toHaveBeenCalledTimes(1);
  });

  it("supports keyboard submission while preserving Shift+Enter and IME composition", async () => {
    const user = userEvent.setup();
    queryModel.mockImplementation((_input: QueryModelInput, handlers: SendQueryEventHandlers) => {
      handlers.onEvent({ response: "Done", type: "completed" });
      return () => undefined;
    });
    render(<QueryChat />);
    const input = screen.getByRole("textbox", { name: "Message input" });

    expect((screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement).disabled)
      .toBe(true);
    await user.type(input, "First line");
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    fireEvent.keyDown(input, { isComposing: true, key: "Enter" });
    expect(queryModel).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(queryModel).toHaveBeenCalledTimes(1));
    expect(queryModel.mock.calls[0]?.[0].prompt).toContain("First line");
  });

  it("stops a run without discarding partial output", async () => {
    const user = userEvent.setup();
    const cleanupStream = vi.fn();
    let handlers: SendQueryEventHandlers | undefined;
    queryModel.mockImplementation((_input: QueryModelInput, nextHandlers: SendQueryEventHandlers) => {
      handlers = nextHandlers;
      return cleanupStream;
    });
    render(<QueryChat />);

    await user.type(screen.getByRole("textbox", { name: "Message input" }), "Start streaming");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await act(async () => handlers?.onEvent({ delta: "Partial answer", type: "delta" }));
    await user.click(screen.getByRole("button", { name: "Stop generating" }));

    expect(screen.getByText("Partial answer")).toBeTruthy();
    expect(await screen.findByText("Response stopped.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Send message" })).toBeTruthy();
    expect(cleanupStream).toHaveBeenCalledTimes(1);
  });

  it("cancels the active transport when the chat unmounts", async () => {
    const user = userEvent.setup();
    const cleanupStream = vi.fn();
    queryModel.mockReturnValue(cleanupStream);
    const { unmount } = render(<QueryChat />);

    await user.type(screen.getByRole("textbox", { name: "Message input" }), "Unmount this run");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    unmount();

    expect(cleanupStream).toHaveBeenCalledTimes(1);
  });

  it("shows sanitized failures and retries through the assistant-ui runtime", async () => {
    const user = userEvent.setup();
    const handlerRuns: SendQueryEventHandlers[] = [];
    queryModel.mockImplementation((_input: QueryModelInput, handlers: SendQueryEventHandlers) => {
      handlerRuns.push(handlers);
      return () => undefined;
    });
    render(<QueryChat />);

    await user.type(screen.getByRole("textbox", { name: "Message input" }), "Retry this");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await act(async () => handlerRuns[0]?.onEvent({ error: "raw provider detail", type: "failed" }));

    expect(await screen.findByText("The assistant could not complete the response. Please try again.")).toBeTruthy();
    expect(screen.queryByText("raw provider detail")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() => expect(queryModel).toHaveBeenCalledTimes(2));
    expect(queryModel.mock.calls[1]?.[0]).toEqual({ context: [], files: [], prompt: "Retry this" });
  });

  it("disables composer interpolation when reduced motion is requested", () => {
    useReducedMotion.mockReturnValue(true);
    queryModel.mockReturnValue(() => undefined);

    const { container } = render(<QueryChat />);

    expect(container.querySelector("[data-query-composer-host]")?.getAttribute("data-layout-duration"))
      .toBe("0");
  });
});

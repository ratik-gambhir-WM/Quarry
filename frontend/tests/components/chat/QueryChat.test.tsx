// @vitest-environment happy-dom

import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  RunAssistantThreadInput,
  SendQueryEventHandlers,
} from "@/contracts/quarryApi";
import { QueryChat } from "@/components/chat/QueryChat";

const { runAssistantThread, createAssistantThread, listAssistantThreads, useReducedMotion } = vi.hoisted(() => ({
  runAssistantThread: vi.fn(),
  createAssistantThread: vi.fn(),
  listAssistantThreads: vi.fn(),
  useReducedMotion: vi.fn(() => false),
}));

vi.mock("@quarry/runtime", () => ({ runtime: { api: { runAssistantThread, createAssistantThread, listAssistantThreads } } }));
vi.mock("motion/react", () => ({
  motion: {
    div: ({
      children,
      initial: _initial,
      animate: _animate,
      layout: _layout,
      layoutId,
      transition,
      ...props
    }: {
      children: React.ReactNode;
      initial?: unknown;
      animate?: unknown;
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
    runAssistantThread.mockReset();
    createAssistantThread.mockReset().mockResolvedValue({threadId: "thread-1"});
    listAssistantThreads.mockReset().mockResolvedValue({threads: [], nextCursor: null});
    useReducedMotion.mockReturnValue(false);
  });

  it("uses one composer for the empty-to-streaming transition and renders authoritative Markdown", async () => {
    const user = userEvent.setup();
    const cleanupStream = vi.fn();
    let handlers: SendQueryEventHandlers | undefined;
    runAssistantThread.mockImplementation(
      (_input: RunAssistantThreadInput, nextHandlers: SendQueryEventHandlers) => {
        handlers = nextHandlers;
        return cleanupStream;
      },
    );
    render(<QueryChat userEmail="analyst@example.com" />);

    expect(
      screen.getByRole("heading", { name: "How can I help you today?" }),
    ).toBeTruthy();
    expect(
      screen.getAllByRole("textbox", { name: "Message input" }),
    ).toHaveLength(1);
    expect(
      screen.queryByRole("button", { name: /Summarize deal risks/ }),
    ).toBeNull();
    const composer = screen.getByRole("textbox", { name: "Message input" });
    const composerRoot = composer.closest('[data-slot="query-composer"]');
    expect(composerRoot?.hasAttribute("data-compact")).toBe(false);
    expect(composer.getAttribute("rows")).toBe("2");

    await user.type(
      composer,
      "Summarize the most important deal risks and open questions.",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));

    expect(runAssistantThread).toHaveBeenCalledWith(
      expect.objectContaining({
        threadId: "thread-1",
        files: [],
        prompt: "Summarize the most important deal risks and open questions.",
      }),
      expect.any(Object),
    );
    expect(
      screen.getByText(
        "Summarize the most important deal risks and open questions.",
      ),
    ).toBeTruthy();
    expect(
      screen.getByRole("status", { name: "Assistant is typing" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Stop generating" }),
    ).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Message input" })).toBe(
      composer,
    );
    expect(composerRoot?.hasAttribute("data-compact")).toBe(true);
    expect(composer.getAttribute("rows")).toBe("1");

    await act(async () =>
      handlers?.onEvent({ delta: "# Finding", type: "delta" }),
    );
    expect(
      await screen.findByRole("heading", { name: "Finding" }),
    ).toBeTruthy();
    await act(async () =>
      handlers?.onEvent({
        response: "<script>alert('unsafe')</script>\n\n# Final answer",
        type: "completed",
      }),
    );

    expect(
      await screen.findByRole("heading", { name: "Final answer" }),
    ).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Finding" })).toBeNull();
    expect(document.querySelector("script")).toBeNull();
    const copyButton = screen.getByRole("button", { name: "Copy response" });
    await user.click(copyButton);
    expect(await screen.findByText("Response copied.")).toBeTruthy();
    await user.keyboard("{ArrowRight}");
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Retry response" }));
    expect(screen.getByRole("button", { name: "Retry response" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Send message" })).toBeTruthy();
    expect(cleanupStream).toHaveBeenCalledTimes(1);
  });

  it("supports keyboard submission while preserving Shift+Enter and IME composition", async () => {
    const user = userEvent.setup();
    runAssistantThread.mockImplementation(
      (_input: RunAssistantThreadInput, handlers: SendQueryEventHandlers) => {
        handlers.onEvent({ response: "Done", type: "completed" });
        return () => undefined;
      },
    );
    render(<QueryChat userEmail="analyst@example.com" />);
    const input = screen.getByRole("textbox", { name: "Message input" });

    expect(
      (
        screen.getByRole("button", {
          name: "Send message",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    await user.type(input, "First line");
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    fireEvent.keyDown(input, { isComposing: true, key: "Enter" });
    expect(runAssistantThread).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(runAssistantThread).toHaveBeenCalledTimes(1));
    expect(runAssistantThread.mock.calls[0]?.[0].prompt).toContain("First line");
  });

  it("stops a run without discarding partial output", async () => {
    const user = userEvent.setup();
    const cleanupStream = vi.fn();
    let handlers: SendQueryEventHandlers | undefined;
    runAssistantThread.mockImplementation(
      (_input: RunAssistantThreadInput, nextHandlers: SendQueryEventHandlers) => {
        handlers = nextHandlers;
        return cleanupStream;
      },
    );
    render(<QueryChat userEmail="analyst@example.com" />);

    await user.type(
      screen.getByRole("textbox", { name: "Message input" }),
      "Start streaming",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await act(async () =>
      handlers?.onEvent({ delta: "Partial answer", type: "delta" }),
    );
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Message input" }), { key: "Escape" });

    expect(screen.getByText("Partial answer")).toBeTruthy();
    expect(await screen.findByText("Response stopped.")).toBeTruthy();
    expect(await screen.findByRole("button", { name: "Send message" })).toBeTruthy();
    expect(cleanupStream).toHaveBeenCalledTimes(1);
  });

  it("cancels the active transport when the chat unmounts", async () => {
    const user = userEvent.setup();
    const cleanupStream = vi.fn();
    runAssistantThread.mockReturnValue(cleanupStream);
    const { unmount } = render(<QueryChat userEmail="analyst@example.com" />);

    await user.type(
      screen.getByRole("textbox", { name: "Message input" }),
      "Unmount this run",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    unmount();

    expect(cleanupStream).toHaveBeenCalledTimes(1);
  });

  it("shows sanitized failures and retries through the custom AgentRuntime", async () => {
    const user = userEvent.setup();
    const handlerRuns: SendQueryEventHandlers[] = [];
    runAssistantThread.mockImplementation(
      (_input: RunAssistantThreadInput, handlers: SendQueryEventHandlers) => {
        handlerRuns.push(handlers);
        return () => undefined;
      },
    );
    render(<QueryChat userEmail="analyst@example.com" />);

    await user.type(
      screen.getByRole("textbox", { name: "Message input" }),
      "Retry this",
    );
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await act(async () =>
      handlerRuns[0]?.onEvent({ error: "raw provider detail", type: "failed" }),
    );

    expect(
      await screen.findByText(
        "The assistant could not complete the response. Please try again.",
      ),
    ).toBeTruthy();
    expect(screen.queryByText("raw provider detail")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));

    await waitFor(() => expect(runAssistantThread).toHaveBeenCalledTimes(2));
    expect(runAssistantThread.mock.calls[1]?.[0]).toMatchObject({
      threadId: "thread-1", files: [], prompt: "Retry this",
    });
  });

  it("sends picked files and a chosen model, and preserves the files for retry", async () => {
    const user = userEvent.setup();
    const handlerRuns: SendQueryEventHandlers[] = [];
    runAssistantThread.mockImplementation((_input, handlers) => {
      handlerRuns.push(handlers);
      return () => undefined;
    });
    render(<QueryChat userEmail="analyst@example.com" />);
    const file = new File(["Review notes"], "notes.txt", { type: "text/plain" });
    await user.upload(screen.getByLabelText("Choose attachments"), file);
    expect(screen.getByRole("button", { name: "Remove notes.txt" })).toBeTruthy();
    await user.click(screen.getByRole("combobox", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /GPT-5.5/ }));
    await user.type(screen.getByRole("textbox", { name: "Message input" }), "Review the attached notes");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(runAssistantThread).toHaveBeenCalledTimes(1));
    expect(runAssistantThread.mock.calls[0]?.[0]).toMatchObject({ model: "gpt-5.5", files: [file] });
    expect(screen.queryByRole("button", { name: "Remove notes.txt" })).toBeNull();
    expect(screen.getByRole("list", { name: "Attached files" })).toBeTruthy();
    await act(async () => handlerRuns[0]?.onEvent({ type: "failed", error: "test failure" }));
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(runAssistantThread).toHaveBeenCalledTimes(2));
    expect(runAssistantThread.mock.calls[1]?.[0]).toMatchObject({ model: "gpt-5.5", files: [file] });
  });

  it("supports custom model IDs, attachment removal, drop and paste validation", async () => {
    const user = userEvent.setup();
    runAssistantThread.mockReturnValue(() => undefined);
    render(<QueryChat userEmail="analyst@example.com" />);
    const input = screen.getByRole("textbox", { name: "Message input" });
    const composer = input.closest("form");
    const file = new File(["Content"], "drop.txt", { type: "text/plain" });
    if (!composer) throw new Error("Composer form is missing");
    fireEvent.drop(composer, { dataTransfer: { types: ["Files"], files: [file] } });
    expect(screen.getByRole("button", { name: "Remove drop.txt" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Remove drop.txt" }));
    expect(screen.queryByRole("list", { name: "Attachments" })).toBeNull();
    fireEvent.paste(input, { clipboardData: { files: [new File(["x"], "unsafe.exe")] } });
    expect(screen.getByRole("alert").textContent).toContain("this file type isn’t supported");
    await user.click(screen.getByRole("combobox", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: /Custom model ID/ }));
    await user.type(input, "Use this model");
    expect((screen.getByRole("button", { name: "Send message" }) as HTMLButtonElement).disabled).toBe(true);
    await user.type(screen.getByRole("textbox", { name: "Custom model ID" }), "custom-model-id");
    await user.click(screen.getByRole("button", { name: "Send message" }));
    await waitFor(() => expect(runAssistantThread).toHaveBeenCalledTimes(1));
    expect(runAssistantThread.mock.calls[0]?.[0].model).toBe("custom-model-id");
  });

  it("disables composer interpolation when reduced motion is requested", () => {
    useReducedMotion.mockReturnValue(true);
    runAssistantThread.mockReturnValue(() => undefined);

    const { container } = render(<QueryChat userEmail="analyst@example.com" />);

    expect(
      container
        .querySelector("[data-query-composer-host]")
        ?.getAttribute("data-layout-duration"),
    ).toBe("0");
  });
});

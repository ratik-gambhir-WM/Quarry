// @vitest-environment happy-dom
import { type ReactNode, useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PromptInput } from "@/components/stealth/prompt-input";
import { loadMentionFiles, mentionAt } from "@/components/chat/fileMentions";
import type { ChatFileReference } from "@/contracts/quarryApi";
const api = vi.hoisted(() => ({ listDeals: vi.fn(), listDealDocuments: vi.fn() }));
vi.mock("@quarry/runtime", () => ({ runtime: { api } }));
vi.mock("motion/react", () => ({
  motion: { div: ({ children }: { children: ReactNode }) => <div>{children}</div> },
  useReducedMotion: () => true,
}));
afterEach(cleanup);
beforeEach(() => {
  vi.resetAllMocks();
  api.listDeals.mockResolvedValue([{ dealId: "DEAL-A", dealName: "Alpha" }, { dealId: "DEAL-B", dealName: "Beta" }]);
  api.listDealDocuments.mockImplementation(async (id: string) => [{ fileId: `file-${id}`, displayName: id === "DEAL-A" ? "Architecture.pdf" : "Budget.xlsx" }]);
});
function Example({ dealId = null, send = vi.fn() }: { dealId?: string | null; send?: ReturnType<typeof vi.fn> }) {
  const [value, setValue] = useState("");
  const [references, setReferences] = useState<readonly ChatFileReference[]>([]);
  return <PromptInput value={value} onValueChange={setValue} references={references} onReferencesChange={setReferences} scopeDealId={dealId}
    onSend={() => send({ value, references })} onStop={vi.fn()} generating={false} disabled={false} compact={false} condensed={false}
    files={[]} onFilesAdd={vi.fn()} onFileRemove={vi.fn()} model={{ type: "default" }} onModelChange={vi.fn()} error={null} />;
}
describe("file mentions", () => {
  it("only triggers at a word boundary and reads the query at the caret", () => {
    expect(mentionAt("user@example.com", 16)).toBeNull();
    expect(mentionAt("Review @arch then", 12)).toEqual({ start: 7, end: 12, query: "arch" });
  });
  it("loads files across deals in Assistant and only the active data room when scoped", async () => {
    expect(await loadMentionFiles(api, null)).toHaveLength(2);
    api.listDeals.mockClear(); api.listDealDocuments.mockClear();
    const result = await loadMentionFiles(api, "DEAL-B");
    expect(api.listDeals).not.toHaveBeenCalled();
    expect(api.listDealDocuments).toHaveBeenCalledExactlyOnceWith("DEAL-B");
    expect(result[0].name).toBe("Budget.xlsx");
  });
  it("filters case-insensitively, selects with keyboard, preserves IDs during text edits, and removes with Backspace", async () => {
    const user = userEvent.setup(); const send = vi.fn();
    render(<Example send={send} />);
    const input = screen.getByRole("textbox", { name: "Message input" }) as HTMLTextAreaElement;
    await user.type(input, "Review @ARCH");
    await screen.findByRole("option", { name: /Architecture.pdf/ });
    expect(screen.queryByRole("option", { name: /Budget/ })).toBeNull();
    await user.keyboard("{Enter}");
    expect(input.value).toBe("Review ");
    expect(screen.getByRole("button", { name: "Remove reference Architecture.pdf" })).toBeTruthy();
    fireEvent.change(input, { target: { value: "Summarize the risks." } });
    await user.click(screen.getByRole("button", { name: "Send message" }));
    expect(send).toHaveBeenCalledWith({ value: "Summarize the risks.", references: [{ dealId: "DEAL-A", fileId: "file-DEAL-A", name: "Architecture.pdf" }] });
    input.focus(); input.setSelectionRange(0, 0); await user.keyboard("{Backspace}");
    expect(screen.queryByRole("button", { name: "Remove reference Architecture.pdf" })).toBeNull();
  });
  it("selects multiple files by mouse, prevents duplicates, and Escape dismisses only the picker", async () => {
    const user = userEvent.setup(); render(<Example />);
    const input = screen.getByRole("textbox", { name: "Message input" });
    await user.type(input, "@");
    await user.click(await screen.findByRole("option", { name: /Architecture/ }));
    await user.type(input, "@");
    await screen.findByRole("option", { name: /Budget/ });
    expect(screen.queryByRole("option", { name: /Architecture/ })).toBeNull();
    await user.keyboard("{ArrowDown}{Enter}");
    expect(screen.getByRole("button", { name: "Remove reference Budget.xlsx" })).toBeTruthy();
    await user.type(input, "@");
    await screen.findByText("No files available.");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
  });
  it("shows loading and recoverable errors without falling back to fixture files", async () => {
    const user = userEvent.setup(); api.listDealDocuments.mockRejectedValueOnce(new Error("offline"));
    render(<Example dealId="DEAL-A" />);
    await user.type(screen.getByRole("textbox", { name: "Message input" }), "@");
    await screen.findByRole("alert");
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByRole("option", { name: /Architecture/ });
    expect(api.listDeals).not.toHaveBeenCalled();
  });
  it("bounds the rendered results and keyboard navigation reaches later files", async () => {
    api.listDealDocuments.mockResolvedValue(Array.from({ length: 150 }, (_, i) => ({ fileId: `f-${i}`, displayName: `${String(i).padStart(3, "0")}.pdf` })));
    const user = userEvent.setup(); render(<Example dealId="DEAL-A" />);
    const input = screen.getByRole("textbox", { name: "Message input" });
    await user.type(input, "@");
    await waitFor(() => expect(screen.getAllByRole("option")).toHaveLength(50));
    for (let i = 0; i < 80; i++) fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getByRole("option", { selected: true }).textContent).toContain("080.pdf");
  });
});

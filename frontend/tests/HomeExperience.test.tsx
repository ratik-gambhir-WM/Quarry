// @vitest-environment happy-dom
import { act, cleanup, render, renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AiSearchCard } from "@/components/hub/cards/AiSearchCard";
import { SuggestedContentCard } from "@/components/hub/cards/SuggestedContentCard";
import { useAgentRuntime } from "@/components/chat/AgentRuntime";

afterEach(cleanup);

function Destination() {
  const location = useLocation();
  return <output data-testid="destination">{JSON.stringify({ path: location.pathname, state: location.state })}</output>;
}

describe("Home experience", () => {
  it("opens a reviewed draft in Assistant, with no empty submission", async () => {
    const user = userEvent.setup();
    render(<MemoryRouter initialEntries={["/hub"]}><AiSearchCard suggestions={["Plan a review"]} /><Destination /></MemoryRouter>);
    expect((screen.getByRole("button", { name: "Open Assistant" }) as HTMLButtonElement).disabled).toBe(true);
    await user.click(screen.getByRole("button", { name: "Plan a review" }));
    const input = screen.getByRole("textbox", { name: "Your question" });
    expect(document.activeElement).toBe(input);
    expect((input as HTMLTextAreaElement).value).toBe("Plan a review");
    await user.click(screen.getByRole("button", { name: "Open Assistant" }));
    expect(screen.getByTestId("destination").textContent).toBe(JSON.stringify({ path: "/hub/assistant", state: { assistantDraft: "Plan a review" } }));
  });

  it("prefills the runtime once without sending or replacing later edits", () => {
    const queryModel = vi.fn();
    const api = { queryModel };
    const onContextTruncated = vi.fn();
    const { result, rerender } = renderHook(({ initialDraft }) => useAgentRuntime({ api, userEmail: "", onContextTruncated, initialDraft }), { initialProps: { initialDraft: "Review this draft" } });
    expect(result.current.state.draft).toBe("Review this draft");
    expect(result.current.state.messages).toEqual([]);
    expect(queryModel).not.toHaveBeenCalled();
    act(() => result.current.setDraft("My edits"));
    rerender({ initialDraft: "Review this draft" });
    expect(result.current.state.draft).toBe("My edits");
    act(() => result.current.newChat());
    expect(result.current.state.draft).toBe("");
  });

  it("labels samples, avoids inactive file buttons, and supports keyboard tabs", async () => {
    const user = userEvent.setup();
    render(<SuggestedContentCard />);
    expect(screen.getByText("Sample content")).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
    screen.getByRole("tab", { name: "Bookmarks" }).focus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "Drafts", selected: true })).toBeTruthy();
    expect(screen.getByRole("tabpanel").getAttribute("aria-labelledby")).toBe(screen.getByRole("tab", { name: "Drafts" }).id);
  });
});

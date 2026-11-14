// @vitest-environment happy-dom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { DealScopeView } from "@/components/deal-room/DealScopeView";

const questions = ["What drives growth?", "Which contracts can terminate?", "How resilient is the hosting infrastructure?"];
afterEach(cleanup);

describe("DealScopeView", () => {
  it("preserves question text and order, with only the first answer expanded initially", () => {
    const { container } = render(<DealScopeView questions={questions} />);
    expect(screen.getByRole("heading", { level: 1, name: "Key Questions" })).toBeTruthy();
    const triggers = questions.map((name) => screen.getByRole("button", { name }));
    expect(triggers.map((button) => button.getAttribute("aria-expanded"))).toEqual(["true", "false", "false"]);
    const markup = container.textContent ?? "";
    expect(markup.indexOf(questions[0])).toBeLessThan(markup.indexOf(questions[1]));
    expect(screen.getAllByText("No answer yet.")).toHaveLength(1);
    expect(screen.getByRole("status").textContent).toBe("3 questions");
    expect(screen.queryByRole("table")).toBeNull();
  });

  it("supports keyboard navigation and independent expansion", async () => {
    const user = userEvent.setup();
    render(<DealScopeView questions={questions} />);
    const first = screen.getByRole("button", { name: questions[0] });
    const second = screen.getByRole("button", { name: questions[1] });
    first.focus();
    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(second);
    await user.keyboard("{Enter}");
    expect(second.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getAllByText("No answer yet.")).toHaveLength(2);
    await user.keyboard("{Enter}");
    expect(second.getAttribute("aria-expanded")).toBe("false");
    expect(first.getAttribute("aria-expanded")).toBe("true");
  });

  it("filters locally without renumbering and restores disclosure state and search focus on clear", async () => {
    const user = userEvent.setup();
    render(<DealScopeView questions={questions} />);
    const search = screen.getByRole("searchbox", { name: "Search questions" });
    await user.type(search, " HOSTING ");
    expect(screen.getByRole("status").textContent).toBe("1 of 3 questions");
    expect(screen.queryByRole("button", { name: questions[0] })).toBeNull();
    expect(screen.getByText("03")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: questions[2] }));
    await user.click(screen.getByRole("button", { name: "Clear question search" }));
    expect(document.activeElement).toBe(search);
    expect(screen.getByRole("button", { name: questions[0] }).getAttribute("aria-expanded")).toBe("true");
    expect(screen.getByRole("button", { name: questions[2] }).getAttribute("aria-expanded")).toBe("true");
  });

  it("distinguishes no search matches from missing source questions", async () => {
    const user = userEvent.setup();
    render(<DealScopeView questions={questions} />);
    await user.type(screen.getByRole("searchbox"), "no-such-question");
    expect(screen.getByText(/No questions match your search/)).toBeTruthy();
    expect(screen.getByRole("status").textContent).toBe("0 of 3 questions");
    await user.click(screen.getByRole("button", { name: "Clear question search" }));
    expect(screen.getByRole("button", { name: questions[0] }).getAttribute("aria-expanded")).toBe("true");
  });

  it("renders the existing empty source state", () => {
    render(<DealScopeView questions={[]} />);
    expect(screen.getByText("No key questions were extracted from the submitted source files.")).toBeTruthy();
    expect(screen.getByRole("searchbox")).toHaveProperty("disabled", true);
    expect(screen.getByRole("status").textContent).toBe("0 questions");
  });

  it("renders every question without hidden pagination", () => {
    const manyQuestions = Array.from({ length: 12 }, (_, index) => `Question ${index + 1}?`);
    render(<DealScopeView questions={manyQuestions} />);
    expect(screen.getByRole("button", { name: "Question 12?" })).toBeTruthy();
  });
});

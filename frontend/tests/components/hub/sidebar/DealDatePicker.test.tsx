// @vitest-environment happy-dom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { DatePicker, parseDateInput } from "@/components/ui/stealth-date/date-picker";
import { addMonths, parseISODate, startOfMonth, toISODate } from "@/components/ui/stealth-date/calendar";

function Picker({ onSubmit = () => {} }: { onSubmit?: () => void }) {
  const [value, setValue] = useState<Date | null>(parseISODate("2026-10-15"));
  return (
    <form onSubmit={(event) => { event.preventDefault(); onSubmit(); }}>
      <label htmlFor="date">Start date</label>
      <DatePicker id="date" locale="en-US" calendarLabel="start date" value={value} onValueChange={setValue} />
      <label htmlFor="other">Other field</label><input id="other" />
      <output data-testid="date-value">{value ? toISODate(value) : "empty"}</output>
      <button type="submit">Continue</button>
    </form>
  );
}

// Happy DOM rejects finished animation promises when Motion cancels Web Animations.
// Exercise the same controls with Motion's JavaScript animation fallback in this test.
const animationDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
beforeAll(() => { Reflect.deleteProperty(Element.prototype, "animate"); });
afterAll(() => {
  if (animationDescriptor) Object.defineProperty(Element.prototype, "animate", animationDescriptor);
});
afterEach(cleanup);

describe("deal date picker", () => {
  it("reads relative dates and validates real calendar dates", () => {
    const today = new Date(2026, 9, 30);
    expect(toISODate(parseDateInput("next fri", { today, locale: "en-US" })!)).toBe("2026-11-06");
    expect(toISODate(parseDateInput("tomorrow", { today, locale: "en-US" })!)).toBe("2026-10-31");
    expect(parseDateInput("2026-02-30", { today })).toBeNull();
    expect(toISODate(parseISODate("0004-02-29")!)).toBe("0004-02-29");
    expect(parseISODate("2026-02-30")).toBeNull();
    expect(toISODate(startOfMonth(parseISODate("0004-02-29")!))).toBe("0004-02-01");
    expect(toISODate(addMonths(parseISODate("0004-02-29")!, 1))).toBe("0004-03-29");
  });

  it("previews typed input, commits and clears a date without submitting", async () => {
    const user = userEvent.setup();
    const submit = vi.fn();
    render(<Picker onSubmit={submit} />);
    const input = screen.getByLabelText("Start date");
    await user.clear(input);
    await user.type(input, "2026-02-28");
    expect(screen.getByTestId("date-value").textContent).toBe("2026-10-15");
    await user.keyboard("{Enter}");
    expect(screen.getByTestId("date-value").textContent).toBe("2026-02-28");
    expect(submit).not.toHaveBeenCalled();
    await user.clear(input);
    fireEvent.blur(input);
    expect(screen.getByTestId("date-value").textContent).toBe("empty");
  });

  it("keeps invalid typed input from submitting an older date", async () => {
    const user = userEvent.setup();
    const submit = vi.fn();
    render(<Picker onSubmit={submit} />);
    const input = screen.getByLabelText("Start date") as HTMLInputElement;
    await user.clear(input);
    await user.type(input, "2026-02-30");
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(submit).not.toHaveBeenCalled();
    expect(input.validity.valid).toBe(false);
    expect(screen.getByTestId("date-value").textContent).toBe("2026-10-15");
    expect(screen.getByText(/Couldn’t read that/)).toBeDefined();
  });

  it("lets a click outside the calendar move focus to another field", async () => {
    const user = userEvent.setup();
    render(<Picker />);
    await user.click(screen.getByRole("button", { name: /Choose start date/ }));
    await user.click(screen.getByLabelText("Other field"));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByLabelText("Other field")));
  });

  it("navigates and selects dates in the calendar, returning focus to the field", async () => {
    const user = userEvent.setup();
    render(<Picker />);
    await user.click(screen.getByRole("button", { name: /Choose start date/ }));
    await user.click(screen.getByRole("button", { name: "Next month" }));
    const day = await screen.findByRole("button", { name: "Sunday, November 1, 2026" });
    await user.click(day);
    await waitFor(() => expect(screen.getByTestId("date-value").textContent).toBe("2026-11-01"));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByLabelText("Start date")));
  });
});

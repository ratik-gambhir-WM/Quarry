// @vitest-environment happy-dom
import { useRef, useState } from "react";
import { cleanup, createEvent, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Input } from "@/components/ui/input";
import { CheckboxField } from "@/components/ui/checkbox-field";
import { PasswordField } from "@/components/ui/password-field";
import { SearchField } from "@/components/ui/search-field";

function SearchExample() {
  const [value, setValue] = useState("");
  return <SearchField aria-label="Search notes" clearLabel="Clear note search" value={value} onValueChange={setValue} />;
}

function InputExample() {
  const ref = useRef<HTMLInputElement>(null);
  return <form onSubmit={(event) => event.preventDefault()}><label htmlFor="native">Name</label><Input id="native" name="name" ref={ref} required /><button type="button" onClick={() => ref.current?.focus()}>Focus name</button></form>;
}

afterEach(cleanup);

describe("Stealth input adaptations", () => {
  it("keeps native uncontrolled values, validation and forwarded focus", async () => {
    const user = userEvent.setup();
    render(<InputExample />);
    const field = screen.getByLabelText("Name") as HTMLInputElement;
    expect(field.validity.valueMissing).toBe(true);
    await user.type(field, "Analyst");
    expect(field.value).toBe("Analyst");
    await user.click(screen.getByRole("button", { name: "Focus name" }));
    expect(document.activeElement).toBe(field);
    expect(new FormData(field.form ?? undefined).get("name")).toBe("Analyst");
  });

  it("retains native checkbox keyboard behavior", async () => {
    const user = userEvent.setup();
    render(<label><CheckboxField name="scope" value="security" /> Security</label>);
    const checkbox = screen.getByRole("checkbox", { name: "Security" }) as HTMLInputElement;
    checkbox.focus();
    await user.keyboard(" ");
    expect(checkbox.checked).toBe(true);
    await user.keyboard(" ");
    expect(checkbox.checked).toBe(false);
  });

  it("clears a search and returns focus to the input", async () => {
    const user = userEvent.setup();
    render(<SearchExample />);
    const field = screen.getByRole("searchbox", { name: "Search notes" }) as HTMLInputElement;
    await user.type(field, "quarry");
    await user.click(screen.getByRole("button", { name: "Clear note search" }));
    expect(field.value).toBe("");
    expect(document.activeElement).toBe(field);
  });

  it("reveals an autofilled password without losing it and announces Caps Lock", async () => {
    const user = userEvent.setup();
    const submit = vi.fn();
    render(<form onSubmit={(event) => { event.preventDefault(); submit(new FormData(event.currentTarget).get("password")); }}><label htmlFor="password">Password</label><PasswordField id="password" name="password" autoComplete="current-password" /><button type="submit">Submit</button></form>);
    const field = screen.getByLabelText("Password") as HTMLInputElement;
    field.value = "  secret  ";
    field.focus();
    field.setSelectionRange(2, 4);
    await user.click(screen.getByRole("button", { name: "Show password" }));
    expect(field.type).toBe("text");
    expect(field.value).toBe("  secret  ");
    expect(field.selectionStart).toBe(2);
    await user.click(screen.getByRole("button", { name: "Hide password" }));
    const capsEvent = createEvent.keyDown(field, { key: "A" });
    Object.defineProperty(capsEvent, "getModifierState", { value: (key: string) => key === "CapsLock" });
    fireEvent(field, capsEvent);
    expect(screen.getByRole("status").textContent).toBe("Caps Lock is on");
    await user.click(screen.getByRole("button", { name: "Submit" }));
    expect(submit).toHaveBeenCalledWith("  secret  ");
  });
});

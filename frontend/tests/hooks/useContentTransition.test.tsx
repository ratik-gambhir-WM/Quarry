// @vitest-environment happy-dom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useContentTransition } from "@/hooks/useContentTransition";
import { useMotionInput } from "@/hooks/useMotionInput";

const originalAnimate = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
// Only the cancellation and completion surface used by the hook needs a test double.
const animate = vi.fn(() => ({ cancel: vi.fn(), finished: new Promise<void>(() => {}) }));
let preference: MediaQueryList;

beforeEach(() => {
  preference = window.matchMedia("(prefers-reduced-motion: reduce)");
  vi.spyOn(window, "matchMedia").mockReturnValue(preference);
  vi.spyOn(preference, "matches", "get").mockReturnValue(false);
  Object.defineProperty(Element.prototype, "animate", { configurable: true, value: animate });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  animate.mockClear();
  if (originalAnimate) Object.defineProperty(Element.prototype, "animate", originalAnimate);
  else Reflect.deleteProperty(Element.prototype, "animate");
  delete document.documentElement.dataset.motionInput;
});

function Fixture({ view }: { view: string }) {
  useMotionInput();
  const ref = useContentTransition(view);
  return <div ref={ref}><input aria-label="Draft" defaultValue="Keep this draft" onKeyDown={event => event.stopPropagation()} /><p>{view}</p></div>;
}

it("animates only a changed view and preserves the existing input, draft and focus", () => {
  const { rerender } = render(<StrictMode><Fixture view="overview" /></StrictMode>);
  const input = screen.getByRole("textbox", { name: "Draft" });
  fireEvent.change(input, { target: { value: "An unsaved edit" } });
  input.focus();
  expect(animate).not.toHaveBeenCalled();
  rerender(<StrictMode><Fixture view="settings" /></StrictMode>);
  expect(animate).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("textbox", { name: "Draft" })).toBe(input);
  expect((input as HTMLInputElement).value).toBe("An unsaved edit");
  expect(document.activeElement).toBe(input);
  rerender(<StrictMode><Fixture view="settings" /></StrictMode>);
  expect(animate).toHaveBeenCalledTimes(1);
});

it("keeps keyboard changes instant and restores motion for pointer input", () => {
  const { rerender, unmount } = render(<Fixture view="overview" />);
  fireEvent.keyDown(document, { key: "ArrowRight" });
  expect(document.documentElement.dataset.motionInput).toBe("keyboard");
  rerender(<Fixture view="settings" />);
  expect(animate).not.toHaveBeenCalled();
  fireEvent.pointerDown(document);
  expect(document.documentElement.dataset.motionInput).toBe("pointer");
  rerender(<Fixture view="activity" />);
  expect(animate).toHaveBeenCalledTimes(1);
  unmount();
  fireEvent.keyDown(document, { key: "Tab" });
  expect(document.documentElement.dataset.motionInput).toBeUndefined();
});

it("cancels interrupted motion and removes listeners when the view unmounts", () => {
  const { rerender, unmount } = render(<Fixture view="overview" />);
  rerender(<Fixture view="settings" />);
  const first = animate.mock.results[0].value;
  rerender(<Fixture view="activity" />);
  expect(first.cancel).toHaveBeenCalledTimes(1);
  const second = animate.mock.results[1].value;
  fireEvent.keyDown(screen.getByRole("textbox", { name: "Draft" }), { key: "Escape" });
  expect(second.cancel).toHaveBeenCalledTimes(1);
  unmount();
  const cancellations = second.cancel.mock.calls.length;
  fireEvent.keyDown(document, { key: "Enter" });
  expect(second.cancel).toHaveBeenCalledTimes(cancellations);
});

it("skips reduced motion and cancels an active animation when the preference changes", () => {
  const matches = vi.spyOn(preference, "matches", "get");
  matches.mockReturnValue(true);
  const { rerender } = render(<Fixture view="overview" />);
  rerender(<Fixture view="settings" />);
  expect(animate).not.toHaveBeenCalled();
  matches.mockReturnValue(false);
  rerender(<Fixture view="activity" />);
  const animation = animate.mock.results[0].value;
  matches.mockReturnValue(true);
  act(() => { preference.dispatchEvent(new Event("change")); });
  expect(animation.cancel).toHaveBeenCalledTimes(1);
});

it("continues to show changed content when native animations are unavailable", () => {
  Reflect.deleteProperty(Element.prototype, "animate");
  const { rerender } = render(<Fixture view="overview" />);
  rerender(<Fixture view="settings" />);
  expect(screen.getByText("settings")).toBeTruthy();
  expect(animate).not.toHaveBeenCalled();
});

// @vitest-environment happy-dom
import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { HighlightMatch } from "@/components/ui/stealth/highlight-match";

afterEach(cleanup);

it("keeps accent and ligature highlights aligned after emoji", () => {
  const { container } = render(<HighlightMatch text="📁 Łódź Straße" query="lodz strasse" />);
  expect([...container.querySelectorAll("mark")].map((mark) => mark.textContent)).toEqual(["Łódź", "Straße"]);
  expect(container.textContent).toBe("📁 Łódź Straße");
});

it("merges overlapping words without duplicating text or interpreting markup", () => {
  const { container } = render(<HighlightMatch text="<b>liability</b>" query="liability liability lia" />);
  expect(container.querySelectorAll("mark")).toHaveLength(1);
  expect(container.querySelector("b")).toBeNull();
  expect(container.textContent).toBe("<b>liability</b>");
});

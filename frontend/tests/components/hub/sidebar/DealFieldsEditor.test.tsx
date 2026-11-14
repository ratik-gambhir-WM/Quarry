// @vitest-environment happy-dom
import { useState } from "react";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DealFieldsEditor } from "@/components/hub/sidebar/DealFieldsEditor";
import { testCompany, testDealInput } from "@/test/fixtures/deals";
import {
  dealStatusLabels,
  dealStatusOptions,
  type DealFields,
} from "@/data/dealModel";
const { api } = vi.hoisted(() => ({
  api: {
    listCompanies: vi.fn(),
    createCompany: vi.fn(),
    updateCompany: vi.fn(),
  },
}));
vi.mock("@quarry/runtime", () => ({ runtime: { api } }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.useRealTimers();
});
function Editor() {
  const [fields, setFields] = useState<DealFields>(testDealInput());
  return (
    <>
      <DealFieldsEditor
        userEmail="owner@example.com"
        fields={fields}
        onChange={setFields}
      />
      <output data-testid="fields">{JSON.stringify(fields)}</output>
    </>
  );
}
describe("DealFieldsEditor", () => {
  it("offers every diligence stage and preserves dates on close and reopening", async () => {
    const user = userEvent.setup();
    api.listCompanies.mockResolvedValue({ companies: [], hasMore: false });
    render(<Editor />);
    const status = screen.getByRole("combobox", { name: "Status" });
    expect(
      Array.from(
        status.querySelectorAll("option"),
        (option) => option.textContent,
      ),
    ).toEqual(dealStatusOptions.map((code) => dealStatusLabels[code]));
    fireEvent.change(screen.getByLabelText("Actual close (optional)"), {
      target: { value: "2026-10-01" },
    });
    fireEvent.blur(screen.getByLabelText("Actual close (optional)"));
    for (const code of ["closed", "pencils_down", "prepare", "pipeline"]) {
      await user.selectOptions(status, code);
      const fields: DealFields = JSON.parse(
        screen.getByTestId("fields").textContent ?? "{}",
      );
      expect(fields.status).toBe(code);
      expect(fields.actualCloseDate).toBe("2026-10-01");
      expect(fields.engagementCompletedDate).toBeNull();
    }
  });

  it.each(["create", "edit"] as const)(
    "cancels a %s company draft with Enter without saving",
    async (mode) => {
      const user = userEvent.setup();
      api.listCompanies.mockResolvedValue({
        companies: [testCompany("Reusable")],
        hasMore: false,
      });
      render(<Editor />);
      await user.click(
        await screen.findByRole("button", {
          name:
            mode === "create"
              ? "Create company"
              : "Edit shared company Reusable",
        }),
      );
      await user.clear(screen.getByLabelText("Company name"));
      await user.type(screen.getByLabelText("Company name"), "Unsaved company");
      screen.getByRole("button", { name: "Cancel company" }).focus();
      await user.keyboard("{Enter}");
      expect(
        screen.queryByRole("button", { name: "Cancel company" }),
      ).toBeNull();
      expect(api.createCompany).not.toHaveBeenCalled();
      expect(api.updateCompany).not.toHaveBeenCalled();
    },
  );

  it.each(["input", "button"] as const)(
    "saves a company once with Enter on its %s without submitting the deal",
    async (target) => {
      const user = userEvent.setup();
      const submitDeal = vi.fn();
      api.listCompanies.mockResolvedValue({ companies: [], hasMore: false });
      api.createCompany.mockResolvedValue(testCompany("New company"));
      render(
        <form
          onSubmit={(event) => {
            event.preventDefault();
            submitDeal();
          }}
        >
          <Editor />
        </form>,
      );
      await user.click(screen.getByRole("button", { name: "Create company" }));
      await user.type(screen.getByLabelText("Company name"), "New company");
      if (target === "button")
        screen.getByRole("button", { name: "Save company" }).focus();
      await user.keyboard("{Enter}");
      await screen.findByText(/remains available if you cancel/);
      expect(api.createCompany).toHaveBeenCalledOnce();
      expect(submitDeal).not.toHaveBeenCalled();
    },
  );

  it("reuses a company for several roles, clears optional dates, and removes assignments", async () => {
    const user = userEvent.setup();
    api.listCompanies.mockResolvedValue({
      companies: [testCompany("Reusable")],
      hasMore: false,
    });
    render(<Editor />);
    await user.click(
      await screen.findByRole("button", { name: "Add Reusable" }),
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Role for company 1" }),
      "buyer",
    );
    await user.click(screen.getByRole("button", { name: "Add Reusable" }));
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Role for company 2" }),
      "portfolio_company",
    );
    fireEvent.change(screen.getByLabelText("Expected close (optional)"), {
      target: { value: "2026-12-01" },
    });
    fireEvent.blur(screen.getByLabelText("Expected close (optional)"));
    fireEvent.change(screen.getByLabelText("Expected close (optional)"), {
      target: { value: "" },
    });
    fireEvent.blur(screen.getByLabelText("Expected close (optional)"));
    const fields: DealFields = JSON.parse(
      screen.getByTestId("fields").textContent ?? "{}",
    );
    expect(fields.expectedCloseDate).toBeNull();
    expect(fields.parties.map((p) => p.role)).toEqual([
      "buyer",
      "portfolio_company",
    ]);
    expect(fields.parties[0]?.companyId).toBe(fields.parties[1]?.companyId);
    await user.click(
      screen.getByRole("button", { name: "Remove company role 1" }),
    );
    expect(
      JSON.parse(screen.getByTestId("fields").textContent ?? "{}").parties,
    ).toHaveLength(1);
  });
  it("explicitly creates a saved directory entry and exposes failures with retry", async () => {
    const user = userEvent.setup();
    api.listCompanies
      .mockRejectedValueOnce(new Error("Unavailable"))
      .mockResolvedValue({ companies: [], hasMore: false });
    api.createCompany.mockResolvedValue(testCompany("New Company"));
    render(<Editor />);
    await user.click(
      await screen.findByRole("button", { name: "Retry companies" }),
    );
    await screen.findByText(/No matching companies/);
    await user.click(screen.getByRole("button", { name: "Create company" }));
    await user.type(screen.getByLabelText("Company name"), "New Company");
    await user.click(screen.getByRole("button", { name: "Save company" }));
    await waitFor(() =>
      expect(api.createCompany).toHaveBeenCalledWith({
        name: "New Company",
        website: null,
        industry: null,
        description: null,
      }),
    );
    expect(
      await screen.findByText(/remains available if you cancel/),
    ).toBeTruthy();
  });

  it.each(["create", "edit"] as const)(
    "sends only company fields when saving a %s form",
    async (mode) => {
      const user = userEvent.setup();
      const company = testCompany("Reusable");
      api.listCompanies.mockResolvedValue({
        companies: [company],
        hasMore: false,
      });
      api.createCompany.mockResolvedValue({
        ...company,
        name: "Updated company",
      });
      api.updateCompany.mockResolvedValue({
        ...company,
        name: "Updated company",
      });
      render(<Editor />);
      await user.click(
        await screen.findByRole("button", {
          name:
            mode === "create"
              ? "Create company"
              : "Edit shared company Reusable",
        }),
      );
      await user.clear(screen.getByLabelText("Company name"));
      await user.type(screen.getByLabelText("Company name"), "Updated company");
      await user.type(
        screen.getByLabelText("Company website (HTTPS)"),
        "https://example.com/",
      );
      await user.type(screen.getByLabelText("Industry"), "Fleet management");
      await user.type(
        screen.getByLabelText("Description"),
        "Company description",
      );
      await user.click(screen.getByRole("button", { name: "Save company" }));

      const payload = {
        name: "Updated company",
        website: "https://example.com/",
        industry: "Fleet management",
        description: "Company description",
      };
      if (mode === "create") {
        expect(api.createCompany).toHaveBeenCalledExactlyOnceWith(payload);
        expect(api.updateCompany).not.toHaveBeenCalled();
      } else {
        expect(api.updateCompany).toHaveBeenCalledExactlyOnceWith(
          company.companyId,
          { ...payload, expectedRevision: company.revision },
        );
        expect(api.createCompany).not.toHaveBeenCalled();
      }
      expect(
        await screen.findByText(
          mode === "create"
            ? /was saved to the directory and selected/
            : /was updated for every linked deal/,
        ),
      ).toBeTruthy();
    },
  );
});

it("retains company edits and the displayed revision after a stale-save conflict", async () => {
  const company = { ...testCompany("Original"), revision: 7 };
  api.listCompanies.mockResolvedValue({ companies: [company], hasMore: false });
  api.updateCompany.mockRejectedValue(Object.assign(new Error("stale"), { status: 409 }));
  const user = userEvent.setup();
  render(<Editor />);
  await user.click(await screen.findByRole("button", { name: "Edit shared company Original" }));
  await user.clear(screen.getByLabelText("Company name"));
  await user.type(screen.getByLabelText("Company name"), "My draft");
  await user.click(screen.getByRole("button", { name: "Save company" }));
  expect((await screen.findByRole("alert")).textContent).toContain("Your entries are kept");
  expect((screen.getByLabelText("Company name") as HTMLInputElement).value).toBe("My draft");
  expect(api.updateCompany).toHaveBeenCalledExactlyOnceWith(company.companyId, expect.objectContaining({ name: "My draft", expectedRevision: 7 }));
  expect(api.createCompany).not.toHaveBeenCalled();
});


describe("company search typing", () => {
  it("debounces requests and retains the input, results, and pagination while typing", async () => {
    vi.useFakeTimers();
    api.listCompanies.mockResolvedValue({ companies: [testCompany("Reusable")], hasMore: true });
    render(<Editor />);
    await act(() => vi.advanceTimersByTimeAsync(0));
    const search = screen.getByLabelText("Search company directory");
    const list = screen.getByRole("list", { name: "Matching companies" });
    const next = screen.getByRole("button", { name: "More companies" });
    search.focus();
    for (const value of ["R", "Re", "Reu", "Reus", "Reusa", "Reusab", "Reusabl"]) {
      fireEvent.change(search, { target: { value } });
      await act(() => vi.advanceTimersByTimeAsync(40));
    }
    fireEvent.change(search, { target: { value: "Reusable" } });
    expect(document.activeElement).toBe(search);
    expect(screen.getByLabelText("Search company directory")).toBe(search);
    expect(screen.getByRole("list", { name: "Matching companies" })).toBe(list);
    expect(next.parentElement?.hidden).toBe(false);
    expect(next.hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Add Reusable" }).hasAttribute("disabled")).toBe(true);
    expect(screen.queryByText("Loading companies…")).toBeNull();
    expect(api.listCompanies).toHaveBeenCalledTimes(1);
    await act(() => vi.advanceTimersByTimeAsync(249));
    expect(api.listCompanies).toHaveBeenCalledTimes(1);
    await act(() => vi.advanceTimersByTimeAsync(1));
    expect(api.listCompanies).toHaveBeenCalledTimes(2);
    expect(api.listCompanies).toHaveBeenLastCalledWith({ query: "Reusable", limit: 30, offset: 0 });
    expect(screen.getByRole("list", { name: "Matching companies" })).toBe(list);
    expect(screen.getByRole("button", { name: "Add Reusable" }).hasAttribute("disabled")).toBe(false);
  });

  it("ignores a superseded response and resets pagination for a new search", async () => {
    vi.useFakeTimers();
    let finishOld: (page: { companies: ReturnType<typeof testCompany>[]; hasMore: boolean }) => void = () => { throw new Error("Request not started"); };
    api.listCompanies
      .mockResolvedValueOnce({ companies: [testCompany("First")], hasMore: true })
      .mockImplementationOnce(() => new Promise((resolve) => { finishOld = resolve; }))
      .mockResolvedValue({ companies: [testCompany("Latest")], hasMore: false });
    render(<Editor />);
    await act(() => vi.advanceTimersByTimeAsync(0));
    fireEvent.click(screen.getByRole("button", { name: "More companies" }));
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(api.listCompanies).toHaveBeenLastCalledWith({ query: "", limit: 30, offset: 30 });
    fireEvent.change(screen.getByLabelText("Search company directory"), { target: { value: "Latest" } });
    await act(() => vi.advanceTimersByTimeAsync(250));
    expect(api.listCompanies).toHaveBeenLastCalledWith({ query: "Latest", limit: 30, offset: 0 });
    expect(screen.getByRole("button", { name: "Add Latest" })).toBeTruthy();
    await act(async () => { finishOld({ companies: [testCompany("Obsolete")], hasMore: true }); });
    expect(screen.queryByRole("button", { name: "Add Obsolete" })).toBeNull();
    expect(screen.getByRole("button", { name: "Add Latest" })).toBeTruthy();
  });

  it("clears a scheduled search when the editor unmounts", async () => {
    vi.useFakeTimers();
    api.listCompanies.mockResolvedValue({ companies: [], hasMore: false });
    const { unmount } = render(<Editor />);
    await act(() => vi.advanceTimersByTimeAsync(0));
    fireEvent.change(screen.getByLabelText("Search company directory"), { target: { value: "Unsubmitted" } });
    unmount();
    await act(() => vi.advanceTimersByTimeAsync(250));
    expect(api.listCompanies).toHaveBeenCalledTimes(1);
  });
});

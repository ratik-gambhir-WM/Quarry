import { useEffect, useRef, useState } from "react";
import { runtime } from "@quarry/runtime";
import {
  codeLabel, dealStatusOptions, diligenceScopeOptions, engagementTypeOptions,
  partyRoleOptions,
  type Company, type CompanyInput, type CompanyPage, type DealFields,
} from "../../../data/dealModel";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { Field, FieldLabel } from "../../ui/field";
import { CodeSelect, TransactionTypePicker } from "./DealTypePicker";

type DealFieldsEditorProps = {
  fields: DealFields;
  onChange: (fields: DealFields) => void;
  userEmail: string;
  initialCompanies?: readonly Company[];
  onCompanySaved?: (company: Company) => void;
  onCompanyFormChange?: (open: boolean) => void;
};

export function DealFieldsEditor(props: DealFieldsEditorProps) {
  return <div className="grid gap-5">
    <DealDetailsEditor fields={props.fields} onChange={props.onChange} />
    <DealCompanyRolesEditor {...props} />
  </div>;
}

export function DealDetailsEditor({ fields, onChange }: Pick<DealFieldsEditorProps, "fields" | "onChange">) {
  const update = <K extends keyof DealFields>(key: K, value: DealFields[K]) => onChange({ ...fields, [key]: value });
  return <div className="grid gap-5">
    <div className="grid gap-4 sm:grid-cols-2">
      <Field><FieldLabel htmlFor="deal-name">Deal name</FieldLabel><Input id="deal-name" maxLength={300} required value={fields.dealName} onChange={(e) => update("dealName", e.currentTarget.value)} /></Field>
      <Field><FieldLabel htmlFor="deal-code">Deal code (optional)</FieldLabel><Input id="deal-code" maxLength={100} value={fields.dealCode ?? ""} onChange={(e) => update("dealCode", e.currentTarget.value || null)} /></Field>
      <CodeSelect label="Engagement type" options={engagementTypeOptions} value={fields.engagementType} onChange={(value) => update("engagementType", value)} />
      <TransactionTypePicker value={fields.transactionType} onChange={(value) => update("transactionType", value)} />
      <CodeSelect label="Status" options={dealStatusOptions} value={fields.status} onChange={(value) => update("status", value)} />
      {([
        ["startDate", "Start date"], ["expectedCloseDate", "Expected close"],
        ["actualCloseDate", "Actual close"], ["engagementCompletedDate", "Engagement completed"],
      ] as const).map(([key, label]) => <Field key={key}><FieldLabel htmlFor={`deal-${key}`}>{label} (optional)</FieldLabel><Input id={`deal-${key}`} type="date" value={fields[key] ?? ""} onChange={(e) => update(key, e.currentTarget.value || null)} /></Field>)}
    </div>
    <fieldset className="grid gap-2"><legend className="mb-2 text-sm font-semibold">Diligence scope (optional)</legend>
      <div className="grid gap-2 sm:grid-cols-2">{diligenceScopeOptions.map((scope) => <label className="flex items-center gap-2 text-sm" key={scope}>
        <input type="checkbox" checked={fields.scopes.includes(scope)} onChange={(e) => update("scopes", e.currentTarget.checked ? [...fields.scopes, scope] : fields.scopes.filter((s) => s !== scope))} />{codeLabel(scope)}
      </label>)}</div>
    </fieldset>
  </div>;
}

export function DealCompanyRolesEditor({ fields, onChange, userEmail, initialCompanies = [], onCompanySaved, onCompanyFormChange }: DealFieldsEditorProps) {
  const [companies, setCompanies] = useState(() => new Map(initialCompanies.map((company) => [company.companyId, company])));
  const [query, setQuery] = useState("");
  const [attempt, setAttempt] = useState(0);
  const [companyForm, setCompanyForm] = useState<Company | "new" | null>(null);
  const [notice, setNotice] = useState("");
  const update = <K extends keyof DealFields>(key: K, value: DealFields[K]) => onChange({ ...fields, [key]: value });
  function addCompany(company: Company) {
    setCompanies((current) => new Map(current).set(company.companyId, company));
    update("parties", [...fields.parties, { companyId: company.companyId, role: "target", notes: null }]);
  }
  function editCompany(company: Company | "new" | null) {
    setCompanyForm(company);
    onCompanyFormChange?.(company !== null);
  }
  return (
    <fieldset className="grid gap-3"><legend className="mb-2 text-sm font-semibold">Companies and roles (optional for pipeline deals)</legend>
      <p className="text-xs text-muted-foreground">Select directory records explicitly. One company can have several roles. Active and completed acquisitions require exactly one target.</p>
      {fields.parties.map((party, index) => <div className="grid gap-3 rounded-lg border border-border p-3" key={index}>
        <span className="text-sm font-semibold">{companies.get(party.companyId)?.name ?? "Company unavailable"}</span>
        <CodeSelect label={`Role for company ${index + 1}`} options={partyRoleOptions} value={party.role} onChange={(role) => update("parties", fields.parties.map((p, i) => i === index ? { ...p, role } : p))} />
        <label className="grid gap-1 text-xs">Deal-specific notes for company {index + 1}<Input maxLength={2000} value={party.notes ?? ""} onChange={(e) => update("parties", fields.parties.map((p, i) => i === index ? { ...p, notes: e.currentTarget.value || null } : p))} /></label>
        <Button type="button" variant="outline" onClick={() => update("parties", fields.parties.filter((_, i) => i !== index))}>Remove company role {index + 1}</Button>
      </div>)}
      <label className="grid gap-2 text-sm">Search company directory<Input maxLength={200} value={query} onChange={(e) => setQuery(e.currentTarget.value)} /></label>
      <CompanyResults key={`${userEmail}:${query}:${attempt}`} userEmail={userEmail} query={query} onAdd={addCompany} onEdit={editCompany} onRetry={() => setAttempt((n) => n + 1)} disabled={fields.parties.length >= 100} />
      <Button type="button" variant="outline" onClick={() => editCompany("new")}>Create company</Button>
      {companyForm ? <CompanyForm key={companyForm === "new" ? "new" : companyForm.companyId} company={companyForm === "new" ? undefined : companyForm} userEmail={userEmail} onCancel={() => editCompany(null)} onSaved={(company, created) => {
        setCompanies((current) => new Map(current).set(company.companyId, company));
        if (created) addCompany(company);
        onCompanySaved?.(company);
        setNotice(created ? `${company.name} was saved to the directory and selected. It remains available if you cancel this deal.` : `${company.name} was updated for every linked deal.`);
        editCompany(null); setAttempt((n) => n + 1);
      }} /> : null}
      {notice ? <p className="text-sm text-muted-foreground" role="status">{notice}</p> : null}
    </fieldset>
  );
}

function CompanyResults({ userEmail, query, onAdd, onEdit, onRetry, disabled }: {
  userEmail: string; query: string; onAdd: (company: Company) => void; onEdit: (company: Company) => void; onRetry: () => void; disabled: boolean;
}) {
  const [result, setResult] = useState<{ page?: CompanyPage; error?: string }>({});
  const [offset, setOffset] = useState(0);
  const requestRef = useRef(0);
  useEffect(() => {
    let active = true;
    const request = ++requestRef.current;
    void runtime.api.listCompanies(userEmail, { query, limit: 30, offset }).then((page) => {
      if (active && request === requestRef.current) setResult({ page });
    }).catch((error: unknown) => {
      if (active && request === requestRef.current) setResult({ error: error instanceof Error ? error.message : String(error) });
    });
    return () => { active = false; };
  }, [offset, query, userEmail]);
  function nextPage(next: number) { setResult({}); setOffset(next); }
  if (result.error) return <div role="alert">Couldn’t load companies. {result.error} <Button type="button" variant="outline" onClick={onRetry}>Retry companies</Button></div>;
  if (!result.page) return <p role="status" className="text-sm">Loading companies…</p>;
  return <div className="grid gap-2">
    {result.page.companies.length === 0 ? <p className="text-sm text-muted-foreground">No matching companies. Create one to add it to the directory.</p> : null}
    <ul aria-label="Matching companies" className="max-h-48 overflow-y-auto rounded-md border border-border">
      {result.page.companies.map((company) => <li className="flex items-center gap-2 border-b border-border p-2 last:border-0" key={company.companyId}>
        <span className="min-w-0 flex-1 text-sm">{company.name}{company.website ? <small className="block truncate text-muted-foreground">{company.website}</small> : null}</span>
        <Button disabled={disabled} type="button" variant="outline" onClick={() => onAdd(company)} aria-label={`Add ${company.name}`}>Add</Button>
        <Button type="button" variant="ghost" onClick={() => onEdit(company)} aria-label={`Edit shared company ${company.name}`}>Edit company</Button>
      </li>)}
    </ul>
    <div className="flex gap-2"><Button disabled={!offset} type="button" variant="ghost" onClick={() => nextPage(Math.max(0, offset - 30))}>Previous companies</Button><Button disabled={!result.page.hasMore} type="button" variant="ghost" onClick={() => nextPage(offset + 30)}>More companies</Button></div>
  </div>;
}

function CompanyForm({ company, userEmail, onSaved, onCancel }: {
  company?: Company; userEmail: string; onSaved: (company: Company, created: boolean) => void; onCancel: () => void;
}) {
  const [input, setInput] = useState<CompanyInput>(() => ({ userEmail, name: company?.name ?? "", website: company?.website ?? null, industry: company?.industry ?? null, description: company?.description ?? null }));
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  async function save() {
    if (!input.name.trim()) { setError("Company name is required."); return; }
    setPending(true); setError("");
    try { const saved = company ? await runtime.api.updateCompany(company.companyId, input) : await runtime.api.createCompany(input); onSaved(saved, !company); }
    catch (error) { setError(error instanceof Error ? error.message : String(error)); }
    finally { setPending(false); }
  }
  return <section onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); if (!pending) void save(); } }} className="grid gap-3 rounded-lg border border-border p-4" aria-label={company ? "Edit shared company" : "Create company"}>
    <h3 className="text-sm font-semibold">{company ? "Edit shared company" : "Create company"}</h3>
    {company ? <p className="text-xs text-muted-foreground">These details apply to every deal using this company. Role notes belong to this deal only.</p> : null}
    {([ ["name", "Company name"], ["website", "Company website (HTTPS)"], ["industry", "Industry"], ["description", "Description"] ] as const).map(([key, label]) => <label className="grid gap-1 text-sm" key={key}>{label}<Input disabled={pending} maxLength={key === "description" ? 4000 : key === "website" ? 2048 : 300} value={input[key] ?? ""} onChange={(e) => setInput({ ...input, [key]: e.currentTarget.value || (key === "name" ? "" : null) })} /></label>)}
    {error ? <p role="alert" className="text-sm text-error">{error}</p> : null}
    <div className="flex gap-2"><Button disabled={pending} type="button" onClick={() => void save()}>{pending ? "Saving company…" : "Save company"}</Button><Button disabled={pending} type="button" variant="ghost" onClick={onCancel}>Cancel company</Button></div>
  </section>;
}

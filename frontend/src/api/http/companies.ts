import type {
  Company,
  CompanyInput,
  CompanyPage,
  CompanySearch,
} from "../../data/dealModel";
import type { QuarryApi } from "../../contracts/quarryApi";

import { get, post } from "./transport";

function createCompany(input: CompanyInput) {
  return post<Company, CompanyInput>("/api/v1/companies", input);
}

function updateCompany(id: string, input: CompanyInput) {
  return post<Company, CompanyInput>(
    `/api/v1/companies/${encodeURIComponent(id)}/update`,
    input,
  );
}

function listCompanies(options: CompanySearch = {}) {
  const params = new URLSearchParams();
  if (options.query !== undefined) params.set("query", options.query);
  if (options.limit !== undefined) params.set("limit", String(options.limit));
  if (options.offset !== undefined)
    params.set("offset", String(options.offset));
  return get<CompanyPage>(`/api/v1/companies?${params}`);
}

export const httpCompaniesApi: Pick<
  QuarryApi,
  | "createCompany"
  | "updateCompany"
  | "listCompanies"
> = {
  createCompany,
  updateCompany,
  listCompanies,
};

export const engagementTypeOptions = ["buy_side", "sell_side", "other"] as const;
export type EngagementType = (typeof engagementTypeOptions)[number];
export const transactionTypeOptions = ["platform_acquisition", "add_on_acquisition", "strategic_acquisition", "carve_out", "merger", "divestiture", "recapitalization", "growth_equity", "other", "unknown"] as const;
export type TransactionType = (typeof transactionTypeOptions)[number];
export const dealStatusOptions = ["pipeline", "prepare", "review_plan_discovery", "interviews_site_visit", "debrief_flash_report", "follow_ups_analysis", "qa_readout_closeout", "closed", "pencils_down"] as const;
export type DealStatus = (typeof dealStatusOptions)[number];
export const dealStatusLabels = {
  pipeline: "Pipeline",
  prepare: "Prepare",
  review_plan_discovery: "Review & plan discovery",
  interviews_site_visit: "Interviews & site visit (SV)",
  debrief_flash_report: "Debrief & flash report",
  follow_ups_analysis: "Follow-ups & analysis",
  qa_readout_closeout: "QA, readout & closeout",
  closed: "Closed",
  pencils_down: "Pencils Down",
} satisfies Record<DealStatus, string>;
export function isTerminalDealStatus(status: DealStatus): boolean {
  return status === "closed" || status === "pencils_down";
}

export const partyRoleOptions = ["target", "buyer", "seller", "pe_sponsor", "portfolio_company", "parent_company", "client", "other"] as const;
export type PartyRole = (typeof partyRoleOptions)[number];
export const diligenceScopeOptions = ["product_architecture", "corporate_it", "cybersecurity", "application_security", "sdlc", "it_organization", "it_financials"] as const;
export type DiligenceScope = (typeof diligenceScopeOptions)[number];

export type Company = {
  companyId: string; userId: number; name: string; website: string | null;
  industry: string | null; description: string | null; createdAt: string; updatedAt: string;
};
export type CompanyInput = Pick<Company, "name" | "website" | "industry" | "description">;
export type CompanyPage = { companies: Company[]; hasMore: boolean };
export type CompanySearch = { query?: string; limit?: number; offset?: number };
export type PartyInput = { companyId: string; role: PartyRole; notes: string | null };
export type DealParty = PartyInput & { dealPartyId: string; company: Company };
export type DealFields = {
  dealName: string; dealCode: string | null; engagementType: EngagementType;
  transactionType: TransactionType; status: DealStatus; startDate: string | null;
  expectedCloseDate: string | null; actualCloseDate: string | null;
  engagementCompletedDate: string | null; parties: PartyInput[]; scopes: DiligenceScope[];
};
export function codeLabel(code: string): string {
  const labels: Record<string, string> = { buy_side: "Buy-side", sell_side: "Sell-side", pe_sponsor: "PE sponsor", sdlc: "SDLC", corporate_it: "Corporate IT", it_organization: "IT organization", it_financials: "IT financials", add_on_acquisition: "Add-on acquisition" };
  return (dealStatusLabels as Record<string, string>)[code] ?? labels[code] ?? code.replace(/_/g, " ").replace(/^./, (c) => c.toUpperCase());
}
export function partyNames(parties: readonly DealParty[], role: PartyRole) {
  return parties.filter((p) => p.role === role).map((p) => p.company.name).join(", ") || undefined;
}

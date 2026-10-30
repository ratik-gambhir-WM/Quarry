import type {
  SavedDeal,
  SavedDealMetadata,
  SaveDealInput,
} from "../../data/dealExtraction";
import type { Company, DealParty, PartyRole } from "../../data/dealModel";
export function testCompany(
  name: string,
  companyId = "00000000-0000-4000-8000-000000000001",
): Company {
  return {
    companyId,
    userId: 1,
    name,
    website: null,
    industry: null,
    description: null,
    createdAt: "2026-10-01T00:00:00Z",
    updatedAt: "2026-10-01T00:00:00Z",
  };
}
export function testParty(
  name: string,
  role: PartyRole,
  companyId?: string,
): DealParty {
  const company = testCompany(name, companyId);
  return {
    companyId: company.companyId,
    company,
    dealPartyId: `${role}-${name}`,
    role,
    notes: null,
  };
}
export function testDeal(overrides: Partial<SavedDeal> = {}): SavedDeal {
  return {
    dealId: "DEAL-000184",
    userId: 1,
    dealName: "Project Alpha",
    dealCode: null,
    engagementType: "buy_side",
    transactionType: "unknown",
    status: "pipeline",
    startDate: null,
    expectedCloseDate: null,
    actualCloseDate: null,
    engagementCompletedDate: null,
    archivedAt: null,
    createdAt: "2026-10-01T00:00:00Z",
    updatedAt: "2026-10-01T00:00:00Z",
    parties: [],
    scopes: [],
    ...overrides,
  };
}
export function testMetadata(dealId = "DEAL-000184"): SavedDealMetadata {
  return {
    dealId,
    factSheetLink: null,
    keyQuestionsJson: "[]",
    localPath: null,
    rlLink: null,
    sharepointLink: null,
    sowLink: null,
  };
}
export function testDealInput(): SaveDealInput {
  const deal = testDeal();
  return {
    dealId: deal.dealId,
    dealName: deal.dealName,
    dealCode: deal.dealCode,
    engagementType: deal.engagementType,
    transactionType: deal.transactionType,
    status: deal.status,
    startDate: deal.startDate,
    expectedCloseDate: deal.expectedCloseDate,
    actualCloseDate: deal.actualCloseDate,
    engagementCompletedDate: deal.engagementCompletedDate,
    parties: [],
    scopes: [],
    localPath: null,
    sharepointLink: null,
  };
}

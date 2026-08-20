import { DataRoomChip } from "../../../data/dataRoom";

export type SearchMode = "keyword" | "semantic";

export type RepositoryResult = {
  category: string;
  date: string;
  excerpt: string;
  fileName: string;
  id: string;
  location: string;
  semanticScore: number;
  keywordScore: number;
};

type RepositorySearchFilters = {
  category: string;
  date: string;
  mode: SearchMode;
  query: string;
  results: RepositoryResult[];
};

export const ALL_CATEGORIES = "All categories";
export const ANY_TIME = "Any time";

const featuredResults: RepositoryResult[] = [
  {
    category: "Risk & Insurance",
    date: "Oct 18, 2023",
    excerpt:
      "The policy specifically excludes environmental hazards where secondary liability coverage would otherwise apply.",
    fileName: "Global_Risk_Assurance_2023.pdf",
    id: "risk-assurance",
    keywordScore: 96,
    location: "Page 42",
    semanticScore: 98,
  },
  {
    category: "Legal",
    date: "Mar 7, 2024",
    excerpt:
      "The transaction remains subject to comprehensive liability review regarding environmental site assessment and hazard remediation.",
    fileName: "Project_Alpha_Master_Agreement.docx",
    id: "master-agreement",
    keywordScore: 82,
    location: "Section 8.4",
    semanticScore: 84,
  },
  {
    category: "Audit",
    date: "Dec 12, 2022",
    excerpt:
      "Third-party hazard coverage requirements for environmental impacts must be validated before final approval.",
    fileName: "FY22_Audit_Appendix_B.pdf",
    id: "audit-appendix",
    keywordScore: 74,
    location: "Page 17",
    semanticScore: 61,
  },
];

const chipFileNames = [
  "Project_Alpha_Q3_Financial_Report.pdf",
  "APAC_IP_Litigation_Register.docx",
  "Competitive_Matrix_Q4.xlsx",
  "Workforce_Metrics_Q4.xlsx",
  "GDPR_Compliance_Audit.pdf",
];

export function buildRepositoryResults(chips: DataRoomChip[]) {
  return [...featuredResults, ...chips.map((chip, index) => resultFromChip(chip, index))];
}

export function getRepositoryCategories(results: RepositoryResult[]) {
  return [ALL_CATEGORIES, ...Array.from(new Set(results.map((result) => result.category))).sort()];
}

export function formatRepositoryFileName(fileName: string) {
  return fileName.replace(/_/g, " ");
}

export function filterRepositoryResults({
  category,
  date,
  mode,
  query,
  results,
}: RepositorySearchFilters) {
  const terms = getSearchQueryTerms(query);

  return results.filter((result) => {
    if (category !== ALL_CATEGORIES && result.category !== category) {
      return false;
    }

    if (date !== ANY_TIME && !result.date.endsWith(date)) {
      return false;
    }

    if (terms.length === 0) {
      return true;
    }

    const searchable = `${result.fileName} ${result.category} ${result.location} ${result.excerpt}`.toLowerCase();
    return mode === "keyword"
      ? terms.every((term) => searchable.includes(term))
      : terms.some((term) => searchable.includes(term));
  });
}

export function getSearchQueryTerms(query: string) {
  return query
    .toLowerCase()
    .split(/\s+/)
    .map((term) => term.replace(/[^a-z0-9_-]/g, ""))
    .filter((term) => term.length > 2);
}

function resultFromChip(chip: DataRoomChip, index: number): RepositoryResult {
  const excerpt =
    chip.type === "text"
      ? chip.body
      : chip.type === "chart"
        ? `${chip.footer}. Supporting market share analysis is available in the source workbook.`
        : chip.rows.map((row) => `${row.label}: ${row.value}`).join(". ");

  return {
    category: chip.category,
    date: "Jan 15, 2024",
    excerpt,
    fileName: chipFileNames[index] ?? `Project_Alpha_Insight_${index + 1}.pdf`,
    id: `repository-${chip.id}`,
    keywordScore: Math.max(58, 91 - index * 6),
    location: chip.type === "chart" ? "Chart 4" : chip.type === "metrics" ? "Summary" : `Page ${12 + index * 3}`,
    semanticScore: Math.max(64, 94 - index * 5),
  };
}

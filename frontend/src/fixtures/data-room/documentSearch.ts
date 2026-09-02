import type { DocumentSearchResult } from "../../components/data-room/document-search/documentSearchModel";

const relatedDocumentResults: DocumentSearchResult[] = [
  {
    excerpt:
      "The policy excludes environmental hazards where secondary liability coverage would otherwise apply.",
    fileName: "Global_Risk_Assurance_2023.pdf",
    id: "risk-assurance",
    location: "Page 42",
  },
  {
    excerpt:
      "The transaction remains subject to liability review regarding environmental site assessment and remediation.",
    fileName: "Project_Alpha_Master_Agreement.docx",
    id: "master-agreement",
    location: "Section 8.4",
  },
];

export function buildDocumentSearchFixtureResults(
  currentFileName: string,
): DocumentSearchResult[] {
  return [
    {
      excerpt:
        "Review the document overview, principal terms, liability coverage, and environmental risk references.",
      fileName: currentFileName,
      id: "current-document-overview",
      location: "Page 1",
      target: { kind: "pdf-page", page: 1 },
    },
    {
      excerpt:
        "Continue to the supporting analysis and detailed diligence observations in the current document.",
      fileName: currentFileName,
      id: "current-document-details",
      location: "Page 2",
      target: { kind: "pdf-page", page: 2 },
    },
    ...relatedDocumentResults,
  ];
}

import type { FileReviewSummary } from "../../data/fileReview";

// Illustrative review findings only. They are intentionally separate from
// file metadata returned by the product API and local desktop runtime.
export const illustrativeFileReviewSummaries: FileReviewSummary[] = [
  {
    impact: "Supports the base-case revenue thesis",
    impactLevel: "high",
    keyFinding: "Recurring revenue growth remains resilient",
    opportunities: "Upsell motion appears underdeveloped",
    opportunityLevel: "medium",
    risks: "Customer concentration needs validation",
    riskLevel: "high",
    status: "Needs attention",
  },
  {
    impact: "Provides supporting operating evidence",
    impactLevel: "medium",
    keyFinding: "Operating cadence is documented consistently",
    opportunities: "Process automation could improve margins",
    opportunityLevel: "high",
    risks: "No material issue identified in initial review",
    riskLevel: "low",
    status: "Reviewed",
  },
  {
    impact: "May affect the integration workplan",
    impactLevel: "medium",
    keyFinding: "Several dependencies require owner confirmation",
    opportunities: "Consolidation could reduce tooling spend",
    opportunityLevel: "medium",
    risks: "Implementation timing remains uncertain",
    riskLevel: "medium",
    status: "Needs attention",
  },
  {
    impact: "Analysis pending",
    impactLevel: "none",
    keyFinding: "Review has not started",
    opportunities: "Analysis pending",
    opportunityLevel: "none",
    risks: "Analysis pending",
    riskLevel: "none",
    status: "Pending",
  },
];

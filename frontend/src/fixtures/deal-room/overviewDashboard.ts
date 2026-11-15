// Illustrative overview data until assessment metrics and planning have backing contracts.
export const overviewDashboardPreview = {
  documents: 48,
  findings: 12,
  openItems: 5,
  progress: 62,
  workstreams: [
    { name: "Product architecture", stage: "Analysis" },
    { name: "Corporate IT", stage: "Discovery" },
    { name: "Cybersecurity", stage: "QA" },
    { name: "SDLC", stage: "Not Started" },
  ],
  actions: [
    { title: "Confirm application inventory", detail: "Product architecture", priority: "High" },
    { title: "Review access management policies", detail: "Cybersecurity", priority: "Medium" },
    { title: "Validate infrastructure cost baseline", detail: "Corporate IT", priority: "Medium" },
  ],
  milestones: [
    { title: "Management interviews", date: "Week 1", detail: "Discovery" },
    { title: "Preliminary findings review", date: "Week 2", detail: "Assessment" },
    { title: "Final readout", date: "Week 3", detail: "Deliverable" },
  ],
} as const;

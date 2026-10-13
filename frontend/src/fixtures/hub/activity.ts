export const hubActivityTasks = [
  {
    checked: true,
    creator: "JD, AT",
    label: "Finalize Q3 Financial Extract for Project Alpha",
    practiceArea: "Project Alpha",
    tag: { tone: "error" as const, value: "High Priority" },
  },
  {
    checked: false,
    creator: "JD, AT",
    label: "Review legal disclosures for Logistics Merger",
    practiceArea: "Logistics Merger",
    tag: { tone: "success" as const, value: "Due Today" },
  },
  {
    checked: false,
    creator: "JD, AT",
    label: "Approve analyst transcript summary: CEO Site Visit",
    practiceArea: "Project Alpha",
    tag: { tone: "muted" as const, value: "Alpha" },
  },
  {
    checked: false,
    creator: "JD, AT",
    label: "Initialize Data Room for Project Gamma",
    practiceArea: "Project Gamma",
    tag: { tone: "icon" as const, value: "more" as const },
  },
] as const;

export const hubRecentFiles = [
  {
    creator: "Vault",
    deal: "Project Alpha",
    icon: "pdf" as const,
    time: "2m ago",
    title: "Q3 Financial Report.pdf",
    tone: "error" as const,
  },
  {
    creator: "Vault",
    deal: "Project Beta",
    icon: "doc" as const,
    time: "1h ago",
    title: "Meeting Minutes - Legal Review.doc",
    tone: "accent" as const,
  },
  {
    creator: "Vault",
    deal: "Logistics Merger",
    icon: "sheet" as const,
    time: "3h ago",
    title: "Logistics_Due_Diligence.xlsx",
    tone: "primary" as const,
  },
  {
    creator: "Vault",
    deal: "Project Alpha",
    icon: "pdf" as const,
    time: "Yesterday",
    title: "Environmental Impact Study.pdf",
    tone: "error" as const,
  },
] as const;

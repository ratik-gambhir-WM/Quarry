export const hubActivityTasks = [
  {
    checked: true,
    label: "Finalize Q3 Financial Extract for Project Alpha",
    tag: { tone: "error" as const, value: "High Priority" },
  },
  {
    checked: false,
    label: "Review legal disclosures for Logistics Merger",
    tag: { tone: "success" as const, value: "Due Today" },
  },
  {
    checked: false,
    label: "Approve analyst transcript summary: CEO Site Visit",
    tag: { tone: "muted" as const, value: "Alpha" },
  },
  {
    checked: false,
    label: "Initialize Data Room for Project Gamma",
    tag: { tone: "icon" as const, value: "more" as const },
  },
] as const;

export const hubRecentFiles = [
  {
    deal: "Project Alpha",
    icon: "pdf" as const,
    time: "2m ago",
    title: "Q3 Financial Report.pdf",
    tone: "error" as const,
  },
  {
    deal: "Project Beta",
    icon: "doc" as const,
    time: "1h ago",
    title: "Meeting Minutes - Legal Review.doc",
    tone: "accent" as const,
  },
  {
    deal: "Logistics Merger",
    icon: "sheet" as const,
    time: "3h ago",
    title: "Logistics_Due_Diligence.xlsx",
    tone: "primary" as const,
  },
  {
    deal: "Project Alpha",
    icon: "pdf" as const,
    time: "Yesterday",
    title: "Environmental Impact Study.pdf",
    tone: "error" as const,
  },
] as const;

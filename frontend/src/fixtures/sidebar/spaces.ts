import type { SidebarSpace } from "../../data/sidebar";

export const workspaceSidebarSpaces: SidebarSpace[] = [
  {
    description: "Workstreams and findings",
    icon: "graph",
    id: "diligence",
    label: "Diligence",
    sections: [
      {
        title: "Workstreams",
        items: [
          { icon: "dashboard", id: "commercial-review", label: "Commercial Review" },
          { icon: "timeline", id: "financial-review", label: "Financial Review" },
          { icon: "grid", id: "operational-review", label: "Operational Review" },
          { icon: "shield", id: "legal-compliance", label: "Legal & Compliance" },
        ],
      },
      {
        title: "Workspace",
        items: [
          { icon: "folderOpen", id: "data-requests", label: "Data Requests" },
          { icon: "sparkles", id: "findings", label: "Findings" },
          { icon: "listAlt", id: "deliverables", label: "Deliverables" },
        ],
      },
    ],
  },
  {
    description: "Research and saved evidence",
    icon: "search",
    id: "research",
    label: "Research",
    sections: [
      {
        title: "Discover",
        items: [
          { icon: "search", id: "research-library", label: "Research Library" },
          { icon: "person", id: "expert-calls", label: "Expert Calls" },
          { icon: "timeline", id: "market-signals", label: "Market Signals" },
        ],
      },
      {
        title: "Your Library",
        items: [
          { icon: "bookmark", id: "saved-evidence", label: "Saved Evidence" },
          { icon: "doc", id: "recent-work", label: "Recent Work" },
        ],
      },
    ],
  },
  {
    description: "Team workflows and tools",
    icon: "grid",
    id: "operations",
    label: "Operations",
    sections: [
      {
        title: "Team",
        items: [
          { icon: "checkCircle", id: "assignments", label: "Assignments" },
          { icon: "timeline", id: "calendar", label: "Calendar" },
          { icon: "notification", id: "activity", label: "Activity" },
        ],
      },
      {
        title: "Manage",
        items: [
          { icon: "settings", id: "integrations", label: "Integrations" },
          { icon: "dataset", id: "templates", label: "Templates" },
        ],
      },
    ],
  },
];

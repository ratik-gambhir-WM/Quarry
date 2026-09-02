export type SuggestedContentTab = "Recent" | "Analyzed" | "Files to review";

type SuggestedContentItem = {
  icon: "doc" | "folderOpen" | "pdf" | "sheet";
  label: string;
  tone?: "error" | "primary";
};

type SuggestedContentGroup = {
  items: SuggestedContentItem[];
  label: string;
};

export const suggestedContentByTab: Record<SuggestedContentTab, SuggestedContentGroup[]> = {
  Recent: [
    {
      label: "Today",
      items: [
        { icon: "doc", label: "Project Katalyst Carveout to Integration", tone: "error" },
        { icon: "doc", label: "AIDI C-Templates", tone: "error" },
      ],
    },
    {
      label: "Yesterday",
      items: [
        { icon: "folderOpen", label: "Clearlake Capital Account-BetaNXT SSA - Documents" },
        { icon: "folderOpen", label: "00. BD" },
        { icon: "doc", label: "WM - Agile Frameworks - Technology Summary Report", tone: "error" },
        { icon: "doc", label: "WM - Agile Frameworks - Discovery Debrief", tone: "error" },
      ],
    },
  ],
  Analyzed: [
    {
      label: "Today",
      items: [
        { icon: "pdf", label: "Q3 Financial Report.pdf", tone: "error" },
        { icon: "doc", label: "Meeting Minutes - Legal Review.doc", tone: "primary" },
      ],
    },
    {
      label: "Yesterday",
      items: [
        { icon: "sheet", label: "Logistics_Due_Diligence.xlsx", tone: "primary" },
        { icon: "pdf", label: "Environmental Impact Study.pdf", tone: "error" },
      ],
    },
  ],
  "Files to review": [
    {
      label: "Today",
      items: [
        { icon: "doc", label: "Project Alpha integration workplan", tone: "primary" },
        { icon: "folderOpen", label: "Logistics Merger - Legal Review" },
      ],
    },
    {
      label: "Yesterday",
      items: [{ icon: "doc", label: "CEO Site Visit transcript summary", tone: "primary" }],
    },
  ],
};

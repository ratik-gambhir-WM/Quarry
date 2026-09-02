export type WorkspaceLocationState = {
  accountLookupComplete?: boolean;
  accountLookupError?: string;
  accountUser?: WorkspaceAccountUser | null;
  email?: string;
};

export type WorkspaceAccountUser = {
  apiKey: string;
  createdAt: string;
  email: string;
  firstName: string;
  id: number;
  lastName: string;
  role: string;
  updatedAt: string;
};

export type WorkspaceSidebarIcon = "personSearch" | "terminal" | "timeline" | "folderOpen" | "sparkles";

export type WorkspaceSidebarTool = {
  href?: string;
  icon: WorkspaceSidebarIcon;
  name: string;
};

export type DealTimelineTone = "accent" | "error" | "muted" | "primary";

export type DealTimelineItem = {
  category: string;
  date: string;
  detail: string;
  id: string;
  time?: string;
  timestamp: string;
  title: string;
  tone: DealTimelineTone;
};

export type DealMetric = {
  label: string;
  tone?: "default" | "error";
  value: string;
};

export type DealTask = {
  done?: boolean;
  id: string;
  label: string;
  priority?: boolean;
};

export type DealRoomData = {
  id: string;
  keyQuestions: string[];
  metrics: DealMetric[];
  name: string;
  overviewSubtitle: string;
  phaseLabel: string;
  pendingTasks: DealTask[];
  sectorLabel: string;
  stageLabel: string;
  summary: string;
  timeline: DealTimelineItem[];
};

export type DealPortfolioMetadata = {
  closeDate?: string;
  dealSponsor?: string;
  primaryBuyer?: string;
  startDate?: string;
  status: string;
  targetCompany?: string;
  transactionType?: string;
};

export type WorkspaceDeal = {
  colorClassName: string;
  complete?: boolean;
  portfolio: DealPortfolioMetadata;
  room: DealRoomData;
};

export type WorkspaceInsight = {
  category: string;
  deal: string;
  fileIcon: "doc" | "image" | "pdf" | "sheet";
  fileName: string;
  image?: boolean;
  quote: string;
  toneClassName: string;
  toneTextClassName: string;
};

export function getDealRoomPath(dealId: string) {
  return `/hub/deals/${dealId}`;
}

export function getDataRoomPath(dealId: string) {
  return `/hub/deals/${dealId}/data-room`;
}

export function getTeamLabel(email?: string) {
  const teamName = email?.split("@")[0]?.replace(/[._-]/g, " ") ?? "Analyst Team";

  return teamName.replace(/\b\w/g, (character) => character.toUpperCase());
}

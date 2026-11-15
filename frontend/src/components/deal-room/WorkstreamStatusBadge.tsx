import { Badge } from "../ui/badge";

const stageStyles = {
  "Not Started": "border-outline-variant bg-surface-container-high text-on-surface-variant",
  Discovery: "border-icon-navigation/20 bg-icon-navigation/10 text-[color-mix(in_srgb,var(--theme-icon-navigation)_80%,var(--theme-text-strong))]",
  Analysis: "border-icon-assistant/20 bg-icon-assistant/10 text-[color-mix(in_srgb,var(--theme-icon-assistant)_80%,var(--theme-text-strong))]",
  QA: "border-icon-files/20 bg-icon-files/10 text-[color-mix(in_srgb,var(--theme-icon-files)_80%,var(--theme-text-strong))]",
  Complete: "border-success/20 bg-success/10 text-[color-mix(in_srgb,var(--theme-success)_80%,var(--theme-text-strong))]",
} as const;

export function WorkstreamStatusBadge({ stage }: { stage: keyof typeof stageStyles }) {
  return (
    <Badge variant="outline" className={`h-6 w-28 justify-center px-2.5 ${stageStyles[stage]}`}>
      {stage}
    </Badge>
  );
}

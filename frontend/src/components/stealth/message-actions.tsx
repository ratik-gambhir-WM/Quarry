// Adapted from https://stealth.pm/ui/message-actions (MIT; see ./LICENSE).
import { Toolbar, ToolbarButton } from "../ui/toolbar";
import { Check, Copy, RefreshCw } from "lucide-react";

export function MessageActions({ copied, disabled, onCopy, onRetry }: {
  copied: boolean;
  disabled: boolean;
  onCopy(): void;
  onRetry(): void;
}) {
  return (
    <Toolbar aria-label="Message actions" className="mt-2 flex min-h-8 items-center gap-0.5">
      <ToolbarButton aria-label="Copy response" title={copied ? "Copied" : "Copy response"} onClick={onCopy}>
        {copied ? <Check className="stealth-action-enter size-3.5" /> : <Copy className="size-3.5" />}
      </ToolbarButton>
      <ToolbarButton aria-label="Retry response" title="Retry response" disabled={disabled} onClick={onRetry}>
        <RefreshCw className="size-3.5" />
      </ToolbarButton>
      <span className="sr-only" role="status">{copied ? "Response copied." : ""}</span>
    </Toolbar>
  );
}

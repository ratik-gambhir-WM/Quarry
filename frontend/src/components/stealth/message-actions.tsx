// Adapted from https://stealth.pm/ui/message-actions (MIT; see ./LICENSE).
import { Toolbar } from "@base-ui/react/toolbar";
import { Check, Copy, RefreshCw } from "lucide-react";

export function MessageActions({ copied, disabled, onCopy, onRetry }: {
  copied: boolean;
  disabled: boolean;
  onCopy(): void;
  onRetry(): void;
}) {
  const buttonClass = "relative inline-flex size-8 items-center justify-center rounded-[6px] text-muted outline-none transition-[background-color,color,scale] duration-150 hover:bg-surface-container-high hover:text-on-surface active:scale-90 focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-50 motion-reduce:transition-none motion-reduce:active:scale-100 pointer-coarse:size-11";
  return (
    <Toolbar.Root aria-label="Message actions" className="mt-2 flex min-h-8 items-center gap-0.5">
      <Toolbar.Button aria-label="Copy response" title={copied ? "Copied" : "Copy response"} onClick={onCopy} className={buttonClass}>
        {copied ? <Check className="stealth-action-enter size-3.5" /> : <Copy className="size-3.5" />}
      </Toolbar.Button>
      <Toolbar.Button aria-label="Retry response" title="Retry response" disabled={disabled} onClick={onRetry} className={buttonClass}>
        <RefreshCw className="size-3.5" />
      </Toolbar.Button>
      <span className="sr-only" role="status">{copied ? "Response copied." : ""}</span>
    </Toolbar.Root>
  );
}

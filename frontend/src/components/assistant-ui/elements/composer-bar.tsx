import { AuiIf, ComposerPrimitive } from "@assistant-ui/react";
import { ArrowUp, Square } from "lucide-react";
import type { FC } from "react";
import { cn } from "../../../lib/utils";
import { Button } from "../../ui/button";

type ComposerBarProps = {
  compact: boolean;
};

// Scoped adaptation of assistant-ui's runtime Composer element (2026-09-15).
// Quarry keeps only the supported text input and send/cancel actions.
export const ComposerBar: FC<ComposerBarProps> = ({ compact }) => (
  <ComposerPrimitive.Root
    className={cn(
      "group/query-composer relative flex w-full flex-col rounded-2xl border border-outline-variant/40 bg-surface-container-lowest p-2 shadow-[0_12px_34px_rgba(7,1,84,0.07)] transition-[border-color,box-shadow,border-radius] focus-within:border-outline/50 focus-within:shadow-[0_14px_38px_rgba(7,1,84,0.1)]",
      compact && "data-[compact]:min-h-14 data-[compact]:flex-row data-[compact]:items-end data-[compact]:rounded-full data-[compact]:p-1.5",
    )}
    compact={compact}
    data-slot="query-composer"
  >
    <ComposerPrimitive.Input
      aria-label="Message input"
      autoFocus
      className={cn(
        "max-h-48 min-h-16 w-full resize-none bg-transparent px-2.5 py-2 text-[15px] leading-6 text-on-surface outline-none placeholder:text-muted",
        compact && "group-data-[compact]/query-composer:min-h-11 group-data-[compact]/query-composer:px-3.5 group-data-[compact]/query-composer:py-2.5",
      )}
      data-slot="query-composer-input"
      enterKeyHint="send"
      placeholder="Send a message..."
      rows={compact ? 1 : 2}
    />
    <div
      className={cn(
        "flex justify-end",
        compact && "self-end group-data-[compact]/query-composer:shrink-0 group-data-[compact]/query-composer:self-center",
      )}
      data-slot="query-composer-actions"
    >
      <AuiIf condition={(state) => !state.thread.isRunning}>
        <ComposerPrimitive.Send asChild>
          <Button
            aria-label="Send message"
            className="rounded-full"
            size="icon-sm"
            title="Send message"
            type="button"
          >
            <ArrowUp />
          </Button>
        </ComposerPrimitive.Send>
      </AuiIf>
      <AuiIf condition={(state) => state.thread.isRunning}>
        <ComposerPrimitive.Cancel asChild>
          <Button
            aria-label="Stop generating"
            className="rounded-full"
            size="icon-sm"
            title="Stop generating"
            type="button"
          >
            <Square className="fill-current" />
          </Button>
        </ComposerPrimitive.Cancel>
      </AuiIf>
    </div>
  </ComposerPrimitive.Root>
);

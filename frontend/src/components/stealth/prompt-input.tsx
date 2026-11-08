// Adapted from https://stealth.pm/ui/prompt-input (MIT; see ./LICENSE).
// Quarry owns drafts and transport; only supported text-chat controls are exposed.
import { ArrowUp, FileText, Paperclip, Square, Upload, X } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useEffect, useId, useRef, useState } from "react";
import { cn } from "../../lib/utils";
import { Button } from "../ui/button";
import { CHAT_FILE_ACCEPT } from "../chat/chatAttachments";
import { isChatModelValid, type ChatModelSelection } from "../chat/chatModel";
import { AttachmentList } from "./attachment-list";
import type { ChatFileReference } from "../../contracts/quarryApi";
import { FileMentionPicker } from "../chat/FileMentionPicker";
import { mentionAt } from "../chat/fileMentions";
import { ModelPicker } from "./model-picker";

type PromptInputProps = {
  references?: readonly ChatFileReference[];
  onReferencesChange?(references: readonly ChatFileReference[]): void;
  scopeDealId?: string | null;
  value: string;
  onValueChange(value: string): void;
  onSend(): void;
  onStop(): void;
  generating: boolean;
  disabled: boolean;
  compact: boolean;
  condensed: boolean;
  files: readonly File[];
  onFilesAdd(files: readonly File[]): void;
  onFileRemove(index: number): void;
  model: ChatModelSelection;
  onModelChange(model: ChatModelSelection): void;
  error: string | null;
};

export function PromptInput({
  value,
  references = [],
  onReferencesChange,
  scopeDealId = null,
  onValueChange,
  onSend,
  onStop,
  generating,
  disabled,
  compact,
  condensed,
  files,
  onFilesAdd,
  onFileRemove,
  model,
  onModelChange,
  error,
}: PromptInputProps) {
  const area = useRef<HTMLTextAreaElement>(null);
  const reduce = useReducedMotion();
  const previouslyDisabled = useRef(disabled);
  useEffect(() => {
    if (previouslyDisabled.current && !disabled) area.current?.focus({ preventScroll: true });
    previouslyDisabled.current = disabled;
  }, [disabled]);
  const fileInput = useRef<HTMLInputElement>(null);
  const dragDepth = useRef(0);
  const [dragging, setDragging] = useState(false);
  const noticeId = useId();
  const mentionId = useId();
  const [mention, setMention] = useState<ReturnType<typeof mentionAt>>(null);
  const [activeMention, setActiveMention] = useState(0);
  const selectMention = (file: ChatFileReference) => {
    if (!mention || !onReferencesChange || references.length + files.length >= 20) return;
    if (!references.some((reference) => reference.fileId === file.fileId && reference.dealId === file.dealId)) {
      onReferencesChange([...references, file]);
    }
    onValueChange(value.slice(0, mention.start) + value.slice(mention.end));
    const caret = mention.start;
    setMention(null);
    requestAnimationFrame(() => { area.current?.focus(); area.current?.setSelectionRange(caret, caret); });
  };
  const modelValid = isChatModelValid(model);
  const canSend = !disabled && !generating && modelValid && Boolean(value.trim());
  const notice = error || (!modelValid ? "Enter a valid model ID (up to 128 characters) to send." : null);
  const focus = () => area.current?.focus({ preventScroll: true });
  const send = () => {
    if (!canSend) return;
    setMention(null);
    onSend();
    focus();
  };
  const stop = () => {
    onStop();
    focus();
  };
  const textClass =
    "col-start-1 row-start-1 min-w-0 whitespace-pre-wrap px-3.5 pt-3 pb-1 text-[15px] leading-6 [overflow-wrap:anywhere]";
  const sizeStyle = { minHeight: compact ? 40 : 64, maxHeight: 192 };

  return (
    <form
      className={cn(
        "relative isolate flex w-full flex-col rounded-[16px] border border-outline-variant bg-surface-container-lowest shadow-sm transition-[border-color,box-shadow] duration-150 motion-reduce:transition-none",
        condensed && "[&>div:last-child]:p-1.5",
      )}
      data-slot="query-composer"
      data-compact={compact ? "" : undefined}
      data-state={generating ? "generating" : "idle"}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setMention(null);
      }}
      onDragEnter={(event) => {
        if (disabled || !event.dataTransfer.types.includes("Files")) return;
        event.preventDefault();
        dragDepth.current += 1;
        setDragging(true);
      }}
      onDragOver={(event) => {
        if (disabled || !event.dataTransfer.types.includes("Files")) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
      }}
      onDragLeave={(event) => {
        if (!event.dataTransfer.types.includes("Files")) return;
        dragDepth.current = Math.max(0, dragDepth.current - 1);
        if (dragDepth.current === 0) setDragging(false);
      }}
      onDrop={(event) => {
        if (disabled || !event.dataTransfer.types.includes("Files")) return;
        event.preventDefault();
        dragDepth.current = 0;
        setDragging(false);
        onFilesAdd(Array.from(event.dataTransfer.files));
        focus();
      }}
      onSubmit={(event) => {
        event.preventDefault();
        send();
      }}
    >
      {mention && onReferencesChange ? (
        <FileMentionPicker
          id={mentionId}
          dealId={scopeDealId}
          query={mention.query}
          selected={references}
          activeIndex={activeMention}
          limitReached={references.length + files.length >= 20}
          onSelect={selectMention}
          onRetry={focus}
        />
      ) : null}
      {references.length > 0 ? (
        <div className="flex flex-wrap gap-1.5 px-3.5 pt-3" aria-label="Referenced files">
          {references.map((file) => (
            <span
              key={`${file.dealId}:${file.fileId}`}
              className="inline-flex max-w-full items-center gap-1.5 rounded-md bg-file-mention px-2 py-1 text-xs text-on-file-mention"
            >
              <FileText aria-hidden className="size-3.5 shrink-0" />
              <span className="truncate" title={`${file.name} · ${file.dealId}`}>{file.name}</span>
              <button
                type="button"
                disabled={disabled}
                aria-label={`Remove reference ${file.name}`}
                className="min-h-6 min-w-6 shrink-0 rounded-sm p-0.5 hover:bg-white/15 focus-visible:outline-2 disabled:opacity-50"
                onClick={() => {
                  onReferencesChange?.(references.filter((reference) => reference !== file));
                  focus();
                }}
              >
                <X aria-hidden className="size-3.5" />
              </button>
            </span>
          ))}
        </div>
      ) : null}
      <AttachmentList files={files} disabled={disabled} onRemove={(index) => { onFileRemove(index); focus(); }} />
      <input
        ref={fileInput}
        aria-label="Choose attachments"
        type="file"
        multiple
        accept={CHAT_FILE_ACCEPT}
        className="hidden"
        disabled={disabled}
        onChange={(event) => {
          onFilesAdd(Array.from(event.target.files ?? []));
          event.target.value = "";
          focus();
        }}
      />
      <div className="grid min-w-0">
        <div
          aria-hidden
          className={cn(textClass, "invisible overflow-hidden")}
          style={sizeStyle}
        >
          {value + " "}
        </div>
        <textarea
          ref={area}
          aria-label="Message input"
          autoFocus
          disabled={disabled}
          aria-describedby={notice ? noticeId : undefined}
          onPaste={(event) => {
            if (event.clipboardData.files.length > 0) {
              event.preventDefault();
              onFilesAdd(Array.from(event.clipboardData.files));
            }
          }}
          className={cn(
            textClass,
            "w-full resize-none overflow-y-auto overscroll-contain bg-transparent text-on-surface outline-none placeholder:text-muted [scrollbar-width:thin]",
          )}
          style={sizeStyle}
          rows={compact ? 1 : 2}
          value={value}
          aria-activedescendant={mention ? `${mentionId}-active` : undefined}
          aria-controls={mention ? mentionId : undefined}
          aria-expanded={mention ? true : undefined}
          aria-autocomplete="list"
          onSelect={(event) => {
            const next = mentionAt(event.currentTarget.value, event.currentTarget.selectionStart);
            if (event.currentTarget.selectionStart !== event.currentTarget.selectionEnd || !next) setMention(null);
            else if (mention && (mention.start !== next.start || mention.end !== next.end)) {
              setMention(next);
              setActiveMention(0);
            }
          }}
          onChange={(event) => {
            onValueChange(event.target.value);
            setMention(mentionAt(event.target.value, event.target.selectionStart));
            setActiveMention(0);
          }}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing || event.keyCode === 229) return;
            if (mention && onReferencesChange && ["ArrowDown", "ArrowUp", "Enter", "Escape"].includes(event.key)) {
              event.preventDefault();
              event.stopPropagation();
              if (event.key === "Escape") setMention(null);
              else if (event.key === "Enter") document.getElementById(mentionId)?.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.click();
              else setActiveMention((index) => Math.max(0, index + (event.key === "ArrowDown" ? 1 : -1)));
              return;
            }
            if (event.key === "Backspace" && event.currentTarget.selectionStart === 0 && event.currentTarget.selectionEnd === 0 && references.length) {
              event.preventDefault();
              onReferencesChange?.(references.slice(0, -1));
              return;
            }
            if (event.key === "Escape" && generating) {
              event.preventDefault();
              // Escape stops the response before it can close the Data Room panel.
              event.stopPropagation();
              stop();
            } else if (
              event.key === "Enter" &&
              (!event.shiftKey || event.metaKey || event.ctrlKey)
            ) {
              event.preventDefault();
              send();
            }
          }}
          enterKeyHint="send"
          placeholder="Ask anything…"
        />
      </div>
      {notice ? <p id={noticeId} role="alert" className="px-3.5 pt-1 text-xs text-error">{notice}</p> : null}
      <div
        className="flex items-end justify-between gap-2 p-2"
        data-slot="query-composer-actions"
      >
        <div className="flex shrink-0 items-center gap-1 self-end">
          <Button
            aria-label="Attach files"
            title="Attach files (up to 20 files, 50 MiB total)"
            className="size-8 rounded-[8px] text-muted pointer-coarse:size-11"
            disabled={disabled}
            onClick={() => fileInput.current?.click()}
            variant="ghost"
            size="icon"
            type="button"
          >
            <Paperclip />
          </Button>
        </div>
        <div className="ml-auto flex min-w-0 items-end gap-1">
          <ModelPicker value={model} onValueChange={onModelChange} disabled={disabled} />
          <Button
            aria-label={generating ? "Stop generating" : "Send message"}
            title={generating ? "Stop generating" : "Send message"}
            className="relative size-8 overflow-hidden rounded-[8px] bg-chat-action text-on-chat-action hover:bg-[color-mix(in_srgb,var(--theme-chat-action),white_8%)] disabled:opacity-100 disabled:[&_svg]:opacity-55 pointer-coarse:size-11 motion-reduce:transition-none"
            disabled={!generating && !canSend}
            onClick={generating ? stop : send}
            size="icon"
            type="button"
          >
            <motion.div
              key={generating ? "stop" : "send"}
              initial={
                reduce ? false : { opacity: 0, y: generating ? -6 : 6, scale: 0.7 }
              }
              animate={{ opacity: 1, y: 0, scale: 1 }}
              transition={
                reduce
                  ? { duration: 0 }
                  : { type: "spring", stiffness: 420, damping: 28 }
              }
            >
              {generating ? (
                <Square className="size-3 fill-current" />
              ) : (
                <ArrowUp className="size-4" />
              )}
            </motion.div>
            {generating ? (
              <span
                aria-hidden
                className="absolute inset-0.5 animate-spin rounded-[6px] border border-current border-t-transparent opacity-40 motion-reduce:animate-none"
              />
            ) : null}
          </Button>
        </div>
      </div>
      {dragging ? (
        <div aria-hidden className="pointer-events-none absolute inset-1 z-10 grid place-items-center rounded-[12px] border border-dashed border-primary bg-surface-container-lowest/95 text-sm text-on-surface">
          <span className="flex items-center gap-2"><Upload className="size-4" />Drop to attach</span>
        </div>
      ) : null}
    </form>
  );
}

// Attachment chip presentation adapted from Stealth Prompt Input (MIT; see ./LICENSE).
import { FileText, X } from "lucide-react";
import { formatChatFileSize } from "../chat/chatAttachments";

export function AttachmentList({ files, onRemove, disabled = false }: {
  files: readonly File[];
  onRemove?: (index: number) => void;
  disabled?: boolean;
}) {
  if (files.length === 0) return null;
  return (
    <ul aria-label={onRemove ? "Attachments" : "Attached files"} className="flex max-w-full gap-2 overflow-x-auto overscroll-x-contain p-2 [scrollbar-width:thin]">
      {files.map((file, index) => (
        <li key={`${file.name}:${index}`} className="stealth-action-enter flex max-w-56 shrink-0 items-center gap-2 rounded-[10px] border border-outline-variant bg-surface-container-low px-2 py-1.5 text-xs text-on-surface">
          <FileText aria-hidden className="size-4 shrink-0 text-muted" />
          <span className="min-w-0">
            <span className="block truncate" title={file.name}>{file.name}</span>
            <span className="block text-[10px] text-muted">{formatChatFileSize(file.size)}</span>
          </span>
          {onRemove ? (
            <button
              aria-label={`Remove ${file.name}`}
              title={`Remove ${file.name}`}
              className="ml-1 inline-flex size-6 shrink-0 items-center justify-center rounded-[6px] text-muted outline-none hover:bg-surface-container-high focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-50 pointer-coarse:size-11"
              disabled={disabled}
              onClick={() => onRemove(index)}
              type="button"
            >
              <X className="size-3.5" />
            </button>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

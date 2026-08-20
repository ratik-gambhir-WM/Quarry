import { useEffect, useRef, useState } from "react";
import { Icon } from "../ui/Icon";

type ConnectSharePointModalProps = {
  onClose: () => void;
};

export function ConnectSharePointModal({ onClose }: ConnectSharePointModalProps) {
  const teamsChannelInputRef = useRef<HTMLInputElement>(null);
  const [sharePointLink, setSharePointLink] = useState("");
  const [teamsChannelLink, setTeamsChannelLink] = useState("");

  useEffect(() => {
    teamsChannelInputRef.current?.focus();

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  return (
    <div
      className="modal-backdrop fixed inset-0 z-[100] flex items-center justify-center px-4 py-6"
      role="presentation"
    >
      <button
        aria-label="Close connect to SharePoint dialog"
        className="absolute inset-0 cursor-default"
        onClick={onClose}
        type="button"
      />

      <section
        aria-describedby="connect-sharepoint-description"
        aria-labelledby="connect-sharepoint-title"
        aria-modal="true"
        className="relative z-10 w-full max-w-[560px] overflow-hidden rounded-[20px] border border-outline-variant bg-surface-container-lowest shadow-[0_28px_70px_rgba(7,1,84,0.24)]"
        role="dialog"
      >
        <header className="flex items-start justify-between gap-5 border-b border-outline-variant px-6 py-5">
          <div>
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-muted">Data Room</p>
            <h2
              className="mt-2 text-[2rem] font-bold leading-none text-text-main [font-family:var(--font-heading)]"
              id="connect-sharepoint-title"
            >
              Connect to SharePoint
            </h2>
            <p className="mt-2 text-[13px] leading-5 text-muted" id="connect-sharepoint-description">
              Add the Teams channel and SharePoint locations associated with this data room.
            </p>
          </div>
          <button
            aria-label="Close connect to SharePoint dialog"
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
            onClick={onClose}
            type="button"
          >
            <Icon className="h-5 w-5 rotate-45" name="plus" />
          </button>
        </header>

        <div className="space-y-5 px-6 py-5">
          <ConnectionLinkField
            id="teams-channel-link"
            inputRef={teamsChannelInputRef}
            label="Teams channel link"
            onChange={setTeamsChannelLink}
            placeholder="https://teams.microsoft.com/l/channel/..."
            value={teamsChannelLink}
          />
          <ConnectionLinkField
            id="sharepoint-link"
            label="SharePoint link"
            onChange={setSharePointLink}
            placeholder="https://yourcompany.sharepoint.com/sites/..."
            value={sharePointLink}
          />
        </div>

        <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-outline-variant bg-surface-container-low/70 px-6 py-4">
          <p className="text-[12px] text-muted">Connection submission is coming soon.</p>
          <div className="flex items-center gap-3">
            <button
              className="rounded-full px-5 py-3 text-[13px] font-semibold text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
              onClick={onClose}
              type="button"
            >
              Cancel
            </button>
            <button
              className="inline-flex min-w-[132px] cursor-not-allowed items-center justify-center gap-2 rounded-full bg-primary-container px-6 py-3 text-[13px] font-semibold text-on-primary-container opacity-50"
              disabled
              title="Connection submission is coming soon"
              type="button"
            >
              <Icon className="h-4 w-4" name="sharepoint" />
              <span>Connect</span>
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}

type ConnectionLinkFieldProps = {
  id: string;
  inputRef?: React.RefObject<HTMLInputElement | null>;
  label: string;
  onChange: (value: string) => void;
  placeholder: string;
  value: string;
};

function ConnectionLinkField({
  id,
  inputRef,
  label,
  onChange,
  placeholder,
  value,
}: ConnectionLinkFieldProps) {
  return (
    <div className="space-y-2">
      <label className="px-1 text-[11px] font-bold uppercase tracking-[0.16em] text-muted" htmlFor={id}>
        {label}
      </label>
      <input
        className="w-full rounded-2xl border border-outline-variant bg-surface-container-lowest px-4 py-3 text-[14px] text-text-main outline-none transition placeholder:text-muted/60 focus:border-primary-container focus:ring-4 focus:ring-primary-fixed/40"
        id={id}
        onChange={(event) => onChange(event.currentTarget.value)}
        placeholder={placeholder}
        ref={inputRef}
        type="url"
        value={value}
      />
    </div>
  );
}

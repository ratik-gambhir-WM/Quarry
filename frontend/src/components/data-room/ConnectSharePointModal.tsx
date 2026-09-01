import { useEffect, useRef, useState } from "react";
import { DialogBackdrop } from "../ui/DialogBackdrop";
import { DialogHeader } from "../ui/DialogHeader";
import { Icon } from "../ui/Icon";
import { ModalTextField } from "../ui/ModalField";

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
    <DialogBackdrop closeLabel="Close connect to SharePoint dialog" onClose={onClose}>
      <section
        aria-describedby="connect-sharepoint-description"
        aria-labelledby="connect-sharepoint-title"
        aria-modal="true"
        className="relative z-10 w-full max-w-[560px] overflow-hidden rounded-[20px] border border-outline-variant bg-surface-container-lowest shadow-[0_28px_70px_rgba(7,1,84,0.24)]"
        role="dialog"
      >
        <DialogHeader
          className="border-b border-outline-variant px-6 py-5"
          closeLabel="Close connect to SharePoint dialog"
          description={
            <span id="connect-sharepoint-description">
              Add the Teams channel and SharePoint locations associated with this data room.
            </span>
          }
          eyebrow="Data Room"
          onClose={onClose}
          title="Connect to SharePoint"
          titleId="connect-sharepoint-title"
        />

        <div className="space-y-5 px-6 py-5">
          <ModalTextField
            id="teams-channel-link"
            label="Teams channel link"
            onValueChange={setTeamsChannelLink}
            placeholder="https://teams.microsoft.com/l/channel/..."
            ref={teamsChannelInputRef}
            required={false}
            type="url"
            value={teamsChannelLink}
          />
          <ModalTextField
            id="sharepoint-link"
            label="SharePoint link"
            onValueChange={setSharePointLink}
            placeholder="https://yourcompany.sharepoint.com/sites/..."
            required={false}
            type="url"
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
    </DialogBackdrop>
  );
}

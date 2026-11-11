import { useCallback, useEffect, useId, useRef, useState, type RefObject } from "react";
import { runtime } from "@quarry/runtime";
import { Trash2 } from "lucide-react";
import { waitForDealDeletion } from "../../api/dealDeletion";
import { sessionStore } from "../../auth/sessionStore";
import type { DealDeletion } from "../../contracts/quarryApi";
import { Input } from "../ui/input";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogTitle,
} from "../ui/stealth/alert-dialog";
import { Button } from "../ui/stealth/button";
import type { DeleteDealTarget } from "./DealActionsMenu";

type DeletionState =
  | { status: "confirm"; error?: string }
  | { status: "submitting" | "tracking" | "completed" }
  | { status: "blocked" | "status-error"; operation: DealDeletion };

export function DeleteDealDialog({
  deal,
  fallbackFocus,
  onClose,
}: {
  deal: DeleteDealTarget;
  fallbackFocus: RefObject<HTMLElement | null>;
  onClose: () => void;
}) {
  const [phrase, setPhrase] = useState("");
  const [state, setState] = useState<DeletionState>({ status: "confirm" });
  const request = useRef<AbortController | null>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const inputId = useId();
  const expectedPhrase = `delete ${deal.name}`;
  const confirming = state.status === "confirm" || state.status === "submitting";
  const busy = state.status === "submitting" || state.status === "tracking";
  const focusCloseButton = useCallback((button: HTMLButtonElement | null) => {
    cancelButton.current = button;
    button?.focus();
  }, []);

  useEffect(() => {
    const unsubscribe = sessionStore.subscribe(() => {
      request.current?.abort();
      onClose();
    });
    return () => {
      request.current?.abort();
      unsubscribe();
    };
  }, [onClose]);

  async function deleteOrCheckStatus(previous?: DealDeletion) {
    if (request.current || (!previous && phrase !== expectedPhrase)) return;
    const controller = new AbortController();
    request.current = controller;
    let operation = previous;
    if (previous) cancelButton.current?.focus();
    setState({ status: previous ? "tracking" : "submitting" });
    try {
      const receipt = previous
        ? await runtime.api.getDealDeletion(previous.deletionId)
        : await runtime.api.deleteDeal(deal.id);
      if (controller.signal.aborted) return;
      if (!receipt) {
        setState({ status: "completed" });
        return;
      }
      operation = receipt;
      setState({ status: "tracking" });
      const result = await waitForDealDeletion(runtime.api, receipt, controller.signal);
      if (controller.signal.aborted) return;
      setState(result.status === "blocked"
        ? { status: "blocked", operation: result }
        : { status: "completed" });
    } catch {
      if (controller.signal.aborted) return;
      setState(operation
        ? { status: "status-error", operation }
        : {
            status: "confirm",
            error: "Deletion could not be confirmed. Try again. If the request reached the server, cleanup may already be running.",
          });
    } finally {
      if (request.current === controller) request.current = null;
    }
  }

  return (
    <AlertDialog
      open
      onOpenChange={(open, details) => {
        if (open) return;
        if (state.status === "submitting") details.cancel();
        else onClose();
      }}
    >
      <AlertDialogContent
        className="max-h-[calc(100dvh-2rem)] max-w-[460px] overflow-y-auto max-sm:max-h-[calc(100dvh-1rem)]"
        initialFocus={cancelButton}
        finalFocus={() => deal.trigger?.isConnected ? deal.trigger : fallbackFocus.current}
      >
        <div className="mb-3 flex size-9 items-center justify-center rounded-full bg-destructive/10 text-destructive">
          <Trash2 aria-hidden="true" className="size-4" />
        </div>
        <AlertDialogTitle className="break-words">
          {confirming ? `Delete ${deal.name}?` : state.status === "completed" ? "Deal deleted" : "Deleting deal"}
        </AlertDialogTitle>
        <AlertDialogDescription className="break-words">
          {confirming
            ? "This permanently deletes the deal, its uploaded documents, and all linked chats for everyone. This cannot be undone."
            : state.status === "completed"
              ? `${deal.name} and its linked deal data have been permanently deleted.`
              : "The deal is hidden from the workspace. Cleanup continues on the server even if you close this dialog."}
        </AlertDialogDescription>
        {confirming ? (
          <form className="mt-4" onSubmit={(event) => { event.preventDefault(); void deleteOrCheckStatus(); }}>
            <p className="mb-4 text-[13px] leading-5 text-muted">
              Personal notebooks are kept, with their deal link removed. Company records and original files outside Quarry are kept.
            </p>
            <label id={`${inputId}-hint`} htmlFor={inputId} className="block text-[13px] leading-5">
              To confirm, type <strong className="break-words select-all font-medium">{expectedPhrase}</strong>
            </label>
            <Input
              id={inputId}
              className="mt-2"
              aria-label="Deletion confirmation"
              aria-describedby={`${inputId}-hint`}
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
              disabled={busy}
              value={phrase}
              onChange={(event) => setPhrase(event.target.value)}
            />
            {state.status === "confirm" && state.error ? <p role="alert" className="mt-3 text-[13px] text-destructive">{state.error}</p> : null}
            <div className="mt-5 flex justify-end gap-2">
              <Button ref={cancelButton} type="button" disabled={busy} onClick={onClose}>Cancel</Button>
              <Button type="submit" variant="danger" disabled={phrase !== expectedPhrase} loading={busy} loadingText="Deleting…">Delete deal</Button>
            </div>
          </form>
        ) : (
          <>
            <div role={state.status === "blocked" || state.status === "status-error" ? "alert" : "status"} className="mt-4 text-[13px] leading-5">
              {state.status === "tracking" ? "Deletion is in progress…" : null}
              {state.status === "completed" ? "Cleanup is complete." : null}
              {state.status === "status-error" ? "Unable to check cleanup status. Deletion has already been accepted. Check status again; do not recreate the deal." : null}
              {state.status === "blocked" ? <>
                <p>Cleanup needs administrator attention. The deal remains inaccessible.</p>
                <p className="mt-2 break-all text-muted">Operation: {state.operation.deletionId}</p>
              </> : null}
            </div>
            <div className="mt-5 flex justify-end gap-2">
              <Button ref={focusCloseButton} onClick={onClose}>{state.status === "tracking" ? "Continue in background" : "Close"}</Button>
              {state.status === "status-error" ? <Button variant="primary" onClick={() => void deleteOrCheckStatus(state.operation)}>Check status</Button> : null}
            </div>
          </>
        )}
      </AlertDialogContent>
    </AlertDialog>
  );
}

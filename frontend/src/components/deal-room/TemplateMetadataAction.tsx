import { useEffect, useRef, useState } from "react";
import { FileJson, MoreHorizontal, RefreshCw } from "lucide-react";
import { runtime } from "@quarry/runtime";
import type {
  TemplateProcessingProgress,
  TemplateRetrieval,
  TemplateRetrievalMetadata,
} from "@/contracts/quarryApi";
import { templateDisplayName } from "@/data/deliverables";
import { Button } from "../ui/button";
import {
  Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,
} from "../ui/dialog";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import { Spinner } from "../ui/spinner";

// Stealth UI's quiet menu/dialog pattern, using Quarry's existing accessible primitives and tokens.
export function TemplateMetadataAction({ templateId, disabled }: {
  templateId: string;
  disabled: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState(0);
  const trigger = useRef<HTMLButtonElement>(null);
  const displayName = templateDisplayName(templateId);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            ref={trigger}
            aria-label={`More actions for ${displayName}`}
            className="absolute right-3 top-3 z-10 bg-surface-container-lowest text-text-main shadow-sm active:scale-95 motion-reduce:transition-none"
            disabled={disabled}
            size="icon"
            type="button"
            variant="outline"
          >
            <MoreHorizontal aria-hidden="true" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent
          align="end"
          className="min-w-[11rem] motion-reduce:animate-none"
          onCloseAutoFocus={(event) => {
            if (open) event.preventDefault();
          }}
        >
          <DropdownMenuItem onSelect={() => setOpen(true)}>
            <FileJson aria-hidden="true" /> View metadata
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <DialogContent
        className="flex max-h-[85dvh] max-w-[50rem] flex-col gap-0 overflow-hidden p-0 motion-reduce:animate-none"
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          trigger.current?.focus();
        }}
      >
        <DialogHeader className="shrink-0 border-b border-outline-variant px-6 py-5 pr-12 text-left">
          <DialogTitle>Template metadata</DialogTitle>
          <DialogDescription className="break-all">{displayName}</DialogDescription>
        </DialogHeader>
        {open ? (
          <MetadataInspector
            key={`${templateId}:${revision}`}
            templateId={templateId}
            onRefresh={() => setRevision((value) => value + 1)}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

type InspectionState =
  | { status: "loading" }
  | { status: "error" }
  | { status: "ready"; data: TemplateRetrieval; vectorsUnavailable: boolean; vectorsLoading: boolean };

const isProcessing = (progress: TemplateProcessingProgress) =>
  progress.status === "pending" || progress.status === "processing";

function MetadataInspector({ templateId, onRefresh }: {
  templateId: string;
  onRefresh: () => void;
}) {
  const [state, setState] = useState<InspectionState>({ status: "loading" });
  const [retryState, setRetryState] = useState<"idle" | "pending" | "error">("idle");
  const retryInFlight = useRef(false);
  const mounted = useRef(true);
  const canRetry = state.status === "ready" && (
    state.data.classification.status === "failed" || state.data.embedding.status === "failed"
  );

  async function retryProcessing() {
    if (!canRetry || retryInFlight.current) return;
    retryInFlight.current = true;
    setRetryState("pending");
    try {
      await runtime.api.retryTemplateProcessing(templateId);
      if (mounted.current) onRefresh();
    } catch {
      if (mounted.current) setRetryState("error");
    } finally {
      retryInFlight.current = false;
    }
  }

  useEffect(() => {
    mounted.current = true;
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function load() {
      try {
        let data = await runtime.api.getTemplateRetrieval(templateId);
        if (!active) return;
        // Read metadata independently: a disabled vector-inspection gate must not hide it.
        let vectorsUnavailable = false;
        if (data.embedding.status === "ready") {
          setState({ status: "ready", data, vectorsUnavailable: false, vectorsLoading: true });
          try {
            data = await runtime.api.getTemplateRetrieval(templateId, true);
          } catch {
            vectorsUnavailable = true;
          }
        }
        if (!active) return;
        setState({ status: "ready", data, vectorsUnavailable, vectorsLoading: false });
        if (isProcessing(data.classification) || isProcessing(data.embedding)) {
          timer = setTimeout(() => { void load(); }, 3000);
        }
      } catch {
        if (active) setState({ status: "error" });
      }
    }
    void load();
    return () => {
      mounted.current = false;
      active = false;
      clearTimeout(timer);
    };
  }, [templateId]);

  return (
    <>
      <div
        className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-6 py-5 text-left"
        tabIndex={0}
        aria-label="Metadata and embeddings"
      >
        {state.status === "loading" ? (
          <ProcessingMessage message="Loading metadata…" />
        ) : null}
        {state.status === "error" ? (
          <p role="alert" className="py-8 text-sm text-destructive">
            Metadata could not be loaded. Refresh to try again.
          </p>
        ) : null}
        {state.status === "ready" ? (
          <InspectionResult
            data={state.data}
            vectorsUnavailable={state.vectorsUnavailable}
            vectorsLoading={state.vectorsLoading}
          />
        ) : null}
        {retryState === "error" ? (
          <p role="alert" className="mt-4 text-sm text-destructive">
            Processing could not be restarted. Refresh its status before trying again.
          </p>
        ) : null}
      </div>
      <div className="flex shrink-0 items-center justify-between gap-3 border-t border-outline-variant px-6 py-3">
        <span className="text-xs text-muted">Saved slide data</span>
        <div className="flex gap-2">
          <Button disabled={state.status === "loading" || retryState === "pending"} onClick={onRefresh} size="sm" variant="outline">
            <RefreshCw aria-hidden="true" className="size-3.5" /> Refresh
          </Button>
          {canRetry ? (
            <Button
              aria-busy={retryState === "pending"}
              disabled={retryState === "pending"}
              onClick={() => void retryProcessing()}
              size="sm"
            >
              {retryState === "pending" ? <Spinner aria-hidden="true" className="size-3.5 motion-reduce:animate-none" /> : null}
              {retryState === "pending" ? "Retrying…" : "Retry"}
            </Button>
          ) : null}
        </div>
      </div>
    </>
  );
}

function InspectionResult({ data, vectorsUnavailable, vectorsLoading }: {
  data: TemplateRetrieval;
  vectorsUnavailable: boolean;
  vectorsLoading: boolean;
}) {
  const metadataProcessing = isProcessing(data.classification);
  const waitingForMetadata = metadataProcessing && data.embedding.status === "not_ready";
  const embeddingsProcessing = isProcessing(data.embedding) || waitingForMetadata;
  return (
    <>
      <div className="mb-5 flex flex-wrap gap-2 text-xs">
        <span className="rounded-md bg-surface-container px-2 py-1">
          Metadata: {statusLabel(data.classification.status)}
        </span>
        <span className="rounded-md bg-surface-container px-2 py-1">
          Embeddings: {waitingForMetadata ? "Waiting for metadata" : statusLabel(data.embedding.status)}
        </span>
      </div>
      {metadataProcessing ? <ProcessingMessage message="Metadata is still processing" /> : null}
      {data.classification.status === "failed" ? (
        <p role="alert" className="mb-5 text-sm text-destructive">
          Metadata processing failed. Your saved slide is still available.
        </p>
      ) : null}
      {data.metadata ? <MetadataFields metadata={data.metadata} /> : (
        !metadataProcessing && data.classification.status !== "failed" ? (
          <p className="mb-5 text-sm text-muted">
            No classification metadata is available for this template.
          </p>
        ) : null
      )}
      <section className="mt-6 border-t border-outline-variant pt-5" aria-label="Embeddings">
        <h3 className="mb-3 text-sm font-semibold">Embeddings</h3>
        {vectorsLoading ? <ProcessingMessage message="Loading embedding values…" /> : null}
        {embeddingsProcessing ? <ProcessingMessage message="Embeddings are still processing" /> : null}
        {data.embedding.status === "failed" ? (
          <p role="alert" className="text-sm text-destructive">
            Embedding generation failed. Any saved metadata remains available above.
          </p>
        ) : null}
        {data.embedding.status === "not_ready" && !metadataProcessing ? (
          <p className="text-sm text-muted">No embeddings are available for this template.</p>
        ) : null}
        {vectorsUnavailable ? (
          <p className="text-sm text-muted">
            Embeddings are saved, but their values are unavailable for inspection. Refresh to try again.
          </p>
        ) : null}
        {data.embeddings ? (
          <>
            <dl className="mb-4 grid gap-3 text-xs sm:grid-cols-2">
              <Field label="Model" value={data.embeddings.model ?? "Unavailable"} />
              <Field label="Dimensions" value={String(data.embeddings.dimensions ?? "Unavailable")} />
            </dl>
            <VectorDetails label="Subject vector" vector={data.embeddings.subjectVector} />
            <VectorDetails label="Capability vector" vector={data.embeddings.capabilityVector} />
          </>
        ) : null}
      </section>
    </>
  );
}

function statusLabel(status: TemplateProcessingProgress["status"]) {
  return {
    not_ready: "Not available", pending: "Queued", processing: "Processing",
    ready: "Ready", failed: "Failed",
  }[status];
}

function ProcessingMessage({ message }: { message: string }) {
  return (
    <div role="status" className="my-4 flex items-center gap-3 rounded-lg bg-surface-container-low px-4 py-4 text-sm text-muted">
      <Spinner aria-hidden="true" className="size-4 shrink-0 motion-reduce:animate-none" />
      <span>{message}</span>
    </div>
  );
}

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="mb-1 text-xs text-muted">{label}</dt>
      <dd className="m-0 whitespace-pre-wrap break-words text-sm leading-relaxed text-text-main">
        {value || "None"}
      </dd>
    </div>
  );
}

function MetadataFields({ metadata }: { metadata: TemplateRetrievalMetadata }) {
  const list = (items: string[]) => items.join(", ");
  const domains = metadata.subject.domains.map((domain) =>
    `${domain.id} (${domain.relevance})${domain.topics.length ? `: ${list(domain.topics)}` : ""}`,
  ).join("\n");
  const slots = metadata.templateFit.contentSlots.map((slot) =>
    `${slot.elementId}: ${slot.role} (${slot.capacity})`,
  ).join("\n");
  return (
    <div className="space-y-5">
    <p className="text-sm leading-relaxed text-text-main">{metadata.subject.summary}</p>
    <dl className="grid gap-x-6 gap-y-4 sm:grid-cols-2">
      <Field label="Domains" value={domains} />
      <Field label="Topics" value={list(metadata.subject.otherTopics)} />
      <Field label="Technologies" value={list(metadata.subject.technologies)} />
      <Field label="Entities" value={list(metadata.subject.entities)} />
      <Field label="Claims" value={metadata.subject.claims.join("\n")} />
      <Field label="Synonyms" value={list(metadata.subject.synonyms)} />
      <Field label="Intent" value={list(metadata.communication.intents)} />
      <Field label="Information types" value={list(metadata.communication.informationTypes)} />
      <Field label="Audience" value={list(metadata.communication.audience)} />
      <Field label="Template archetype" value={metadata.templateFit.archetype} />
      <Field label="Layout" value={metadata.visual.layoutType} />
      <Field label="Content density" value={metadata.visual.contentDensity} />
      <Field label="Visual elements" value={list(metadata.visual.visualElements)} />
      <Field label="Structural features" value={list(metadata.visual.structuralFeatures)} />
      <Field label="Content slots" value={slots} />
      <Field label="Retrieval keywords" value={list(metadata.retrievalKeywords)} />
    </dl>
    <details className="rounded-lg border border-outline-variant p-3">
      <summary className="cursor-pointer text-xs font-medium focus-visible:outline-2 focus-visible:outline-action">
        Metadata JSON
      </summary>
      <pre className="mt-3 max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs" tabIndex={0} aria-label="Metadata JSON">
        {JSON.stringify(metadata, null, 2)}
      </pre>
    </details>
    </div>
  );
}

function VectorDetails({ label, vector }: { label: string; vector: number[] | null }) {
  return (
    <details className="mb-2 rounded-lg border border-outline-variant p-3">
      <summary className="cursor-pointer text-xs font-medium focus-visible:outline-2 focus-visible:outline-action">
        {label} <span className="font-normal text-muted">· {vector?.length ?? 0} values</span>
      </summary>
      <pre className="mt-3 max-h-48 overflow-auto whitespace-pre-wrap break-all font-mono text-xs tabular-nums" tabIndex={0} aria-label={label}>
        {vector ? JSON.stringify(vector, null, 2) : "No vector is available."}
      </pre>
    </details>
  );
}

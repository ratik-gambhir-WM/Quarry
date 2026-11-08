import { ArrowDown } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { cn } from "../../lib/utils";
import { Button } from "../ui/button";
import { Skeleton } from "../ui/skeleton";
import type { AssistantMessage, Message } from "./chatModel";
import { PromptInput } from "../stealth/prompt-input";
import { MessageActions } from "../stealth/message-actions";
import { ShimmerText } from "../stealth/shimmer-text";
import { AttachmentList } from "../stealth/attachment-list";
import { StreamingText } from "../stealth/streaming-text";
import { useAgentRuntimeContext, useThreadRuntimeState } from "./AgentRuntime";

export function QueryChatThread({
  contextTruncated,
  floatingPanel = false,
}: {
  contextTruncated: boolean;
  floatingPanel?: boolean;
}) {
  const state = useThreadRuntimeState((snapshot) => snapshot);
  const shouldReduceMotion = useReducedMotion() ?? false;
  const viewportRef = useRef<HTMLDivElement>(null);
  const [isNearBottom, setIsNearBottom] = useState(true);
  const isEmpty = state.messages.length === 0;

  useEffect(() => {
    const viewport = viewportRef.current;
    if (viewport && isNearBottom) viewport.scrollTop = viewport.scrollHeight;
  }, [isNearBottom, state.messages]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <p className="px-4 py-1 text-center text-xs text-muted-foreground">
        {(state.threads.find((t) => t.threadId === state.selectedThreadId)
          ?.dealId ?? state.scopeDealId)
          ? `Deal chat · ${state.threads.find((t) => t.threadId === state.selectedThreadId)?.dealId ?? state.scopeDealId}`
          : "Freeform chat"}
      </p>
      <div
        className="workspace-scrollbar-hidden relative flex min-h-0 flex-1 flex-col overflow-x-hidden overflow-y-auto"
        data-slot="query-thread-viewport"
        onScroll={(event) => {
          const target = event.currentTarget;
          setIsNearBottom(
            target.scrollHeight - target.scrollTop - target.clientHeight < 48,
          );
        }}
        ref={viewportRef}
      >
        <div
          className={cn(
            "mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 pt-4 sm:px-7",
            isEmpty && "justify-center pb-[12vh]",
          )}
        >
          {isEmpty ? <ThreadWelcome /> : null}
          <div
            className="mb-8 flex flex-col gap-7 empty:hidden"
            data-slot="query-message-group"
          >
            {state.isThreadLoading ? <ThreadHistorySkeleton /> : null}
            {state.threadLoadError ? <ThreadLoadError /> : null}
            {state.messages.map((message) => (
              <ThreadMessage key={message.id} message={message} />
            ))}
          </div>
          <div
            className={cn(
              "relative z-10 flex flex-col",
              isEmpty
                ? "gap-3 pb-4 sm:pb-6"
                : "sticky bottom-0 mt-auto gap-2 py-3",
              !isEmpty &&
                !floatingPanel &&
                "bg-[var(--theme-workspace-surface-bottom)]",
            )}
          >
            {!isNearBottom && !isEmpty ? (
              <Button
                aria-label="Scroll to bottom"
                className="absolute -top-10 self-center rounded-full bg-surface-container-lowest shadow-sm"
                onClick={() =>
                  viewportRef.current?.scrollTo({
                    top: viewportRef.current.scrollHeight,
                    behavior: shouldReduceMotion ? "auto" : "smooth",
                  })
                }
                size="icon-sm"
                title="Scroll to bottom"
                type="button"
                variant="outline"
              >
                <ArrowDown />
              </Button>
            ) : null}
            {contextTruncated ? (
              <p className="text-center text-xs text-muted" role="status">
                Older conversation context was omitted to fit this request.
              </p>
            ) : null}
            <motion.div
              data-query-composer-host
              layout
              layoutId="query-chat-composer"
              transition={
                shouldReduceMotion
                  ? { duration: 0 }
                  : { duration: 0.22, ease: "easeOut" }
              }
            >
              <ComposerBar compact={!isEmpty} condensed={floatingPanel} />
            </motion.div>
          </div>
        </div>
      </div>
    </div>
  );
}

function ThreadWelcome() {
  return (
    <div className="mb-6 text-center">
      <h2 className="text-xl font-semibold tracking-[-0.02em] text-text-main sm:text-2xl">
        How can I help you today?
      </h2>
    </div>
  );
}

function ThreadHistorySkeleton() {
  return (
    <div
      aria-label="Loading previous chat"
      className="space-y-5"
      data-testid="thread-history-skeleton"
      role="status"
    >
      <div className="flex justify-end">
        <Skeleton
          aria-hidden="true"
          className="h-10 w-3/5 rounded-2xl motion-reduce:animate-none"
        />
      </div>
      <div aria-hidden="true" className="space-y-2">
        <Skeleton className="h-4 w-full motion-reduce:animate-none" />
        <Skeleton className="h-4 w-11/12 motion-reduce:animate-none" />
        <Skeleton className="h-4 w-3/5 motion-reduce:animate-none" />
      </div>
    </div>
  );
}

function ThreadLoadError() {
  return (
    <p
      className="rounded-xl border border-error/35 bg-error-container/35 p-3 text-sm text-error"
      role="status"
    >
      Couldn’t load this chat. Please select it again.
    </p>
  );
}

function ComposerBar({
  compact,
  condensed,
}: {
  compact: boolean;
  condensed: boolean;
}) {
  const runtime = useAgentRuntimeContext();
  const { draft, draftFiles, draftReferences, scopeDealId, attachmentError, selectedModel, sendError, isPreparingThread, isRunning, isThreadLoading } = useThreadRuntimeState(
    (state) => state,
  );
  return (
    <PromptInput
      value={draft}
      references={draftReferences}
      onReferencesChange={runtime.setReferences}
      scopeDealId={scopeDealId}
      onValueChange={runtime.setDraft}
      onSend={() => runtime.send()}
      onStop={runtime.stop}
      generating={isRunning}
      disabled={isPreparingThread || isThreadLoading}
      files={draftFiles}
      onFilesAdd={runtime.addFiles}
      onFileRemove={runtime.removeFile}
      model={selectedModel}
      onModelChange={runtime.setModel}
      error={attachmentError || sendError}
      compact={compact}
      condensed={condensed}
    />
  );
}

function ThreadMessage({ message }: { message: Message }) {
  return message.role === "user" ? (
    <UserMessageView message={message} />
  ) : (
    <AssistantMessageView message={message} />
  );
}

function UserMessageView({ message }: { message: Message & { role: "user" } }) {
  return (
    <div
      className="stealth-message-enter flex justify-end px-1"
      data-role="user"
      data-slot="query-user-message"
    >
      <div className="max-w-[min(80%,42rem)] rounded-2xl rounded-br-sm bg-surface-container px-4 py-2.5 text-[14px] leading-6 text-on-surface wrap-break-word">
        {textOf(message)}
        {message.fileReferences?.map((file) => <span key={`${file.dealId}:${file.fileId}`} className="m-1 inline-block rounded-md bg-file-mention px-2 py-1 text-xs text-on-file-mention">{file.name}</span>)}
        <AttachmentList files={message.attachments} />
      </div>
    </div>
  );
}

function AssistantMessageView({ message }: { message: AssistantMessage }) {
  const runtime = useAgentRuntimeContext();
  const { copiedMessageId, isRunning: threadRunning } = useThreadRuntimeState(
    (state) => state,
  );
  const content = textOf(message);
  const isRunning = message.status.type === "running";
  const failed =
    message.status.type === "incomplete" && message.status.reason === "error";
  const cancelled =
    message.status.type === "incomplete" &&
    message.status.reason === "cancelled";
  return (
    <div
      className="stealth-message-enter relative px-1 text-[14px] leading-6 text-on-surface"
      data-role="assistant"
      data-slot="query-assistant-message"
    >
      {isRunning && !content ? (
        <div
          aria-label="Assistant is typing"
          className="py-2 text-sm text-muted"
          role="status"
        >
          <ShimmerText>Thinking…</ShimmerText>
        </div>
      ) : (
        <StreamingText content={content} streaming={isRunning} />
      )}
      {failed ? (
        <div className="mt-3 rounded-xl border border-error/35 bg-error-container/35 p-3 text-sm text-error">
          {message.error ?? "The assistant could not complete the response. Please try again."}
          <div className="mt-2">
            <Button
              disabled={threadRunning}
              onClick={() => runtime.retry(message.id)}
              size="sm"
              type="button"
              variant="outline"
            >
              Try again
            </Button>
          </div>
        </div>
      ) : null}
      {cancelled ? (
        <p className="mt-2 text-xs text-muted" role="status">
          Response stopped.
        </p>
      ) : null}
      <span aria-live="polite" className="sr-only">
        {message.status.type === "complete"
          ? "Response complete."
          : failed
            ? "Response failed."
            : ""}
      </span>
      {message.status.type === "complete" ? (
        <MessageActions
          copied={copiedMessageId === message.id}
          disabled={threadRunning}
          onCopy={() => void runtime.copy(message.id)}
          onRetry={() => runtime.retry(message.id)}
        />
      ) : null}
    </div>
  );
}

function textOf(message: Message) {
  return message.content
    .map((part) => (part.type === "text" ? part.text : ""))
    .join("");
}

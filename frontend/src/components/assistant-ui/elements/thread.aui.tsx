import {
  ActionBarPrimitive,
  AuiIf,
  ErrorPrimitive,
  MessagePrimitive,
  SuggestionPrimitive,
  ThreadPrimitive,
  useAuiState,
} from "@assistant-ui/react";
import { ArrowDown, Check, Copy, RefreshCw } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import type { FC } from "react";
import { Button } from "../../ui/button";
import { cn } from "../../../lib/utils";
import { ComposerBar } from "./composer-bar";
import { MarkdownText } from "./markdown-text";
import { TypingIndicator } from "./typing-indicator";

type QueryThreadProps = {
  contextTruncated: boolean;
};

// Scoped adaptation of assistant-ui's Thread registry item (2026-09-15).
// Unsupported attachment, voice, model, reasoning, tool, edit, and branch controls are omitted.
export const QueryThread: FC<QueryThreadProps> = ({ contextTruncated }) => {
  const isEmpty = useAuiState((state) => state.thread.isEmpty);
  const shouldReduceMotion = useReducedMotion() ?? false;

  return (
    <ThreadPrimitive.Root className="aui-root flex h-full min-h-0 flex-col">
      <ThreadPrimitive.Viewport
        className="workspace-scrollbar-hidden relative flex min-h-0 flex-1 flex-col overflow-x-hidden overflow-y-auto"
        data-slot="aui_thread-viewport"
        turnAnchor="top"
      >
        <div
          className={cn(
            "mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 pt-4 sm:px-7",
            isEmpty && "justify-center pb-[12vh]",
          )}
        >
          <AuiIf condition={(state) => state.thread.isEmpty}>
            <ThreadWelcome />
          </AuiIf>

          <div className="mb-8 flex flex-col gap-7 empty:hidden" data-slot="query-message-group">
            <ThreadPrimitive.Messages>{() => <ThreadMessage />}</ThreadPrimitive.Messages>
          </div>

          <ThreadPrimitive.ViewportFooter
            className={cn(
              "relative z-10 flex flex-col",
              isEmpty
                ? "gap-3 pb-4 sm:pb-6"
                : "sticky bottom-0 mt-auto gap-2 bg-[var(--theme-workspace-surface-bottom)] py-3",
            )}
            data-slot="aui_thread-viewport-footer"
          >
            <ThreadScrollToBottom />
            {contextTruncated ? (
              <p className="text-center text-xs text-muted" role="status">
                Older conversation context was omitted to fit this request.
              </p>
            ) : null}
            <motion.div
              data-query-composer-host
              layout
              layoutId="query-chat-composer"
              transition={shouldReduceMotion ? { duration: 0 } : { duration: 0.22, ease: "easeOut" }}
            >
              <ComposerBar compact={!isEmpty} />
            </motion.div>
            <AuiIf condition={(state) => state.thread.isEmpty && state.composer.isEmpty}>
              <ThreadSuggestions />
            </AuiIf>
          </ThreadPrimitive.ViewportFooter>
        </div>
      </ThreadPrimitive.Viewport>
    </ThreadPrimitive.Root>
  );
};

const ThreadWelcome: FC = () => (
  <div className="mb-6 text-center">
    <h1 className="text-xl font-semibold tracking-[-0.02em] text-text-main sm:text-2xl">
      How can I help you today?
    </h1>
  </div>
);

const ThreadSuggestions: FC = () => (
  <div className="flex w-full flex-wrap justify-center gap-2 px-2">
    <ThreadPrimitive.Suggestions>
      {() => (
        <SuggestionPrimitive.Trigger send asChild>
          <Button
            className="h-auto max-w-full gap-1.5 rounded-full border-outline-variant/80 bg-transparent px-3.5 py-2 text-xs font-normal text-on-surface-variant shadow-none sm:text-[13px]"
            variant="outline"
          >
            <SuggestionPrimitive.Title />
            <SuggestionPrimitive.Description className="hidden text-muted md:inline" />
          </Button>
        </SuggestionPrimitive.Trigger>
      )}
    </ThreadPrimitive.Suggestions>
  </div>
);

const ThreadScrollToBottom: FC = () => (
  <ThreadPrimitive.ScrollToBottom asChild>
    <Button
      aria-label="Scroll to bottom"
      className="absolute -top-10 self-center rounded-full bg-surface-container-lowest shadow-sm disabled:invisible"
      size="icon-sm"
      title="Scroll to bottom"
      type="button"
      variant="outline"
    >
      <ArrowDown />
    </Button>
  </ThreadPrimitive.ScrollToBottom>
);

const ThreadMessage: FC = () => {
  const role = useAuiState((state) => state.message.role);
  return role === "user" ? <UserMessage /> : <AssistantMessage />;
};

const UserMessage: FC = () => (
  <MessagePrimitive.Root
    className="flex justify-end px-1"
    data-role="user"
    data-slot="query-user-message"
  >
    <div className="max-w-[min(80%,42rem)] rounded-2xl rounded-br-md bg-surface-container px-4 py-2.5 text-[14px] leading-6 text-on-surface wrap-break-word">
      <MessagePrimitive.Parts>
        {({ part }) => part.type === "text" ? part.text : null}
      </MessagePrimitive.Parts>
    </div>
  </MessagePrimitive.Root>
);

const AssistantMessage: FC = () => {
  const status = useAuiState((state) => state.message.status);

  return (
    <MessagePrimitive.Root
      className="relative px-1 text-[14px] leading-6 text-on-surface"
      data-role="assistant"
      data-slot="query-assistant-message"
    >
      <MessagePrimitive.Parts>
        {({ part }) => {
          if (part.type !== "text") return null;
          if (part.status.type === "running" && part.text === "") {
            return <TypingIndicator />;
          }
          return <MarkdownText />;
        }}
      </MessagePrimitive.Parts>
      <MessageError />
      {status?.type === "incomplete" && status.reason === "cancelled" ? (
        <p className="mt-2 text-xs text-muted" role="status">Response stopped.</p>
      ) : null}
      <MessageStatusAnnouncement />
      <AuiIf condition={(state) => state.message.status?.type === "complete"}>
        <AssistantActions />
      </AuiIf>
    </MessagePrimitive.Root>
  );
};

const MessageError: FC = () => (
  <MessagePrimitive.Error>
    <ErrorPrimitive.Root className="mt-3 rounded-xl border border-error/35 bg-error-container/35 p-3 text-sm text-error">
      <ErrorPrimitive.Message />
      <ActionBarPrimitive.Root className="mt-2">
        <ActionBarPrimitive.Reload asChild>
          <Button size="sm" type="button" variant="outline">Try again</Button>
        </ActionBarPrimitive.Reload>
      </ActionBarPrimitive.Root>
    </ErrorPrimitive.Root>
  </MessagePrimitive.Error>
);

const AssistantActions: FC = () => (
  <ActionBarPrimitive.Root className="mt-2 flex gap-1 text-muted" hideWhenRunning>
    <ActionBarPrimitive.Copy asChild>
      <Button aria-label="Copy response" size="icon-xs" title="Copy response" type="button" variant="ghost">
        <AuiIf condition={(state) => state.message.isCopied}><Check /></AuiIf>
        <AuiIf condition={(state) => !state.message.isCopied}><Copy /></AuiIf>
      </Button>
    </ActionBarPrimitive.Copy>
    <ActionBarPrimitive.Reload asChild>
      <Button aria-label="Retry response" size="icon-xs" title="Retry response" type="button" variant="ghost">
        <RefreshCw />
      </Button>
    </ActionBarPrimitive.Reload>
  </ActionBarPrimitive.Root>
);

const MessageStatusAnnouncement: FC = () => {
  const status = useAuiState((state) => state.message.status);
  const message = status?.type === "complete"
    ? "Response complete."
    : status?.type === "incomplete" && status.reason === "error"
      ? "Response failed."
      : "";

  return <span aria-live="polite" className="sr-only">{message}</span>;
};

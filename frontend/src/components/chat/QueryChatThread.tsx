import { ArrowDown, Check, Copy, RefreshCw, Square, ArrowUp } from "lucide-react";
import { motion, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { cn } from "../../lib/utils";
import { Button } from "../ui/button";
import { Skeleton } from "../ui/skeleton";
import type { AssistantMessage, Message } from "./chatModel";
import { MarkdownText } from "./MarkdownText";
import { queryChatSuggestions } from "./queryChatSuggestions";
import { useAgentRuntimeContext, useThreadRuntimeState } from "./AgentRuntime";
import { TypingIndicator } from "./TypingIndicator";

export function QueryChatThread({ contextTruncated }: { contextTruncated: boolean }) {
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
      <div
        className="workspace-scrollbar-hidden relative flex min-h-0 flex-1 flex-col overflow-x-hidden overflow-y-auto"
        data-slot="query-thread-viewport"
        onScroll={(event) => {
          const target = event.currentTarget;
          setIsNearBottom(target.scrollHeight - target.scrollTop - target.clientHeight < 48);
        }}
        ref={viewportRef}
      >
        <div className={cn("mx-auto flex w-full max-w-3xl flex-1 flex-col px-4 pt-4 sm:px-7", isEmpty && "justify-center pb-[12vh]")}>
          {isEmpty ? <ThreadWelcome /> : null}
          <div className="mb-8 flex flex-col gap-7 empty:hidden" data-slot="query-message-group">
            {state.isThreadLoading ? <ThreadHistorySkeleton /> : null}
            {state.messages.map((message) => (
              <ThreadMessage key={message.id} message={message} />
            ))}
          </div>
          <div className={cn("relative z-10 flex flex-col", isEmpty ? "gap-3 pb-4 sm:pb-6" : "sticky bottom-0 mt-auto gap-2 bg-[var(--theme-workspace-surface-bottom)] py-3")}>
            {!isNearBottom && !isEmpty ? (
              <Button aria-label="Scroll to bottom" className="absolute -top-10 self-center rounded-full bg-surface-container-lowest shadow-sm" onClick={() => viewportRef.current?.scrollTo({ top: viewportRef.current.scrollHeight, behavior: shouldReduceMotion ? "auto" : "smooth" })} size="icon-sm" title="Scroll to bottom" type="button" variant="outline"><ArrowDown /></Button>
            ) : null}
            {contextTruncated ? <p className="text-center text-xs text-muted" role="status">Older conversation context was omitted to fit this request.</p> : null}
            <motion.div data-query-composer-host layout layoutId="query-chat-composer" transition={shouldReduceMotion ? { duration: 0 } : { duration: 0.22, ease: "easeOut" }}>
              <ComposerBar compact={!isEmpty} />
            </motion.div>
            {isEmpty && !state.draft ? <ThreadSuggestions /> : null}
          </div>
        </div>
      </div>
    </div>
  );
}

function ThreadWelcome() {
  return <div className="mb-6 text-center"><h1 className="text-xl font-semibold tracking-[-0.02em] text-text-main sm:text-2xl">How can I help you today?</h1></div>;
}

function ThreadHistorySkeleton() {
  return <div aria-label="Loading previous chat" className="space-y-5" data-testid="thread-history-skeleton" role="status"><div className="flex justify-end"><Skeleton aria-hidden="true" className="h-10 w-3/5 rounded-2xl motion-reduce:animate-none" /></div><div aria-hidden="true" className="space-y-2"><Skeleton className="h-4 w-full motion-reduce:animate-none" /><Skeleton className="h-4 w-11/12 motion-reduce:animate-none" /><Skeleton className="h-4 w-3/5 motion-reduce:animate-none" /></div></div>;
}

function ThreadSuggestions() {
  const runtime = useAgentRuntimeContext();
  return <div className="flex w-full flex-wrap justify-center gap-2 px-2">{queryChatSuggestions.map((suggestion) => <Button className="h-auto max-w-full gap-1.5 rounded-full border-outline-variant/80 bg-transparent px-3.5 py-2 text-xs font-normal text-on-surface-variant shadow-none sm:text-[13px]" key={suggestion.title} onClick={() => void runtime.send(suggestion.prompt)} type="button" variant="outline"><span>{suggestion.title}</span><span className="hidden text-muted md:inline">{suggestion.label}</span></Button>)}</div>;
}

function ComposerBar({ compact }: { compact: boolean }) {
  const runtime = useAgentRuntimeContext();
  const { draft, isPreparingThread, isRunning } = useThreadRuntimeState((state) => state);
  return <div className={cn("group/query-composer relative flex w-full flex-col rounded-2xl border border-outline-variant/40 bg-surface-container-lowest p-2 shadow-[0_12px_34px_rgba(7,1,84,0.07)] transition-[border-color,box-shadow,border-radius] focus-within:border-outline/50 focus-within:shadow-[0_14px_38px_rgba(7,1,84,0.1)]", compact && "min-h-14 flex-row items-end rounded-full p-1.5")} data-compact={compact ? "" : undefined} data-slot="query-composer"><textarea aria-label="Message input" autoFocus className={cn("max-h-48 min-h-16 w-full resize-none bg-transparent px-2.5 py-2 text-[15px] leading-6 text-on-surface outline-none placeholder:text-muted", compact && "min-h-11 px-3.5 py-2.5")} enterKeyHint="send" onChange={(event) => runtime.setDraft(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void runtime.send(); } }} placeholder="Send a message..." rows={compact ? 1 : 2} value={draft} /><div className={cn("flex justify-end", compact && "shrink-0 self-center")} data-slot="query-composer-actions">{isRunning ? <Button aria-label="Stop generating" className="rounded-full" onClick={() => runtime.stop()} size="icon-sm" title="Stop generating" type="button"><Square className="fill-current" /></Button> : <Button aria-label="Send message" className="rounded-full" disabled={isPreparingThread || !draft.trim()} onClick={() => void runtime.send()} size="icon-sm" title="Send message" type="button"><ArrowUp /></Button>}</div></div>;
}

function ThreadMessage({ message }: { message: Message }) {
  return message.role === "user"
    ? <UserMessageView message={message} />
    : <AssistantMessageView message={message} />;
}

function UserMessageView({ message }: { message: Message & { role: "user" } }) {
  return <div className="flex justify-end px-1" data-role="user" data-slot="query-user-message"><div className="max-w-[min(80%,42rem)] rounded-2xl rounded-br-md bg-surface-container px-4 py-2.5 text-[14px] leading-6 text-on-surface wrap-break-word">{textOf(message)}</div></div>;
}

function AssistantMessageView({ message }: { message: AssistantMessage }) {
  const runtime = useAgentRuntimeContext();
  const { copiedMessageId } = useThreadRuntimeState((state) => state);
  const content = textOf(message);
  const isRunning = message.status.type === "running";
  const failed = message.status.type === "incomplete" && message.status.reason === "error";
  const cancelled = message.status.type === "incomplete" && message.status.reason === "cancelled";
  return <div className="relative px-1 text-[14px] leading-6 text-on-surface" data-role="assistant" data-slot="query-assistant-message">{isRunning && !content ? <TypingIndicator /> : <MarkdownText content={content} />}{failed ? <div className="mt-3 rounded-xl border border-error/35 bg-error-container/35 p-3 text-sm text-error">The assistant could not complete the response. Please try again.<div className="mt-2"><Button onClick={() => runtime.retry(message.id)} size="sm" type="button" variant="outline">Try again</Button></div></div> : null}{cancelled ? <p className="mt-2 text-xs text-muted" role="status">Response stopped.</p> : null}<span aria-live="polite" className="sr-only">{message.status.type === "complete" ? "Response complete." : failed ? "Response failed." : ""}</span>{message.status.type === "complete" ? <div className="mt-2 flex gap-1 text-muted"><Button aria-label="Copy response" onClick={() => void runtime.copy(message.id)} size="icon-xs" title="Copy response" type="button" variant="ghost">{copiedMessageId === message.id ? <Check /> : <Copy />}</Button><Button aria-label="Retry response" onClick={() => runtime.retry(message.id)} size="icon-xs" title="Retry response" type="button" variant="ghost"><RefreshCw /></Button></div> : null}</div>;
}

function textOf(message: Message) {
  return message.content.map((part) => part.type === "text" ? part.text : "").join("");
}

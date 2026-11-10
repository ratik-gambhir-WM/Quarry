import { isDealDeleted, subscribeDealDeletion } from "../../data/dealDeletion/lifecycle";
import type { AssistantThread, ChatFileReference } from "../../contracts/quarryApi";
import type { ChatModelSelection, Message } from "./chatModel";

export type ThreadRuntimeState = {
  nextCursor: string | null;
  scopeDealId: string | null;
  copiedMessageId: string | null;
  deletingThreadId: string | null;
  draft: string;
  draftFiles: readonly File[];
  draftReferences: readonly ChatFileReference[];
  attachmentError: string | null;
  selectedModel: ChatModelSelection;
  sendError: string | null;
  isPreparingThread: boolean;
  isRunning: boolean;
  isThreadListLoading: boolean;
  isThreadLoading: boolean;
  messages: readonly Message[];
  selectedThreadId: string | null;
  threadDeleteError: boolean;
  threadLoadError: boolean;
  threadListError: boolean;
  threads: readonly AssistantThread[];
};

const clearedConversation = {
  selectedThreadId: null,
  messages: [],
  draft: "",
  draftFiles: [],
  draftReferences: [],
  copiedMessageId: null,
  attachmentError: null,
  sendError: null,
  isRunning: false,
  isThreadLoading: false,
  isPreparingThread: false,
} satisfies Partial<ThreadRuntimeState>;

/** Each keyed chat provider owns a store; construction and snapshot reads never subscribe. */
export function createThreadRuntimeStore(initial: ThreadRuntimeState, invalidatePendingWork: () => void) {
  const visibleState = (next: ThreadRuntimeState): ThreadRuntimeState => {
    const selected = next.threads.find((thread) => thread.threadId === next.selectedThreadId);
    const threads = next.threads.filter((thread) => !thread.dealId || !isDealDeleted(thread.dealId));
    const deletedSelection = (next.scopeDealId && isDealDeleted(next.scopeDealId))
      || (selected?.dealId && isDealDeleted(selected.dealId));
    return { ...next, ...(deletedSelection ? clearedConversation : {}), threads };
  };
  let state = visibleState(initial);
  let stopDeletion: (() => void) | undefined;
  const listeners = new Set<() => void>();
  const update = (change: Partial<ThreadRuntimeState>) => {
    state = visibleState({ ...state, ...change });
    for (const listener of listeners) listener();
  };
  const deleteDeal = (id: string) => {
    const selected = state.threads.find((thread) => thread.threadId === state.selectedThreadId);
    if (state.scopeDealId === id || selected?.dealId === id) {
      invalidatePendingWork();
      update(clearedConversation);
    } else if (state.threads.some((thread) => thread.dealId === id)) {
      update({ threads: state.threads.filter((thread) => thread.dealId !== id) });
    }
  };
  return {
    get current() { return state; },
    getSnapshot: () => state,
    update,
    subscribe(listener: () => void) {
      listeners.add(listener);
      if (listeners.size === 1) {
        stopDeletion = subscribeDealDeletion(deleteDeal);
        // Close the render-to-subscribe gap, including a Strict Mode resubscription.
        const deals = new Set(state.threads.flatMap((thread) => thread.dealId ? [thread.dealId] : []));
        if (state.scopeDealId) deals.add(state.scopeDealId);
        for (const id of deals) if (isDealDeleted(id)) deleteDeal(id);
      }
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) { stopDeletion?.(); stopDeletion = undefined; }
      };
    },
  };
}

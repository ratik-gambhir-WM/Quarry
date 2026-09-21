import {
  AssistantRuntimeProvider,
  AuiConfig,
  Suggestions,
  useLocalRuntime,
  useRemoteThreadListRuntime,
} from "@assistant-ui/react";
import type { ReactNode } from "react";
import { useMemo } from "react";
import { runtime } from "@quarry/runtime";
import { createQueryModelAdapter } from "./queryModelAdapter";
import { queryChatSuggestions } from "./queryChatSuggestions";
import { createAssistantThreadListAdapter } from "./assistantThreadRuntime";

const queryChatConfig = AuiConfig({
  suggestions: Suggestions(queryChatSuggestions),
});

type QueryChatRuntimeProviderProps = {
  children: ReactNode;
  onContextTruncated: (truncated: boolean) => void;
  userEmail: string;
};

export function QueryChatRuntimeProvider({
  children,
  onContextTruncated,
  userEmail,
}: QueryChatRuntimeProviderProps) {
  return userEmail ? (
    <PersistentQueryChatRuntimeProvider
      onContextTruncated={onContextTruncated}
      userEmail={userEmail}
    >
      {children}
    </PersistentQueryChatRuntimeProvider>
  ) : (
    <LocalQueryChatRuntimeProvider onContextTruncated={onContextTruncated}>
      {children}
    </LocalQueryChatRuntimeProvider>
  );
}

function LocalQueryChatRuntimeProvider({
  children,
  onContextTruncated,
}: Omit<QueryChatRuntimeProviderProps, "userEmail">) {
  const adapter = useMemo(
    () => createQueryModelAdapter(runtime.api, { onContextTruncated }),
    [onContextTruncated],
  );
  const assistantRuntime = useLocalRuntime(adapter);

  return (
    <AssistantRuntimeProvider config={queryChatConfig} runtime={assistantRuntime}>
      {children}
    </AssistantRuntimeProvider>
  );
}

function PersistentQueryChatRuntimeProvider({
  children,
  onContextTruncated,
  userEmail,
}: QueryChatRuntimeProviderProps) {
  const adapter = useMemo(
    () => createQueryModelAdapter(runtime.api, { onContextTruncated, userEmail }),
    [onContextTruncated, userEmail],
  );
  const threadListAdapter = useMemo(
    () => createAssistantThreadListAdapter(runtime.api, userEmail),
    [userEmail],
  );
  const assistantRuntime = useRemoteThreadListRuntime({
    adapter: threadListAdapter,
    runtimeHook: function useAssistantThreadRuntime() {
      return useLocalRuntime(adapter);
    },
  });

  return (
    <AssistantRuntimeProvider config={queryChatConfig} runtime={assistantRuntime}>
      {children}
    </AssistantRuntimeProvider>
  );
}

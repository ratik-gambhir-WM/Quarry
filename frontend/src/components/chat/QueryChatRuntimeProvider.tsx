import {
  AssistantRuntimeProvider,
  AuiConfig,
  Suggestions,
  useLocalRuntime,
} from "@assistant-ui/react";
import type { ReactNode } from "react";
import { useMemo } from "react";
import { runtime } from "@quarry/runtime";
import { createQueryModelAdapter } from "./queryModelAdapter";
import { queryChatSuggestions } from "./queryChatSuggestions";

const queryChatConfig = AuiConfig({
  suggestions: Suggestions(queryChatSuggestions),
});

type QueryChatRuntimeProviderProps = {
  children: ReactNode;
  onContextTruncated: (truncated: boolean) => void;
};

export function QueryChatRuntimeProvider({
  children,
  onContextTruncated,
}: QueryChatRuntimeProviderProps) {
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

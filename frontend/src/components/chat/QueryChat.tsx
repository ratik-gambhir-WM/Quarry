import { useState } from "react";
import { QueryChatRuntimeProvider } from "./AgentRuntime";
import { QueryChatThread as Thread } from "./QueryChatThread";

export function QueryChat() {
  const [contextTruncated, setContextTruncated] = useState(false);

  return (
    <QueryChatRuntimeProvider onContextTruncated={setContextTruncated} userEmail="">
      <QueryChatThread contextTruncated={contextTruncated} />
    </QueryChatRuntimeProvider>
  );
}

export function QueryChatThread({ contextTruncated }: { contextTruncated: boolean }) {
  return <Thread contextTruncated={contextTruncated} />;
}

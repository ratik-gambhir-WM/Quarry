import { useState } from "react";
import { QueryThread } from "../assistant-ui/elements/thread.aui";
import { QueryChatRuntimeProvider } from "./QueryChatRuntimeProvider";

export function QueryChat() {
  const [contextTruncated, setContextTruncated] = useState(false);

  return (
    <QueryChatRuntimeProvider onContextTruncated={setContextTruncated}>
      <QueryChatThread contextTruncated={contextTruncated} />
    </QueryChatRuntimeProvider>
  );
}

export function QueryChatThread({ contextTruncated }: { contextTruncated: boolean }) {
  return <QueryThread contextTruncated={contextTruncated} />;
}

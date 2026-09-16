import { MarkdownTextPrimitive } from "@assistant-ui/react-markdown";
import { memo } from "react";
import remarkGfm from "remark-gfm";

// Scoped adaptation of assistant-ui's markdown-text registry item (2026-09-15).
// react-markdown keeps raw HTML disabled unless rehype-raw is explicitly installed.
export const MarkdownText = memo(function MarkdownText() {
  return (
    <MarkdownTextPrimitive
      className="query-chat-markdown"
      remarkPlugins={[remarkGfm]}
      smooth={false}
    />
  );
});

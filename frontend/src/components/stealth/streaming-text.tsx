// Presentation adapted from https://stealth.pm/ui/streaming-text (MIT; see ./LICENSE).
// Retain Quarry's GFM renderer and authoritative final response rather than a second parser.
import { MarkdownText } from "../chat/MarkdownText";

export function StreamingText({ content, streaming }: { content: string; streaming: boolean }) {
  return (
    <div className="stealth-streaming-text" data-streaming={streaming || undefined} aria-busy={streaming}>
      <MarkdownText content={content} />
      {streaming ? <span aria-hidden className="stealth-stream-cursor mt-1 inline-block h-3 w-1 rounded-full bg-primary/60" /> : null}
    </div>
  );
}

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { runtime } from "@quarry/runtime";
import { Icon } from "../ui/Icon";

export function SummaryPanel({ onError, summary }: { onError: (message: string) => void; summary: string }) {
  async function handleSaveSummary() {
    onError("");
    try {
      await runtime.platform.saveFile({
        contents: summary,
        extensions: ["md", "markdown"],
        mimeType: "text/markdown;charset=utf-8",
        suggestedName: "summary.md",
        title: "Save markdown summary",
      });
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <div className="vault-markdown relative rounded-[19px] border border-white/85 bg-white/76 p-8 pr-16 text-[16px] leading-7 text-text-main shadow-[0_12px_34px_rgba(7,1,84,0.05)]">
      <button
        aria-label="Save markdown summary"
        className="absolute right-5 top-5 flex h-10 w-10 items-center justify-center rounded-full text-primary transition hover:bg-primary/8"
        onClick={() => void handleSaveSummary()}
        title="Save markdown summary"
        type="button"
      >
        <Icon className="h-5 w-5" name="bookmark" />
      </button>
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{summary}</ReactMarkdown>
    </div>
  );
}

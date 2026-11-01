import { useRef, useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowUpRight, Sparkles } from "lucide-react";
import { Button } from "../../ui/button";

type AiSearchCardProps = {
  suggestions: string[];
};

export function AiSearchCard({ suggestions }: AiSearchCardProps) {
  const [draft, setDraft] = useState("");
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const navigate = useNavigate();

  function openAssistant(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!draft.trim()) return;
    navigate("/hub/assistant", { state: { assistantDraft: draft.trim() } });
  }

  return (
    <section aria-labelledby="home-assistant-title" className="flex flex-col gap-4">
      <div className="flex items-center gap-2">
        <Sparkles aria-hidden="true" className="size-4 text-muted" />
        <h2 id="home-assistant-title" className="text-[15px] font-medium text-on-surface">Ask your assistant</h2>
      </div>
      <form onSubmit={openAssistant} className="overflow-hidden rounded-xl border border-outline-variant bg-surface-container-lowest shadow-sm focus-within:border-primary">
        <label htmlFor="home-assistant-draft" className="sr-only">Your question</label>
        <textarea
          id="home-assistant-draft"
          ref={inputRef}
          value={draft}
          onChange={(event) => setDraft(event.currentTarget.value)}
          placeholder="What would you like to work on?"
          className="block min-h-28 w-full resize-y bg-transparent px-4 py-4 text-[16px] leading-6 text-on-surface placeholder:text-muted focus-visible:outline-offset-[-3px]"
          rows={3}
          aria-describedby="home-assistant-hint"
        />
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-outline-variant/60 px-4 py-3">
          <p id="home-assistant-hint" className="text-[12px] leading-5 text-muted">Review your draft in Assistant before sending.</p>
          <Button type="submit" disabled={!draft.trim()} className="h-10 gap-2 px-4">
            Open Assistant <ArrowUpRight aria-hidden="true" className="size-4" />
          </Button>
        </div>
      </form>
      <div className="flex flex-wrap gap-2" aria-label="Suggested questions">
        {suggestions.map((suggestion) => (
          <button
            className="min-h-10 rounded-lg border border-outline-variant px-3 py-2 text-left text-[13px] leading-5 text-on-surface-variant transition-colors hover:bg-surface-container-high"
            key={suggestion}
            type="button"
            onClick={() => {
              setDraft(suggestion);
              inputRef.current?.focus();
            }}
          >
            {suggestion}
          </button>
        ))}
      </div>
    </section>
  );
}

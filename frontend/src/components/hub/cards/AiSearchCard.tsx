import { Icon } from "../../ui/Icon";

type AiSearchCardProps = {
  suggestions: string[];
};

export function AiSearchCard({ suggestions }: AiSearchCardProps) {
  return (
    <section className="col-span-12 px-1 py-2 sm:px-3 lg:col-span-8">
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary text-white">
            <Icon className="h-4 w-4" name="sparkles" />
          </div>
          <h2 className="type-h3 text-text-main">Synthesis AI</h2>
        </div>
        <span className="text-[11px] font-bold uppercase tracking-[0.18em] text-muted">Unified Search</span>
      </div>

      <div className="relative">
        <div className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted">
          <Icon className="h-6 w-6" name="search" />
        </div>
        <input
          className="h-16 w-full rounded-[1.35rem] border border-white/80 bg-white/55 pl-14 pr-40 text-[15px] font-medium text-text-main shadow-sm outline-none transition placeholder:text-muted/70 focus:border-primary/25 focus:bg-white focus:ring-4 focus:ring-primary/10"
          placeholder="Search across deals or ask AI a question..."
          type="text"
        />
        <button className="absolute right-2 top-1/2 inline-flex h-12 -translate-y-1/2 items-center gap-2 rounded-2xl bg-primary px-6 text-sm font-semibold text-white transition hover:bg-primary-container">
          <Icon className="h-4 w-4" name="send" />
          Ask AI
        </button>
      </div>

      <div className="mt-4 flex flex-wrap items-center gap-4">
        <span className="text-[11px] font-bold uppercase tracking-[0.16em] text-muted">Suggested:</span>
        {suggestions.map((suggestion) => (
          <button className="text-[11px] font-bold text-primary transition hover:underline" key={suggestion}>
            {suggestion}
          </button>
        ))}
      </div>
    </section>
  );
}

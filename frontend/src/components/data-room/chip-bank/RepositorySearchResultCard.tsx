import { Icon } from "../../ui/Icon";
import { HighlightedText } from "./HighlightedText";
import { formatRepositoryFileName, RepositoryResult, SearchMode } from "./repositorySearch";

type RepositorySearchResultCardProps = {
  mode: SearchMode;
  query: string;
  result: RepositoryResult;
};

export function RepositorySearchResultCard({ mode, query, result }: RepositorySearchResultCardProps) {
  const score = mode === "semantic" ? result.semanticScore : result.keywordScore;

  return (
    <article className="border border-outline-variant bg-surface-container-lowest p-4 transition hover:border-primary/35">
      <div className="flex items-start gap-3">
        <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-surface-container text-primary">
          <Icon className="h-4.5 w-4.5" name={fileTypeIcon(result.fileName)} />
        </div>
        <div className="min-w-0 flex-1">
          <h4 className="truncate text-[15px] font-bold leading-5 text-text-main" title={result.fileName}>
            {formatRepositoryFileName(result.fileName)}
          </h4>
          <p className="mt-1 text-[11px] font-semibold uppercase tracking-[0.08em] text-muted">
            {result.category} · {result.date} · {result.location}
          </p>
        </div>
        <span className={`shrink-0 rounded-sm px-2 py-1 text-[11px] font-bold ${scoreClasses(score)}`}>{score}%</span>
      </div>

      <p className="mt-3 text-[14px] leading-5 text-text-main/78">
        “
        {mode === "keyword" ? <HighlightedText query={query} text={result.excerpt} /> : result.excerpt}
        ”
      </p>
    </article>
  );
}

function fileTypeIcon(fileName: string): "doc" | "pdf" | "sheet" {
  if (fileName.endsWith(".docx")) {
    return "doc";
  }

  if (fileName.endsWith(".xlsx")) {
    return "sheet";
  }

  return "pdf";
}

function scoreClasses(score: number) {
  if (score >= 90) {
    return "bg-[#e5f5ed] text-[#237a52]";
  }

  if (score >= 75) {
    return "bg-primary/10 text-primary";
  }

  return "bg-surface-container-high text-muted";
}

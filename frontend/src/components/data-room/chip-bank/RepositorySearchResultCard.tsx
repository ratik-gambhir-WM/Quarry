import { Icon } from "../../ui/Icon";
import { HighlightedText } from "./HighlightedText";
import { formatRepositoryFileName, type RepositoryResult, type SearchMode } from "./repositorySearch";

type RepositorySearchResultCardProps = {
  mode: SearchMode;
  query: string;
  result: RepositoryResult;
};

export function RepositorySearchResultCard({ mode, query, result }: RepositorySearchResultCardProps) {
  const score = mode === "semantic" ? result.semanticScore : result.keywordScore;

  return (
    <article className="rounded-lg border border-outline-variant/70 bg-surface-container-lowest p-3 transition hover:border-outline">
      <div className="flex items-start gap-3">
        <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-sidebar-hover text-sidebar-muted">
          <Icon className="h-4.5 w-4.5" name={fileTypeIcon(result.fileName)} />
        </div>
        <div className="min-w-0 flex-1">
          <h4 className="truncate text-[13px] font-semibold leading-5 text-sidebar-active" title={result.fileName}>
            {formatRepositoryFileName(result.fileName)}
          </h4>
          <p className="mt-0.5 text-[10px] font-semibold uppercase tracking-[0.08em] text-sidebar-muted">
            {result.category} · {result.date} · {result.location}
          </p>
        </div>
        <span className={`shrink-0 rounded-sm px-2 py-1 text-[11px] font-bold ${scoreClasses(score)}`}>{score}%</span>
      </div>

      <p className="mt-3 text-[12px] leading-5 text-sidebar-text">
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
    return "bg-sidebar-selected text-sidebar-active";
  }

  return "bg-sidebar-hover text-sidebar-muted";
}

import { useState } from "react";
import { Accordion } from "radix-ui";
import { ChevronDown } from "lucide-react";
import { Badge } from "../ui/badge";
import { SearchField } from "../ui/search-field";
import "./deal-scope.css";

export function DealScopeView({ questions }: { questions: readonly string[] }) {
  const [query, setQuery] = useState("");
  const search = query.trim().toLocaleLowerCase();
  const visibleQuestions = questions
    .map((question, index) => ({ question, index }))
    .filter(({ question }) => question.toLocaleLowerCase().includes(search));
  const count = search
    ? `${visibleQuestions.length} of ${questions.length} questions`
    : `${questions.length} ${questions.length === 1 ? "question" : "questions"}`;

  return (
    <section aria-labelledby="key-questions-heading" className="deal-scope flex min-h-0 min-w-0 flex-1 flex-col">
      <header className="flex shrink-0 flex-wrap items-end justify-between gap-4 border-b border-outline-variant/70 px-5 py-5 sm:px-8 sm:py-6">
        <div>
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="type-h3 text-text-main" id="key-questions-heading">Key Questions</h1>
            <Badge variant="outline" role="status">{count}</Badge>
          </div>
          <p className="mt-1.5 text-xs text-muted">Questions extracted from submitted SOW</p>
        </div>
        <SearchField
          aria-label="Search questions"
          placeholder="Search questions"
          clearLabel="Clear question search"
          containerClassName="w-full sm:w-64"
          disabled={questions.length === 0}
          value={query}
          onValueChange={setQuery}
        />
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-3 py-4 sm:px-6 sm:py-5">
        {questions.length === 0 ? (
          <p className="px-3 py-8 text-sm text-muted">No key questions were extracted from the submitted source files.</p>
        ) : visibleQuestions.length === 0 ? (
          <p className="px-3 py-8 text-sm text-muted">No questions match your search. Try another term or clear the search.</p>
        ) : null}
        <Accordion.Root type="multiple" defaultValue={["question-0"]} asChild>
          <ol aria-label="Key questions" className="grid gap-2">
            {visibleQuestions.map(({ question, index }) => (
              <Accordion.Item key={index} value={`question-${index}`} asChild>
                <li className="scope-question min-w-0 rounded-lg border border-transparent border-b-outline-variant/50 data-[state=open]:border-outline-variant/60 data-[state=open]:bg-surface-container-low">
                  <Accordion.Header asChild>
                    <h2>
                      <Accordion.Trigger className="scope-question-trigger group grid w-full grid-cols-[2rem_minmax(0,1fr)_1rem] items-start gap-3 rounded-lg px-3 py-4 text-left outline-none focus-visible:ring-2 focus-visible:ring-primary sm:px-4">
                        <span aria-hidden="true" className="pt-0.5 text-xs tabular-nums text-muted">{String(index + 1).padStart(2, "0")}</span>
                        <span className="max-w-[100ch] min-w-0 break-words text-sm leading-6 text-text-main sm:text-[15px]">{question}</span>
                        <ChevronDown aria-hidden="true" className="scope-question-chevron mt-1 size-4 shrink-0 text-muted group-data-[state=open]:rotate-180" />
                      </Accordion.Trigger>
                    </h2>
                  </Accordion.Header>
                  <Accordion.Content className="scope-question-body overflow-hidden px-3 sm:px-4">
                    <div className="ml-11 border-t border-outline-variant/60 pb-5 pt-4">
                      <p className="text-xs font-medium text-text-main">Answer</p>
                      <p className="mt-2 text-sm leading-6 text-muted">No answer yet.</p>
                    </div>
                  </Accordion.Content>
                </li>
              </Accordion.Item>
            ))}
          </ol>
        </Accordion.Root>
      </div>
    </section>
  );
}

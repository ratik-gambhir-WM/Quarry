import { Icon } from "../../ui/Icon";
import { ANY_TIME, type SearchMode } from "./repositorySearch";

type RepositorySearchControlsProps = {
  categories: string[];
  category: string;
  date: string;
  mode: SearchMode;
  onCategoryChange: (category: string) => void;
  onDateChange: (date: string) => void;
  onModeChange: (mode: SearchMode) => void;
  onQueryChange: (query: string) => void;
  query: string;
};

const dateOptions = [ANY_TIME, "2024", "2023", "2022"];
const searchModes: SearchMode[] = ["semantic", "keyword"];

export function RepositorySearchControls({
  categories,
  category,
  date,
  mode,
  onCategoryChange,
  onDateChange,
  onModeChange,
  onQueryChange,
  query,
}: RepositorySearchControlsProps) {
  return (
    <>
      <label className="relative block">
        <span className="sr-only">Search repository</span>
        <Icon className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-sidebar-muted" name="search" />
        <input
          className="h-9 w-full rounded-lg border border-outline-variant/70 bg-background px-3 pl-9 text-[13px] text-sidebar-active outline-none transition placeholder:text-sidebar-muted focus:border-outline focus:ring-2 focus:ring-primary-fixed"
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Search files and excerpts..."
          type="search"
          value={query}
        />
      </label>

      <div aria-label="Search mode" className="mt-2 grid grid-cols-2 rounded-lg bg-sidebar-hover p-1">
        {searchModes.map((option) => (
          <button
            aria-pressed={mode === option}
            className={`h-8 rounded-md text-[12px] font-medium capitalize transition ${
              mode === option
                ? "bg-background text-sidebar-active shadow-sm"
                : "text-sidebar-muted hover:text-sidebar-active"
            }`}
            key={option}
            onClick={() => onModeChange(option)}
            type="button"
          >
            {option}
          </button>
        ))}
      </div>

      <div className="mt-2 grid grid-cols-2 gap-2">
        <RepositoryFilterSelect
          label="Filter by category"
          onChange={onCategoryChange}
          options={categories}
          value={category}
        />
        <RepositoryFilterSelect label="Filter by date" onChange={onDateChange} options={dateOptions} value={date} />
      </div>
    </>
  );
}

type RepositoryFilterSelectProps = {
  label: string;
  onChange: (value: string) => void;
  options: string[];
  value: string;
};

export function RepositoryFilterSelect({ label, onChange, options, value }: RepositoryFilterSelectProps) {
  return (
    <label className="relative">
      <span className="sr-only">{label}</span>
      <select
        className="h-9 w-full appearance-none rounded-lg border border-outline-variant/70 bg-background px-3 pr-8 text-[12px] font-medium text-sidebar-active outline-none transition focus:border-outline focus:ring-2 focus:ring-primary-fixed"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
      <Icon className="pointer-events-none absolute right-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-sidebar-muted" name="chevronDown" />
    </label>
  );
}

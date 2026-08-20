import { Icon } from "../../ui/Icon";
import { ANY_TIME, SearchMode } from "./repositorySearch";

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
        <Icon className="absolute left-3 top-1/2 h-5 w-5 -translate-y-1/2 text-muted" name="search" />
        <input
          className="h-11 w-full rounded-md border border-outline-variant bg-surface-container-low px-4 pl-10 text-[14px] text-text-main outline-none transition placeholder:text-muted/70 focus:border-primary focus:bg-surface-container-lowest focus:ring-2 focus:ring-primary/10"
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Search files and excerpts..."
          type="search"
          value={query}
        />
      </label>

      <div aria-label="Search mode" className="mt-3 grid grid-cols-2 rounded-md border border-outline-variant bg-surface-container-high p-1">
        {searchModes.map((option) => (
          <button
            aria-pressed={mode === option}
            className={`h-8 rounded-sm text-[12px] font-semibold capitalize transition ${
              mode === option
                ? "bg-surface-container-lowest text-text-main shadow-sm"
                : "text-muted hover:text-text-main"
            }`}
            key={option}
            onClick={() => onModeChange(option)}
            type="button"
          >
            {option}
          </button>
        ))}
      </div>

      <div className="mt-3 grid grid-cols-2 gap-2">
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
        className="h-9 w-full appearance-none rounded-md border border-outline-variant bg-surface-container-lowest px-3 pr-8 text-[12px] font-semibold text-text-main outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/10"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {option}
          </option>
        ))}
      </select>
      <Icon className="pointer-events-none absolute right-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted" name="chevronDown" />
    </label>
  );
}

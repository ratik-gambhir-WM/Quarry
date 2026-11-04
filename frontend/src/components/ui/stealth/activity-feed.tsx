// Adapted from https://www.stealth.pm/ui/activity-feed.
// Quarry supplies theme tokens, Lucide icons, and externally owned pagination.
import { useId, useSyncExternalStore } from "react";
import { motion, useReducedMotion } from "motion/react";
import type { ReactNode, ComponentProps } from "react";
import { cn } from "@/lib/utils";
import { Button } from "../button";
import { dur, ease } from "../stealth-date/motion";

export type ActivityItem = {
  id: string;
  actor: { name: string };
  action: string;
  at: string;
  icon?: ReactNode;
};

export type ActivityFeedProps = Omit<ComponentProps<"section">, "children"> & {
  items: ActivityItem[];
  loading?: boolean;
  loadingMore?: boolean;
  hasMore?: boolean;
  onLoadMore?: () => void;
};

let clock = 0;
let ticker: number | undefined;
const listeners = new Set<() => void>();
const tick = () => {
  if (document.hidden) return;
  clock = Date.now();
  listeners.forEach((l) => l());
};
function subscribeClock(cb: () => void) {
  listeners.add(cb);
  if (ticker === undefined) {
    ticker = window.setInterval(tick, 30_000);
    document.addEventListener("visibilitychange", tick);
  }
  return () => {
    listeners.delete(cb);
    if (!listeners.size) {
      window.clearInterval(ticker);
      ticker = undefined;
      document.removeEventListener("visibilitychange", tick);
    }
  };
}
const readClock = () => clock || (clock = Date.now());
const useNow = () => useSyncExternalStore(subscribeClock, readClock, () => 0);

const toMs = (date: string) => Date.parse(date);
const dayStart = (ms: number) => new Date(ms).setHours(0, 0, 0, 0);
const fmtWeekday = new Intl.DateTimeFormat(undefined, { weekday: "long" });
const fmtDay = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short" });
const fmtDayYear = new Intl.DateTimeFormat(undefined, {
  day: "numeric", month: "short", year: "numeric",
});
const fmtTime = new Intl.DateTimeFormat(undefined, {
  hour: "numeric", minute: "2-digit",
});

function groupLabel(dayMs: number, now: number) {
  const days = Math.round((dayStart(now) - dayMs) / 864e5);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days > 1 && days < 7) return fmtWeekday.format(dayMs);
  return new Date(dayMs).getFullYear() === new Date(now).getFullYear()
    ? fmtDay.format(dayMs)
    : fmtDayYear.format(dayMs);
}

/** Recent times read as a distance ("4m"); older ones as the clock time, since the day is in the header. */
function shortTime(ms: number, now: number) {
  if (!now || ms > now) return fmtTime.format(ms);
  const s = Math.max(0, (now - ms) / 1000);
  if (s < 45) return "now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m}m`;
  const h = Math.round(m / 60);
  if (h < 12 && dayStart(ms) === dayStart(now)) return `${h}h`;
  return fmtTime.format(ms);
}

export function ActivityFeed({
  items,
  loading = false,
  loadingMore = false,
  hasMore = false,
  onLoadMore,
  className,
  ...rest
}: ActivityFeedProps) {
  const now = useNow();
  const uid = useId();
  const reduce = useReducedMotion();
  const groups: { key: string; label: string; items: ActivityItem[] }[] = [];
  for (const item of items) {
    const day = dayStart(toMs(item.at));
    const key = String(day);
    const last = groups[groups.length - 1];
    if (last?.key === key) last.items.push(item);
    else groups.push({ key, label: groupLabel(day, now), items: [item] });
  }

  return (
    <section
      aria-label="Activity feed"
      aria-busy={loading || loadingMore || undefined}
      className={cn(
        "relative min-w-0 [--feed-bg:var(--theme-surface-base)]",
        className,
      )}
      {...rest}
    >
      {loading && items.length === 0 ? (
        <>
          <span role="status" className="sr-only">
            Loading activity…
          </span>
          <Skeleton />
        </>
      ) : items.length === 0 ? (
        <div role="status" className="py-10 text-center">
          <p className="text-sm font-medium text-on-surface">No activity yet.</p>
          <p className="mt-1 text-sm text-muted">Deal updates and uploads will appear here.</p>
        </div>
      ) : (
        <>
          {groups.map((group) => (
            <div key={group.key} role="group" aria-labelledby={`${uid}-${group.key}`}>
              <h2 id={`${uid}-${group.key}`} className="flex h-10 items-center pl-10 text-xs font-medium text-muted">
                {group.label}
              </h2>
              <ol className="flex flex-col">
                {group.items.map((item, index) => (
                  <motion.li
                    key={item.id}
                    initial={reduce ? false : { opacity: 0, y: 4 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: reduce ? 0 : dur.menu, ease: ease.out }}
                    className="relative"
                  >
                    {index > 0 && (
                      <span aria-hidden className="pointer-events-none absolute left-[13.5px] top-0 h-5 w-px bg-border" />
                    )}
                    {index < group.items.length - 1 && (
                      <span aria-hidden className="pointer-events-none absolute bottom-0 left-[13.5px] top-5 w-px bg-border" />
                    )}
                    <div className="relative grid grid-cols-[28px_minmax(0,1fr)] items-start gap-x-3 py-2 sm:grid-cols-[28px_minmax(0,1fr)_auto]">
                      <span aria-hidden className="relative grid size-7 place-items-center rounded-full border border-border bg-[var(--feed-bg)] text-muted [&_svg]:size-3.5">
                        {item.icon ?? (
                          <span className="text-[10px]">
                            {item.actor.name.split(/\s+/).slice(0, 2).map((word) => word[0]).join("")}
                          </span>
                        )}
                      </span>
                      <p className="min-w-0 pt-1 text-sm leading-5 text-on-surface-variant [overflow-wrap:anywhere]">
                        <span className="font-medium text-on-surface">{item.actor.name}</span>
                        {" · "}{item.action}
                      </p>
                      <time
                        dateTime={item.at}
                        title={new Date(item.at).toLocaleString()}
                        aria-label={new Date(item.at).toLocaleString()}
                        className="col-start-2 whitespace-nowrap pt-1 text-xs leading-5 text-muted tabular-nums sm:col-start-3"
                      >
                        {shortTime(toMs(item.at), now)}
                      </time>
                    </div>
                  </motion.li>
                ))}
              </ol>
            </div>
          ))}
          {hasMore && onLoadMore && (
            <div className="pl-10 pt-3">
              <Button
                variant="ghost"
                disabled={loadingMore}
                aria-busy={loadingMore || undefined}
                onClick={onLoadMore}
              >
                {loadingMore ? "Loading…" : "Load more activity"}
              </Button>
            </div>
          )}
        </>
      )}
    </section>
  );
}

function Skeleton() {
  return (
    <div aria-hidden className="flex flex-col gap-1 py-1">
      <span className="ml-10 h-8 w-16 py-3">
        <span className="block h-2 w-full rounded-full bg-surface-container-high motion-safe:animate-pulse" />
      </span>
      {[72, 88, 60, 80].map((w, i) => (
        <div key={i} className="grid grid-cols-[28px_minmax(0,1fr)_auto] items-center gap-x-3 py-1.5">
          <span className="size-7 rounded-full bg-surface-container-high motion-safe:animate-pulse" />
          <span className="h-2.5 rounded-full bg-surface-container-high motion-safe:animate-pulse" style={{ width: `${w}%` }} />
          <span className="h-2.5 w-6 rounded-full bg-surface-container-high motion-safe:animate-pulse" />
        </div>
      ))}
    </div>
  );
}

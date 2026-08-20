import { FormEvent, PointerEvent, useEffect, useMemo, useRef, useState } from "react";
import { DealRoomData, DealTask, DealTimelineItem, DealTimelineTone } from "../../data/workspace";
import {
  moveReminder,
  ReminderStatus,
  toWorkflowReminder,
  WorkflowReminder,
} from "../../data/reminderWorkflow";
import { Icon } from "../ui/Icon";
import { ActivityTimelineCard } from "./ActivityTimelineCard";

type DealTimelineViewProps = {
  deal: DealRoomData;
  events: DealTimelineItem[];
  onEventsChange: (events: DealTimelineItem[]) => void;
};

type TimelineCategory = "Deliverable" | "Key Activity" | "Key Meeting / Call";

type TimelineFormState = {
  category: TimelineCategory;
  date: string;
  detail: string;
  time: string;
  title: string;
};

type CalendarDay = {
  date: Date;
  dateKey: string;
  dayLabel: string;
};

const categoryOptions: TimelineCategory[] = ["Key Meeting / Call", "Key Activity", "Deliverable"];
const weekdays = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"];

const initialFormState: TimelineFormState = {
  category: "Key Meeting / Call",
  date: "",
  detail: "",
  time: "09:00",
  title: "",
};

const toneByCategory: Record<TimelineCategory, DealTimelineTone> = {
  Deliverable: "accent",
  "Key Activity": "muted",
  "Key Meeting / Call": "primary",
};

const categoryStyles: Record<TimelineCategory, { bar: string; legend: string }> = {
  Deliverable: {
    bar: "bg-[#0055ff] text-white",
    legend: "bg-[#0055ff]",
  },
  "Key Activity": {
    bar: "bg-secondary-container text-on-secondary-container",
    legend: "bg-secondary-container",
  },
  "Key Meeting / Call": {
    bar: "bg-primary text-white",
    legend: "bg-primary",
  },
};

const calendarWeeks = createCalendarWeeks("2026-09-28", 5);

export function DealTimelineView({ deal, events, onEventsChange }: DealTimelineViewProps) {
  const [formState, setFormState] = useState<TimelineFormState>(initialFormState);
  const [editingEventId, setEditingEventId] = useState<string | null>(null);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [toastMessage, setToastMessage] = useState("");

  const eventsByDate = useMemo(() => {
    return events.reduce<Record<string, DealTimelineItem[]>>((groupedEvents, item) => {
      const dateKey = getCalendarDateKey(item.date);
      groupedEvents[dateKey] = [...(groupedEvents[dateKey] ?? []), item];
      return groupedEvents;
    }, {});
  }, [events]);

  function openNewActivity(date = "") {
    setEditingEventId(null);
    setFormState({ ...initialFormState, date });
    setIsModalOpen(true);
  }

  function openEditActivity(item: DealTimelineItem) {
    setEditingEventId(item.id);
    setFormState({
      category: normalizeCategory(item.category),
      date: item.date,
      detail: item.detail,
      time: getEventTime(item),
      title: item.title,
    });
    setIsModalOpen(true);
  }

  function closeModal() {
    setIsModalOpen(false);
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const title = formState.title.trim();
    const detail = formState.detail.trim();

    if (!title || !formState.date) {
      return;
    }

    const timelineItem: DealTimelineItem = {
      category: formState.category,
      date: formState.date,
      detail: detail || "No notes added yet.",
      id: editingEventId ?? `timeline-${Date.now()}`,
      time: formState.time,
      timestamp: formatTimelineTimestamp(formState.date, formState.time),
      title,
      tone: toneByCategory[formState.category],
    };

    onEventsChange(
      editingEventId
        ? events.map((item) => (item.id === editingEventId ? timelineItem : item))
        : [...events, timelineItem],
    );
    setIsModalOpen(false);
    setToastMessage(editingEventId ? "Activity updated and moved successfully" : "Deal activity added successfully");
    setEditingEventId(null);
    window.setTimeout(() => setToastMessage(""), 3200);
  }

  return (
    <>
      <div className="flex flex-col gap-10">
        <header className="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
          <div className="space-y-1">
            <h1 className="type-display text-text-main">Deal Activity</h1>
            <p className="text-[13px] text-text-main/78">{deal.overviewSubtitle}</p>
          </div>

          <div className="flex flex-wrap items-center gap-6">
            <TimelineLegend />
            <button
              className="inline-flex h-10 items-center justify-center gap-2 rounded-full bg-primary px-5 text-[12px] font-bold text-white shadow-[0_10px_26px_rgba(80,101,142,0.24)] transition hover:bg-primary-container focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
              onClick={() => openNewActivity()}
              type="button"
            >
              <Icon className="h-4 w-4" name="plus" />
              Log Activity
            </button>
          </div>
        </header>

        <div className="divide-y divide-outline-variant border-y border-outline-variant">
          <section className="py-10">
            <div className="mb-7 flex items-center justify-between">
              <h2 className="type-h1 text-text-main">Activity Calendar</h2>
              <button
                aria-label="Deal activity actions"
                className="inline-flex h-10 w-10 items-center justify-center rounded-full text-muted transition hover:bg-white/58 hover:text-text-main"
                type="button"
              >
                <Icon className="h-5 w-5" name="more" />
              </button>
            </div>

            <div className="overflow-x-auto">
              <div
                className="grid min-w-[740px] border-l border-t border-outline-variant bg-white/20"
                style={{ gridTemplateColumns: "40px repeat(5, minmax(0, 1fr))" }}
              >
                <div className="border-b border-r border-outline-variant bg-white/35" />
                {weekdays.map((weekday) => (
                  <div
                    className="border-b border-r border-outline-variant bg-[#00004d] px-3 py-3 text-center text-[12px] font-bold uppercase tracking-[0.05em] text-white"
                    key={weekday}
                  >
                    {weekday}
                  </div>
                ))}

                {calendarWeeks.map((week, weekIndex) => (
                  <CalendarWeekRow
                    eventsByDate={eventsByDate}
                    key={weekIndex}
                    onCreateEvent={openNewActivity}
                    onEditEvent={openEditActivity}
                    week={week}
                    weekIndex={weekIndex}
                  />
                ))}
              </div>
            </div>

            <div className="mt-8 flex flex-col gap-4 rounded-xl border border-primary/10 bg-primary/5 p-4 sm:flex-row sm:items-center sm:justify-between">
              <div className="flex items-start gap-3">
                <Icon className="mt-0.5 h-5 w-5 shrink-0 text-primary" name="alert" />
                <p className="text-[12px] leading-5 text-text-main/78">
                  Viewing 5-week overview for <span className="font-bold text-text-main">Phase 1: Discovery</span>. Events are placed by
                  date.
                </p>
              </div>
              <button className="text-left text-[12px] font-bold text-primary transition hover:text-primary-container" type="button">
                Download PDF
              </button>
            </div>
          </section>

          <ActivityTimelineCard className="flex min-h-[480px] flex-col py-10" items={events} />

          <TaskKanbanBoard tasks={deal.pendingTasks} />
        </div>
      </div>

      {isModalOpen ? (
        <NewActivityModal
          formState={formState}
          editing={Boolean(editingEventId)}
          onChange={setFormState}
          onClose={closeModal}
          onSubmit={handleSubmit}
        />
      ) : null}

      <div
        aria-live="polite"
        className={`fixed bottom-8 right-8 z-50 flex items-center gap-3 rounded-[16px] border border-white/80 bg-white/76 px-5 py-4 text-sm font-semibold text-text-main shadow-[0_18px_50px_rgba(7,1,84,0.16)] backdrop-blur-md transition ${
          toastMessage ? "translate-y-0 opacity-100" : "pointer-events-none translate-y-8 opacity-0"
        }`}
      >
        <Icon className="h-5 w-5 text-primary" name="checkCircle" />
        {toastMessage}
      </div>
    </>
  );
}

function TimelineLegend() {
  return (
    <div className="flex flex-wrap items-center gap-4 text-[10px] font-bold uppercase tracking-[0.08em] text-text-main">
      {categoryOptions.map((category) => (
        <div className="flex items-center gap-1.5" key={category}>
          <span className={`h-3 w-3 rounded-sm ${categoryStyles[category].legend}`} />
          {category === "Key Meeting / Call" ? "Key Meeting" : category}
        </div>
      ))}
    </div>
  );
}

type CalendarWeekRowProps = {
  eventsByDate: Record<string, DealTimelineItem[]>;
  onCreateEvent: (date: string) => void;
  onEditEvent: (event: DealTimelineItem) => void;
  week: CalendarDay[];
  weekIndex: number;
};

function CalendarWeekRow({ eventsByDate, onCreateEvent, onEditEvent, week, weekIndex }: CalendarWeekRowProps) {
  return (
    <>
      <div className="flex min-h-[100px] items-center justify-center border-b border-r border-outline-variant px-1 text-center text-[11px] leading-4 text-text-main/78">
        Week
        <br />
        {weekIndex}
      </div>
      {week.map((day) => (
        <CalendarCell
          day={day}
          events={eventsByDate[day.dateKey] ?? []}
          key={day.dateKey}
          onCreateEvent={onCreateEvent}
          onEditEvent={onEditEvent}
        />
      ))}
    </>
  );
}

type CalendarCellProps = {
  day: CalendarDay;
  events: DealTimelineItem[];
  onCreateEvent: (date: string) => void;
  onEditEvent: (event: DealTimelineItem) => void;
};

function CalendarCell({ day, events, onCreateEvent, onEditEvent }: CalendarCellProps) {
  return (
    <div className="group relative min-h-[100px] border-b border-r border-outline-variant p-2 text-[10px] font-medium text-text-main/78 transition hover:bg-primary/5">
      <button
        aria-label={`Add activity on ${formatAccessibleDate(day.date)}`}
        className="absolute inset-0 z-0 cursor-cell focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-primary"
        onClick={() => onCreateEvent(day.dateKey)}
        type="button"
      />
      <div className="pointer-events-none relative z-10">{day.dayLabel}</div>
      <div className="relative z-20 mt-2 space-y-1">
        {events
          .slice()
          .sort((first, second) => getEventTime(first).localeCompare(getEventTime(second)))
          .map((event) => (
            <CalendarEventBar event={event} key={event.id} onEdit={onEditEvent} />
          ))}
      </div>
      <span className="pointer-events-none absolute bottom-2 right-2 z-10 flex items-center gap-1 text-[10px] font-bold text-primary opacity-0 transition group-hover:opacity-100 group-focus-within:opacity-100">
        <Icon className="h-3 w-3" name="plus" />
        Add activity
      </span>
    </div>
  );
}

type CalendarEventBarProps = {
  event: DealTimelineItem;
  onEdit: (event: DealTimelineItem) => void;
};

function CalendarEventBar({ event, onEdit }: CalendarEventBarProps) {
  const category = normalizeCategory(event.category);
  const styles = categoryStyles[category];
  const timeLabel = formatTimeLabel(getEventTime(event));

  return (
    <button
      className={`block h-7 w-full max-w-full truncate px-3 py-1 text-left text-[10px] font-bold leading-5 transition hover:brightness-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed [clip-path:polygon(5%_0%,95%_0%,100%_50%,95%_100%,5%_100%,0%_50%)] ${styles.bar}`}
      onClick={(clickEvent) => {
        clickEvent.stopPropagation();
        onEdit(event);
      }}
      title={`Edit ${event.title} — ${formatTimelineDate(event.date)} at ${timeLabel}`}
      type="button"
    >
      {timeLabel} · {event.title}
    </button>
  );
}

type TaskKanbanBoardProps = {
  tasks: DealTask[];
};

type TaskKanbanColumn = {
  description: string;
  id: ReminderStatus;
  tasks: WorkflowReminder[];
  title: string;
};

const reminderStatusLabels: Record<ReminderStatus, string> = {
  done: "Done",
  "in-progress": "In progress",
  "to-do": "To-do",
};

const reminderStatuses: ReminderStatus[] = ["to-do", "in-progress", "done"];

function getReminderStatusAtPoint(clientX: number, clientY: number): ReminderStatus | null {
  const status = document
    .elementFromPoint(clientX, clientY)
    ?.closest<HTMLElement>("[data-reminder-status]")
    ?.dataset.reminderStatus;
  return reminderStatuses.includes(status as ReminderStatus) ? (status as ReminderStatus) : null;
}

function TaskKanbanBoard({ tasks }: TaskKanbanBoardProps) {
  const [reminders, setReminders] = useState<WorkflowReminder[]>(() =>
    tasks.map(toWorkflowReminder),
  );
  const [draggedReminderId, setDraggedReminderId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<ReminderStatus | null>(null);
  const [newReminderLabel, setNewReminderLabel] = useState("");
  const [showNewReminderForm, setShowNewReminderForm] = useState(false);
  const columns: TaskKanbanColumn[] = [
    {
      description: "Open diligence reminders",
      id: "to-do",
      tasks: reminders.filter((task) => task.status === "to-do"),
      title: "To-do",
    },
    {
      description: "Needs active follow-up",
      id: "in-progress",
      tasks: reminders.filter((task) => task.status === "in-progress"),
      title: "In progress",
    },
    {
      description: "Closed out reminders",
      id: "done",
      tasks: reminders.filter((task) => task.status === "done"),
      title: "Done",
    },
  ];

  function handleAddReminder(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const label = newReminderLabel.trim();

    if (!label) {
      return;
    }

    setReminders((current) => [
      ...current,
      {
        id: `reminder-${Date.now()}`,
        label,
        status: "to-do",
      },
    ]);
    setNewReminderLabel("");
    setShowNewReminderForm(false);
  }

  function updateReminderStatus(reminderId: string, status: ReminderStatus) {
    setReminders((current) => moveReminder(current, reminderId, status));
  }

  function handlePointerStart(event: PointerEvent<HTMLElement>, reminderId: string) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    setDraggedReminderId(reminderId);
  }

  function handlePointerMove(event: PointerEvent<HTMLElement>) {
    const status = getReminderStatusAtPoint(event.clientX, event.clientY);
    setDropTarget(status);
  }

  function handlePointerEnd(event: PointerEvent<HTMLElement>, reminderId: string) {
    const status = getReminderStatusAtPoint(event.clientX, event.clientY);
    if (status) updateReminderStatus(reminderId, status);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    setDraggedReminderId(null);
    setDropTarget(null);
  }

  function handlePointerCancel() {
    setDraggedReminderId(null);
    setDropTarget(null);
  }

  return (
    <section className="py-10">
      <div className="mb-7 flex items-center justify-between gap-4">
        <div>
          <h2 className="type-h1 text-text-main">Pending Tasks</h2>
          <p className="mt-1 text-[13px] text-text-main/70">Track diligence reminders by workflow status.</p>
        </div>
        <button
          aria-expanded={showNewReminderForm}
          className="inline-flex h-11 items-center justify-center gap-2 rounded-full bg-primary px-5 text-[13px] font-bold text-white shadow-[0_10px_26px_rgba(80,101,142,0.22)] transition hover:bg-primary-container focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
          onClick={() => setShowNewReminderForm((isOpen) => !isOpen)}
          type="button"
        >
          <Icon className="h-4 w-4" name="plus" />
          Add Reminder
        </button>
      </div>

      <div className="grid divide-y divide-outline-variant border-y border-outline-variant lg:grid-cols-3 lg:divide-x lg:divide-y-0">
        {columns.map((column) => (
          <section
            className={`flex min-h-[260px] flex-col px-6 py-6 transition-colors ${
              dropTarget === column.id ? "bg-primary/5" : ""
            }`}
            data-reminder-status={column.id}
            key={column.id}
          >
            <div className="mb-6 flex items-start justify-between gap-4">
              <div>
                <h3 className="type-h3 text-text-main">{column.title}</h3>
                <p className="mt-1 text-[12px] text-text-main/62">{column.description}</p>
              </div>
              <span className="rounded-full border border-outline-variant bg-white/70 px-3 py-1 text-[11px] font-bold text-muted">
                {column.tasks.length}
              </span>
            </div>

            <div className="flex flex-1 flex-col gap-5">
              {column.id === "to-do" && showNewReminderForm ? (
                <form
                  className="border-y border-primary/20 py-4"
                  onSubmit={handleAddReminder}
                >
                  <label className="block">
                    <span className="mb-2 block text-[10px] font-bold uppercase tracking-[0.16em] text-muted">New reminder</span>
                    <input
                      autoFocus
                      className="h-11 w-full rounded-[16px] border border-outline-variant bg-white/75 px-4 text-[14px] font-semibold text-text-main outline-none transition placeholder:text-muted/60 focus:border-primary focus:ring-2 focus:ring-primary/16"
                      onChange={(event) => setNewReminderLabel(event.currentTarget.value)}
                      placeholder="Add diligence reminder..."
                      value={newReminderLabel}
                    />
                  </label>
                  <div className="mt-3 flex justify-end gap-2">
                    <button
                      className="rounded-full px-4 py-2 text-[12px] font-bold text-muted transition hover:bg-surface-container-high hover:text-text-main"
                      onClick={() => {
                        setNewReminderLabel("");
                        setShowNewReminderForm(false);
                      }}
                      type="button"
                    >
                      Cancel
                    </button>
                    <button
                      className="rounded-full bg-primary px-4 py-2 text-[12px] font-bold text-white transition hover:bg-primary-container disabled:cursor-not-allowed disabled:opacity-50"
                      disabled={!newReminderLabel.trim()}
                      type="submit"
                    >
                      Add
                    </button>
                  </div>
                </form>
              ) : null}

              {column.tasks.length > 0 ? (
                <div className="divide-y divide-outline-variant border-y border-outline-variant">
                  {column.tasks.map((task) => (
                    <ReminderRow
                      dragging={draggedReminderId === task.id}
                      key={task.id}
                      onMove={updateReminderStatus}
                      onPointerCancel={handlePointerCancel}
                      onPointerEnd={handlePointerEnd}
                      onPointerMove={handlePointerMove}
                      onPointerStart={handlePointerStart}
                      task={task}
                    />
                  ))}
                </div>
              ) : (
                <div className="flex min-h-28 items-center justify-center border-y border-dashed border-outline-variant px-4 text-center text-[13px] font-medium text-muted">
                  {draggedReminderId ? `Drop in ${column.title}` : "No reminders here yet."}
                </div>
              )}
            </div>
          </section>
        ))}
      </div>
    </section>
  );
}

type ReminderRowProps = {
  dragging: boolean;
  onMove: (reminderId: string, status: ReminderStatus) => void;
  onPointerCancel: () => void;
  onPointerEnd: (event: PointerEvent<HTMLElement>, reminderId: string) => void;
  onPointerMove: (event: PointerEvent<HTMLElement>) => void;
  onPointerStart: (event: PointerEvent<HTMLElement>, reminderId: string) => void;
  task: WorkflowReminder;
};

function ReminderRow({
  dragging,
  onMove,
  onPointerCancel,
  onPointerEnd,
  onPointerMove,
  onPointerStart,
  task,
}: ReminderRowProps) {
  return (
    <article
      aria-label={`${task.label}, ${reminderStatusLabels[task.status]}`}
      className={`flex items-start gap-3 py-4 transition ${
        dragging ? "opacity-40" : "opacity-100"
      }`}
    >
      <span
        aria-label={`Drag ${task.label} to another status`}
        className="mt-1 flex h-7 w-5 shrink-0 touch-none cursor-grab select-none items-center justify-center text-muted active:cursor-grabbing"
        onPointerCancel={onPointerCancel}
        onPointerDown={(event) => onPointerStart(event, task.id)}
        onPointerMove={onPointerMove}
        onPointerUp={(event) => onPointerEnd(event, task.id)}
        title="Drag to another status"
      >
        <Icon className="h-4 w-4 rotate-90" name="more" />
      </span>
      <div className="min-w-0 flex-1">
        <p className={`text-[10px] font-bold uppercase tracking-[0.16em] ${task.priority ? "text-error/70" : "text-muted/80"}`}>
          {task.priority ? "High Priority" : reminderStatusLabels[task.status]}
        </p>
        <h4 className={`mt-1 text-[15px] font-bold leading-6 text-text-main ${task.done ? "line-through opacity-60" : ""}`}>
          {task.label}
        </h4>
        <p className={`mt-1 text-[12px] leading-5 ${task.priority ? "text-error" : "text-text-main/68"}`}>
          {task.done
            ? "Task completed."
            : task.priority
              ? "Needs immediate follow-up before the next review cycle."
              : "Open reminder for the diligence team."}
        </p>
      </div>
      <label className="shrink-0">
        <span className="sr-only">Move {task.label}</span>
        <select
          aria-label={`Move ${task.label}`}
          className="h-9 rounded-lg border border-outline-variant bg-background px-2 text-[11px] font-semibold text-text-main outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/16"
          onChange={(event) => onMove(task.id, event.currentTarget.value as ReminderStatus)}
          value={task.status}
        >
          {reminderStatuses.map((status) => (
            <option key={status} value={status}>
              {reminderStatusLabels[status]}
            </option>
          ))}
        </select>
      </label>
    </article>
  );
}

type NewActivityModalProps = {
  editing: boolean;
  formState: TimelineFormState;
  onChange: (formState: TimelineFormState) => void;
  onClose: () => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
};

function NewActivityModal({ editing, formState, onChange, onClose, onSubmit }: NewActivityModalProps) {
  const canSubmit = Boolean(formState.title.trim() && formState.date);
  const titleInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    titleInputRef.current?.focus();
  }, []);

  return (
    <div className="modal-backdrop fixed inset-0 z-[100] flex items-center justify-center p-4" role="presentation">
      <form
        aria-labelledby="activity-dialog-title"
        aria-modal="true"
        className="shrink-0 rounded-[19px] border border-white/80 bg-white/78 p-8 shadow-[0_28px_80px_rgba(7,1,84,0.18)] backdrop-blur-xl"
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onClose();
          }
        }}
        onSubmit={onSubmit}
        role="dialog"
        style={{ width: "min(calc(100vw - 32px), 28rem)" }}
      >
        <div className="mb-7 flex items-center justify-between">
          <h2 className="type-h1 text-text-main" id="activity-dialog-title">
            {editing ? "Edit Activity" : "New Activity"}
          </h2>
          <button
            aria-label={`Close ${editing ? "edit" : "new"} activity`}
            className="inline-flex h-10 w-10 items-center justify-center rounded-full text-text-main transition hover:bg-surface-container"
            onClick={onClose}
            type="button"
          >
            <span className="relative h-5 w-5 before:absolute before:left-1/2 before:top-0 before:h-full before:w-0.5 before:-translate-x-1/2 before:rotate-45 before:rounded-full before:bg-current after:absolute after:left-1/2 after:top-0 after:h-full after:w-0.5 after:-translate-x-1/2 after:-rotate-45 after:rounded-full after:bg-current" />
          </button>
        </div>

        <div className="space-y-5">
          <label className="block min-w-0">
            <span className="mb-2 block text-[12px] font-bold uppercase tracking-[0.12em] text-text-main/75">Activity Title</span>
            <input
              className="h-11 w-full rounded-xl border border-outline-variant bg-white/55 px-4 text-[15px] text-text-main outline-none transition placeholder:text-muted focus:border-primary focus:ring-2 focus:ring-primary/16"
              onChange={(event) => onChange({ ...formState, title: event.target.value })}
              placeholder="e.g. Stakeholder Interview"
              ref={titleInputRef}
              required
              type="text"
              value={formState.title}
            />
          </label>

          <div className="grid min-w-0 gap-4 sm:grid-cols-2">
            <label className="block min-w-0">
              <span className="mb-2 block text-[12px] font-bold uppercase tracking-[0.12em] text-text-main/75">Category</span>
              <select
                className="h-11 w-full rounded-xl border border-outline-variant bg-white/55 px-4 text-[15px] text-text-main outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/16"
                onChange={(event) => onChange({ ...formState, category: event.target.value as TimelineCategory })}
                value={formState.category}
              >
                {categoryOptions.map((category) => (
                  <option key={category}>{category}</option>
                ))}
              </select>
            </label>

            <label className="block min-w-0">
              <span className="mb-2 block text-[12px] font-bold uppercase tracking-[0.12em] text-text-main/75">Date</span>
              <input
                className="h-11 w-full rounded-xl border border-outline-variant bg-white/55 px-4 text-[15px] text-text-main outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/16"
                onChange={(event) => onChange({ ...formState, date: event.target.value })}
                required
                type="date"
                value={formState.date}
              />
            </label>
          </div>

          <label className="block min-w-0">
            <span className="mb-2 block text-[12px] font-bold uppercase tracking-[0.12em] text-text-main/75">Time</span>
            <input
              className="h-11 w-full rounded-xl border border-outline-variant bg-white/55 px-4 text-[15px] text-text-main outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/16"
              onChange={(event) => onChange({ ...formState, time: event.target.value })}
              required
              type="time"
              value={formState.time}
            />
          </label>

          <label className="block min-w-0">
            <span className="mb-2 block text-[12px] font-bold uppercase tracking-[0.12em] text-text-main/75">Notes</span>
            <textarea
              className="min-h-24 w-full resize-y rounded-xl border border-outline-variant bg-white/55 px-4 py-3 text-[15px] leading-6 text-text-main outline-none transition placeholder:text-muted focus:border-primary focus:ring-2 focus:ring-primary/16"
              onChange={(event) => onChange({ ...formState, detail: event.target.value })}
              placeholder="Describe the activity details..."
              rows={3}
              value={formState.detail}
            />
          </label>

          <button
            className="h-12 w-full rounded-2xl bg-primary px-5 text-[15px] font-bold text-white shadow-[0_12px_28px_rgba(80,101,142,0.26)] transition hover:bg-primary-container disabled:cursor-not-allowed disabled:opacity-55"
            disabled={!canSubmit}
            type="submit"
          >
            {editing ? "Save Changes" : "Log Activity"}
          </button>
        </div>
      </form>
    </div>
  );
}

function createCalendarWeeks(startDate: string, weekCount: number) {
  return Array.from({ length: weekCount }, (_, weekIndex) =>
    Array.from({ length: 5 }, (_, dayIndex) => {
      const date = addDays(parseLocalDate(startDate), weekIndex * 7 + dayIndex);
      return {
        date,
        dateKey: toDateKey(date),
        dayLabel: formatCalendarDayLabel(date),
      };
    }),
  );
}

function addDays(date: Date, days: number) {
  const nextDate = new Date(date);
  nextDate.setDate(nextDate.getDate() + days);
  return nextDate;
}

function parseLocalDate(date: string) {
  return new Date(`${date}T12:00:00`);
}

function toDateKey(date: Date) {
  return date.toISOString().slice(0, 10);
}

function getCalendarDateKey(date: string) {
  const parsedDate = parseLocalDate(date);
  const day = parsedDate.getDay();

  if (day === 6) {
    return toDateKey(addDays(parsedDate, -1));
  }

  if (day === 0) {
    return toDateKey(addDays(parsedDate, -2));
  }

  return toDateKey(parsedDate);
}

function normalizeCategory(category: string): TimelineCategory {
  if (category === "Deliverable") {
    return "Deliverable";
  }

  if (category === "Key Activity" || category === "Site Visit") {
    return "Key Activity";
  }

  return "Key Meeting / Call";
}

function formatCalendarDayLabel(date: Date) {
  const day = date.getDate();

  if (day === 1) {
    return `${date.toLocaleString("en-US", { month: "short" }).toUpperCase()} ${day}`;
  }

  return String(day);
}

function formatTimelineDate(date: string) {
  const parsedDate = parseLocalDate(date);

  return new Intl.DateTimeFormat("en-US", { day: "numeric", month: "short" }).format(parsedDate);
}

function formatTimelineTimestamp(date: string, time: string) {
  return `${formatTimelineDate(date)}, ${formatTimeLabel(time)}`;
}

function getEventTime(event: DealTimelineItem) {
  if (event.time) {
    return event.time;
  }

  const timeMatch = event.timestamp.match(/(\d{1,2}):(\d{2})\s*(AM|PM)/i);

  if (!timeMatch) {
    return "09:00";
  }

  const [, hourValue, minute, meridiem] = timeMatch;
  let hour = Number(hourValue) % 12;

  if (meridiem.toUpperCase() === "PM") {
    hour += 12;
  }

  return `${String(hour).padStart(2, "0")}:${minute}`;
}

function formatTimeLabel(time: string) {
  const [hour, minute] = time.split(":").map(Number);
  const timeValue = new Date(2000, 0, 1, hour, minute);

  return new Intl.DateTimeFormat("en-US", { hour: "numeric", minute: "2-digit" }).format(timeValue);
}

function formatAccessibleDate(date: Date) {
  return new Intl.DateTimeFormat("en-US", {
    day: "numeric",
    month: "long",
    weekday: "long",
    year: "numeric",
  }).format(date);
}

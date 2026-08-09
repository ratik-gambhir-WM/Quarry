import { invoke, isTauri } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { WorkspaceCard } from "../components/hub/WorkspaceCard";
import { Icon } from "../components/ui/Icon";
import { TAURI_COMMANDS, type TauriCommandName } from "../lib/constants";

type CommandRisk = "Read only" | "Writes data" | "Uses AI";

type CommandPreset = {
  args: Record<string, unknown>;
  description: string;
  name: TauriCommandName;
  risk: CommandRisk;
};

type EventPreset = {
  description: string;
  eventName: string;
  id: string;
  label: string;
  listeners: string[];
  payload: Record<string, unknown>;
};

type LogDirection = "error" | "invoke" | "received" | "result" | "sent" | "system";
type LogFilter = "all" | "commands" | "errors" | "events";

type ActivityLog = {
  direction: LogDirection;
  durationMs?: number;
  id: number;
  name: string;
  payload: unknown;
  timestamp: Date;
};

const commandPresets: CommandPreset[] = [
  {
    args: { name: "Quarry" },
    description: "A quick round-trip to Rust with no database or file access.",
    name: TAURI_COMMANDS.greet,
    risk: "Read only",
  },
  {
    args: {},
    description: "Returns the SQLite path and current schema version.",
    name: TAURI_COMMANDS.databaseStatus,
    risk: "Read only",
  },
  {
    args: { payload: { email: "analyst@westmonroe.com", source: "tauri-playground" } },
    description: "Echoes a structured payload through the demo Rust command.",
    name: TAURI_COMMANDS.loginDemoCommand,
    risk: "Read only",
  },
  {
    args: { email: "analyst@westmonroe.com" },
    description: "Checks whether an email is already stored in Quarry.",
    name: TAURI_COMMANDS.userExistsByEmail,
    risk: "Read only",
  },
  {
    args: { email: "analyst@westmonroe.com" },
    description: "Fetches a user record by email address.",
    name: TAURI_COMMANDS.getUserByEmail,
    risk: "Read only",
  },
  {
    args: { dealId: "project-alpha" },
    description: "Loads the file tree for a deal data room.",
    name: TAURI_COMMANDS.listDealDataRoom,
    risk: "Read only",
  },
  {
    args: { dealId: "project-alpha", relativePath: "path/to/document.pdf" },
    description: "Builds a preview for one document in a deal data room.",
    name: TAURI_COMMANDS.previewDealDocument,
    risk: "Read only",
  },
  {
    args: { payload: { path: "/absolute/path/to/folder" } },
    description: "Lists supported files under a local folder.",
    name: TAURI_COMMANDS.listSummaryFiles,
    risk: "Read only",
  },
  {
    args: { input: { apiKey: "replace-me", email: "analyst@example.com", firstName: "Quarry", lastName: "Tester", role: "Analyst" } },
    description: "Creates a user record. Replace the example values before running.",
    name: TAURI_COMMANDS.createUser,
    risk: "Writes data",
  },
  {
    args: {
      input: {
        apiKey: "replace-me",
        createdAt: "2026-08-03T15:00:00Z",
        email: "analyst@westmonroe.com",
        firstName: "Quarry",
        id: 1001,
        lastName: "Tester",
        role: "Analyst",
        updatedAt: "2026-08-03T15:00:00Z",
      },
    },
    description: "Upserts a User node through the parallel Helix command flow.",
    name: TAURI_COMMANDS.createWmUser,
    risk: "Writes data",
  },
  {
    args: { payload: { path: "/absolute/path/to/folder" } },
    description: "Summarizes the supported contents of a local directory.",
    name: TAURI_COMMANDS.summarize,
    risk: "Uses AI",
  },
  {
    args: { payload: { paths: ["/absolute/path/to/document.pdf"] } },
    description: "Summarizes an explicit set of local files.",
    name: TAURI_COMMANDS.summarizeSelected,
    risk: "Uses AI",
  },
  {
    args: { payload: { path: "/absolute/path/to/summary.md", summary: "# Summary\n\nPlayground output." } },
    description: "Writes markdown content to a local path.",
    name: TAURI_COMMANDS.saveMarkdownSummary,
    risk: "Writes data",
  },
  {
    args: {
      input: {
        buyerOrPlatformCompany: null,
        carveOutBusiness: null,
        dealName: "Playground Deal",
        dealType: "Buy-side",
        mainDataRoomFolder: "/absolute/path/to/data-room",
        parentOrSellerCompany: null,
        peFirm: "Example PE",
        targetCompany: "Example Target",
      },
    },
    description: "Creates a deal and discovers its source documents.",
    name: TAURI_COMMANDS.saveDealAndExtract,
    risk: "Writes data",
  },
  {
    args: { input: { dealId: 1, projectTimelineFilePath: null, sowFilePath: null } },
    description: "Extracts key questions and an investment thesis for a saved deal.",
    name: TAURI_COMMANDS.extractDealQuestionsAndThesis,
    risk: "Uses AI",
  },
];

const eventPresets: EventPreset[] = [
  {
    description: "Rust receives the request and emits a structured response event.",
    eventName: "login-demo:frontend-request",
    id: "login-demo",
    label: "Login demo round-trip",
    listeners: ["login-demo:backend-response"],
    payload: {
      email: "analyst@westmonroe.com",
      note: "Testing the event bridge from the playground",
      source: "tauri-playground",
    },
  },
];

const directionLabels: Record<LogDirection, string> = {
  error: "Error",
  invoke: "Invoke",
  received: "Received",
  result: "Result",
  sent: "Emitted",
  system: "System",
};

export function TauriPlaygroundPage() {
  const nativeRuntime = isTauri();
  const firstCommand = commandPresets[0];
  const firstEvent = eventPresets[0];
  const [commandPresetName, setCommandPresetName] = useState<TauriCommandName>(firstCommand.name);
  const [commandName, setCommandName] = useState(firstCommand.name as string);
  const [commandArgs, setCommandArgs] = useState(formatJson(firstCommand.args));
  const [eventPresetId, setEventPresetId] = useState(firstEvent.id);
  const [eventName, setEventName] = useState(firstEvent.eventName);
  const [eventPayload, setEventPayload] = useState(formatJson(firstEvent.payload));
  const [listenerDraft, setListenerDraft] = useState("");
  const [listenerNames, setListenerNames] = useState<string[]>(firstEvent.listeners);
  const [isCommandRunning, setIsCommandRunning] = useState(false);
  const [isEventSending, setIsEventSending] = useState(false);
  const [filter, setFilter] = useState<LogFilter>("all");
  const [logs, setLogs] = useState<ActivityLog[]>(() => [
    {
      direction: "system",
      id: Date.now(),
      name: nativeRuntime ? "Tauri runtime connected" : "Browser preview mode",
      payload: nativeRuntime
        ? { commands: commandPresets.length, listeners: firstEvent.listeners }
        : { hint: "Run `npm run tauri dev` to invoke Rust commands and emit events." },
      timestamp: new Date(),
    },
  ]);

  const selectedCommand = commandPresets.find((preset) => preset.name === commandPresetName) ?? firstCommand;
  const commandJsonError = getJsonError(commandArgs);
  const eventJsonError = getJsonError(eventPayload);
  const visibleLogs = useMemo(
    () => logs.filter((entry) => matchesFilter(entry, filter)),
    [filter, logs],
  );

  useEffect(() => {
    if (!nativeRuntime || listenerNames.length === 0) {
      return;
    }

    let disposed = false;
    const unlisteners: UnlistenFn[] = [];

    void Promise.all(
      listenerNames.map((name) =>
        listen<unknown>(name, (event) => {
          addLog({ direction: "received", name: event.event, payload: event.payload });
        }).then((unlisten) => {
          if (disposed) {
            unlisten();
          } else {
            unlisteners.push(unlisten);
          }
        }),
      ),
    ).catch((error) => {
      addLog({ direction: "error", name: "event listener", payload: toErrorPayload(error) });
    });

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [listenerNames, nativeRuntime]);

  function addLog(entry: Omit<ActivityLog, "id" | "timestamp">) {
    setLogs((current) => [
      { ...entry, id: Date.now() + Math.random(), timestamp: new Date() },
      ...current,
    ]);
  }

  function handleCommandChange(nextName: TauriCommandName) {
    const preset = commandPresets.find((item) => item.name === nextName);
    if (!preset) {
      return;
    }

    setCommandName(nextName);
    setCommandPresetName(nextName);
    setCommandArgs(formatJson(preset.args));
  }

  function handleEventPresetChange(nextId: string) {
    const preset = eventPresets.find((item) => item.id === nextId);
    if (!preset) {
      return;
    }

    setEventPresetId(nextId);
    setEventName(preset.eventName);
    setEventPayload(formatJson(preset.payload));
    setListenerNames((current) => Array.from(new Set([...current, ...preset.listeners])));
  }

  function resetCommand() {
    setCommandArgs(formatJson(selectedCommand.args));
  }

  async function runCommand() {
    if (!nativeRuntime || commandJsonError) {
      return;
    }

    const args = JSON.parse(commandArgs) as Record<string, unknown>;
    const startedAt = performance.now();
    addLog({ direction: "invoke", name: commandName, payload: args });
    setIsCommandRunning(true);

    try {
      const response = await invoke<unknown>(commandName, args);
      addLog({
        direction: "result",
        durationMs: Math.round(performance.now() - startedAt),
        name: commandName,
        payload: response ?? null,
      });
    } catch (error) {
      addLog({
        direction: "error",
        durationMs: Math.round(performance.now() - startedAt),
        name: commandName,
        payload: toErrorPayload(error),
      });
    } finally {
      setIsCommandRunning(false);
    }
  }

  async function sendEvent() {
    const normalizedName = eventName.trim();
    if (!nativeRuntime || eventJsonError || !normalizedName) {
      return;
    }

    const payload = JSON.parse(eventPayload) as unknown;
    setIsEventSending(true);

    try {
      await emit(normalizedName, payload);
      addLog({ direction: "sent", name: normalizedName, payload });
    } catch (error) {
      addLog({ direction: "error", name: normalizedName, payload: toErrorPayload(error) });
    } finally {
      setIsEventSending(false);
    }
  }

  function addListener() {
    const name = listenerDraft.trim();
    if (!name || listenerNames.includes(name)) {
      return;
    }

    setListenerNames((current) => [...current, name]);
    setListenerDraft("");
  }

  return (
    <WorkspaceHomeShell activeHomeSection="tauri-playground">
      <div className="mx-auto flex w-full max-w-[1320px] flex-col gap-6 pb-10">
        <header className="flex flex-col justify-between gap-5 sm:flex-row sm:items-end">
          <div>
            <div className="mb-3 flex items-center gap-2 text-[12px] font-bold uppercase tracking-[0.18em] text-primary">
              <span className="flex h-7 w-7 items-center justify-center rounded-lg bg-primary/10">
                <Icon className="h-4 w-4" name="terminal" />
              </span>
              Developer tools
            </div>
            <h1 className="type-h1 text-text-main">Tauri Playground</h1>
            <p className="mt-2 max-w-3xl text-[15px] leading-6 text-muted">
              Invoke Rust commands, emit app events, and inspect every request and response as JSON.
            </p>
          </div>
          <RuntimeBadge connected={nativeRuntime} />
        </header>

        {!nativeRuntime ? (
          <div className="flex items-start gap-3 rounded-[16px] border border-[#c98a28]/30 bg-[#fff7e8] px-5 py-4 text-[#754b0d] [html[data-theme=dark]_&]:border-[#e3ad55]/25 [html[data-theme=dark]_&]:bg-[#2b2113] [html[data-theme=dark]_&]:text-[#f0c882]">
            <Icon className="mt-0.5 h-5 w-5 shrink-0" name="alert" />
            <div>
              <p className="text-[13px] font-bold">Previewing in a browser</p>
              <p className="mt-1 text-[13px] leading-5">
                The editors and log are available, but native actions stay disabled until this page is opened through
                <code className="mx-1 rounded bg-black/6 px-1.5 py-0.5 [font-family:ui-monospace,SFMono-Regular,Menlo,monospace]">npm run tauri dev</code>.
              </p>
            </div>
          </div>
        ) : null}

        <div className="grid gap-6 xl:grid-cols-2">
          <WorkspaceCard className="flex min-h-[570px] flex-col overflow-hidden" radius="compact">
            <CardHeader
              badge={`${commandPresets.length} presets`}
              description="Call a Rust handler and inspect its serialized result."
              icon="terminal"
              title="Command runner"
            />

            <div className="flex flex-1 flex-col gap-5 p-6">
              <div className="grid gap-4 sm:grid-cols-[0.85fr_1.15fr]">
                <label className="space-y-2">
                  <span className="text-[12px] font-bold uppercase tracking-[0.13em] text-muted">Preset</span>
                  <select
                    className="h-12 w-full rounded-[13px] border border-outline-variant bg-surface-container-lowest px-4 text-[13px] font-semibold text-text-main outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/12"
                    onChange={(event) => handleCommandChange(event.target.value as TauriCommandName)}
                    value={commandPresetName}
                  >
                    {commandPresets.map((preset) => (
                      <option key={preset.name} value={preset.name}>
                        {preset.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="space-y-2">
                  <span className="text-[12px] font-bold uppercase tracking-[0.13em] text-muted">Command name</span>
                  <input
                    className="h-12 w-full rounded-[13px] border border-outline-variant bg-surface-container-lowest px-4 text-[13px] text-text-main outline-none transition [font-family:ui-monospace,SFMono-Regular,Menlo,monospace] focus:border-primary focus:ring-2 focus:ring-primary/12"
                    onChange={(event) => setCommandName(event.target.value)}
                    value={commandName}
                  />
                </label>
              </div>

              <div className="flex min-h-[58px] items-start justify-between gap-4 rounded-[13px] bg-surface-container-low px-4 py-3">
                <p className="text-[13px] leading-5 text-on-surface-variant">{selectedCommand.description}</p>
                <RiskBadge risk={selectedCommand.risk} />
              </div>

              <JsonEditor
                error={commandJsonError}
                label="Arguments"
                onChange={setCommandArgs}
                value={commandArgs}
              />

              <div className="mt-auto flex items-center justify-between gap-3">
                <button
                  className="rounded-full px-4 py-2 text-[13px] font-semibold text-muted transition hover:bg-surface-container-high hover:text-text-main"
                  onClick={resetCommand}
                  type="button"
                >
                  Reset JSON
                </button>
                <button
                  className="inline-flex min-w-36 items-center justify-center gap-2 rounded-full bg-primary-container px-5 py-3 text-[13px] font-bold text-on-primary-container shadow-[0_8px_22px_rgba(7,1,84,0.16)] transition hover:bg-primary disabled:cursor-not-allowed disabled:opacity-45"
                  disabled={!nativeRuntime || Boolean(commandJsonError) || !commandName.trim() || isCommandRunning}
                  onClick={() => void runCommand()}
                  type="button"
                >
                  <Icon className="h-4 w-4" name={isCommandRunning ? "refresh" : "send"} />
                  {isCommandRunning ? "Running…" : "Run command"}
                </button>
              </div>
            </div>
          </WorkspaceCard>

          <WorkspaceCard className="flex min-h-[570px] flex-col overflow-hidden" radius="compact">
            <CardHeader
              badge={`${listenerNames.length} listening`}
              description="Send an event and capture backend responses in real time."
              icon="timeline"
              title="Event bridge"
            />

            <div className="flex flex-1 flex-col gap-5 p-6">
              <div className="grid gap-4 sm:grid-cols-[1fr_1.15fr]">
                <label className="space-y-2">
                  <span className="text-[12px] font-bold uppercase tracking-[0.13em] text-muted">Preset</span>
                  <select
                    className="h-12 w-full rounded-[13px] border border-outline-variant bg-surface-container-lowest px-4 text-[14px] font-semibold text-text-main outline-none transition focus:border-primary focus:ring-2 focus:ring-primary/12"
                    onChange={(event) => handleEventPresetChange(event.target.value)}
                    value={eventPresetId}
                  >
                    {eventPresets.map((preset) => (
                      <option key={preset.id} value={preset.id}>
                        {preset.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="space-y-2">
                  <span className="text-[12px] font-bold uppercase tracking-[0.13em] text-muted">Event name</span>
                  <input
                    className="h-12 w-full rounded-[13px] border border-outline-variant bg-surface-container-lowest px-4 text-[13px] text-text-main outline-none transition [font-family:ui-monospace,SFMono-Regular,Menlo,monospace] focus:border-primary focus:ring-2 focus:ring-primary/12"
                    onChange={(event) => setEventName(event.target.value)}
                    value={eventName}
                  />
                </label>
              </div>

              <p className="-mt-2 text-[13px] leading-5 text-on-surface-variant">
                {eventPresets.find((preset) => preset.id === eventPresetId)?.description}
              </p>

              <JsonEditor
                error={eventJsonError}
                label="Payload"
                onChange={setEventPayload}
                value={eventPayload}
              />

              <div className="space-y-3">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-[12px] font-bold uppercase tracking-[0.13em] text-muted">Response listeners</span>
                  <span className="text-[11px] font-semibold text-muted">Live while page is open</span>
                </div>
                <div className="flex flex-wrap gap-2">
                  {listenerNames.map((name) => (
                    <button
                      className="group inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/8 px-3 py-1.5 text-[11px] font-semibold text-primary transition hover:bg-primary/12"
                      key={name}
                      onClick={() => setListenerNames((current) => current.filter((item) => item !== name))}
                      title="Remove listener"
                      type="button"
                    >
                      <span className="h-1.5 w-1.5 rounded-full bg-[#2f9b70]" />
                      {name}
                      <span className="text-primary/45 transition group-hover:text-primary">×</span>
                    </button>
                  ))}
                </div>
                <div className="flex gap-2">
                  <input
                    className="h-10 min-w-0 flex-1 rounded-[11px] border border-outline-variant bg-surface-container-lowest px-3 text-[12px] text-text-main outline-none [font-family:ui-monospace,SFMono-Regular,Menlo,monospace] focus:border-primary"
                    onChange={(event) => setListenerDraft(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        addListener();
                      }
                    }}
                    placeholder="custom:event-name"
                    value={listenerDraft}
                  />
                  <button
                    className="rounded-[11px] border border-outline-variant px-4 text-[12px] font-bold text-text-main transition hover:bg-surface-container-high disabled:opacity-40"
                    disabled={!listenerDraft.trim()}
                    onClick={addListener}
                    type="button"
                  >
                    Add
                  </button>
                </div>
              </div>

              <div className="mt-auto flex justify-end">
                <button
                  className="inline-flex min-w-36 items-center justify-center gap-2 rounded-full bg-primary-container px-5 py-3 text-[13px] font-bold text-on-primary-container shadow-[0_8px_22px_rgba(7,1,84,0.16)] transition hover:bg-primary disabled:cursor-not-allowed disabled:opacity-45"
                  disabled={!nativeRuntime || Boolean(eventJsonError) || !eventName.trim() || isEventSending}
                  onClick={() => void sendEvent()}
                  type="button"
                >
                  <Icon className="h-4 w-4" name="send" />
                  {isEventSending ? "Emitting…" : "Emit event"}
                </button>
              </div>
            </div>
          </WorkspaceCard>
        </div>

        <WorkspaceCard className="overflow-hidden" radius="compact">
          <div className="flex flex-col justify-between gap-4 border-b border-outline-variant px-6 py-5 sm:flex-row sm:items-center">
            <div>
              <div className="flex items-center gap-3">
                <span className="flex h-9 w-9 items-center justify-center rounded-xl bg-surface-container-low text-primary">
                  <Icon className="h-5 w-5" name="listAlt" />
                </span>
                <div>
                  <h2 className="text-[18px] font-bold text-text-main">Activity log</h2>
                  <p className="text-[12px] text-muted">Newest first · in memory only</p>
                </div>
              </div>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {(["all", "commands", "events", "errors"] as LogFilter[]).map((option) => (
                <button
                  className={`rounded-full px-3.5 py-2 text-[11px] font-bold capitalize transition ${
                    filter === option
                      ? "bg-primary-container text-on-primary-container"
                      : "bg-surface-container-low text-muted hover:bg-surface-container-high hover:text-text-main"
                  }`}
                  key={option}
                  onClick={() => setFilter(option)}
                  type="button"
                >
                  {option}
                </button>
              ))}
              <span className="mx-1 hidden h-5 w-px bg-outline-variant sm:block" />
              <button
                className="rounded-full px-3.5 py-2 text-[11px] font-bold text-muted transition hover:bg-surface-container-high hover:text-text-main disabled:opacity-35"
                disabled={logs.length === 0}
                onClick={() => setLogs([])}
                type="button"
              >
                Clear
              </button>
            </div>
          </div>

          <div className="max-h-[560px] min-h-[210px] overflow-y-auto bg-[#f8f9fc] [html[data-theme=dark]_&]:bg-[#0b1020]">
            {visibleLogs.length ? (
              <div className="divide-y divide-outline-variant/70">
                {visibleLogs.map((entry) => (
                  <LogEntry entry={entry} key={entry.id} />
                ))}
              </div>
            ) : (
              <div className="flex min-h-[210px] items-center justify-center px-6 text-center">
                <div>
                  <Icon className="mx-auto h-7 w-7 text-muted/50" name="terminal" />
                  <p className="mt-3 text-[13px] font-semibold text-muted">No matching activity yet</p>
                </div>
              </div>
            )}
          </div>
        </WorkspaceCard>
      </div>
    </WorkspaceHomeShell>
  );
}

type CardHeaderProps = {
  badge: string;
  description: string;
  icon: "terminal" | "timeline";
  title: string;
};

function CardHeader({ badge, description, icon, title }: CardHeaderProps) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-outline-variant bg-surface-container-low/55 px-6 py-5">
      <div className="flex items-start gap-3">
        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-[13px] bg-primary/10 text-primary">
          <Icon className="h-5 w-5" name={icon} />
        </span>
        <div>
          <h2 className="text-[18px] font-bold text-text-main">{title}</h2>
          <p className="mt-1 text-[12px] leading-5 text-muted">{description}</p>
        </div>
      </div>
      <span className="shrink-0 rounded-full border border-outline-variant bg-surface-container-lowest px-3 py-1.5 text-[10px] font-bold uppercase tracking-[0.1em] text-muted">
        {badge}
      </span>
    </div>
  );
}

type JsonEditorProps = {
  error: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
};

function JsonEditor({ error, label, onChange, value }: JsonEditorProps) {
  return (
    <label className="flex flex-1 flex-col gap-2">
      <span className="flex items-center justify-between gap-3">
        <span className="text-[12px] font-bold uppercase tracking-[0.13em] text-muted">{label}</span>
        <span className={`text-[11px] font-semibold ${error ? "text-error" : "text-[#2f7d60] [html[data-theme=dark]_&]:text-[#76c9a5]"}`}>
          {error || "Valid JSON"}
        </span>
      </span>
      <textarea
        className={`min-h-[190px] flex-1 resize-y rounded-[13px] border bg-[#f8f9fc] p-4 text-[12px] leading-5 text-on-surface outline-none transition [font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace] [tab-size:2] [html[data-theme=dark]_&]:bg-[#0b1020] ${
          error ? "border-error/60 focus:ring-2 focus:ring-error/10" : "border-outline-variant focus:border-primary focus:ring-2 focus:ring-primary/12"
        }`}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        value={value}
      />
    </label>
  );
}

function RuntimeBadge({ connected }: { connected: boolean }) {
  return (
    <div className="flex w-fit items-center gap-3 rounded-full border border-outline-variant bg-surface-container-lowest px-4 py-2.5 shadow-[0_6px_18px_rgba(7,1,84,0.05)]">
      <span className={`relative flex h-2.5 w-2.5 ${connected ? "text-[#2f9b70]" : "text-[#c98a28]"}`}>
        {connected ? <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-current opacity-25" /> : null}
        <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-current" />
      </span>
      <span className="text-[12px] font-bold text-text-main">{connected ? "Tauri connected" : "Browser preview"}</span>
    </div>
  );
}

function RiskBadge({ risk }: { risk: CommandRisk }) {
  const classes =
    risk === "Read only"
      ? "bg-[#eaf7f1] text-[#267555] [html[data-theme=dark]_&]:bg-[#163126] [html[data-theme=dark]_&]:text-[#77cba6]"
      : risk === "Writes data"
        ? "bg-[#fff3df] text-[#8b5b14] [html[data-theme=dark]_&]:bg-[#332817] [html[data-theme=dark]_&]:text-[#e9bd73]"
        : "bg-[#eeeafd] text-[#5e4b9b] [html[data-theme=dark]_&]:bg-[#28223f] [html[data-theme=dark]_&]:text-[#b9a9ef]";

  return <span className={`shrink-0 rounded-full px-3 py-1.5 text-[10px] font-bold uppercase tracking-[0.08em] ${classes}`}>{risk}</span>;
}

function LogEntry({ entry }: { entry: ActivityLog }) {
  const [copied, setCopied] = useState(false);
  const payload = formatJson(entry.payload);
  const isError = entry.direction === "error";

  async function copyPayload() {
    try {
      await navigator.clipboard.writeText(payload);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch {
      setCopied(false);
    }
  }

  return (
    <article className="group px-6 py-5 transition hover:bg-surface-container-low/65">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start">
        <div className="flex min-w-[118px] items-center gap-2">
          <span className={`h-2 w-2 rounded-full ${directionDotClass(entry.direction)}`} />
          <span className={`text-[11px] font-bold uppercase tracking-[0.1em] ${isError ? "text-error" : "text-muted"}`}>
            {directionLabels[entry.direction]}
          </span>
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <code className={`break-all text-[13px] font-bold ${isError ? "text-error" : "text-text-main"}`}>{entry.name}</code>
            <div className="flex items-center gap-3 text-[10px] font-semibold text-muted">
              {entry.durationMs !== undefined ? <span>{entry.durationMs} ms</span> : null}
              <time>{formatTime(entry.timestamp)}</time>
              <button
                className="rounded-md px-2 py-1 transition hover:bg-surface-container-high hover:text-text-main"
                onClick={() => void copyPayload()}
                type="button"
              >
                {copied ? "Copied" : "Copy JSON"}
              </button>
            </div>
          </div>
          <pre className={`mt-3 overflow-x-auto whitespace-pre-wrap break-words rounded-[11px] border p-4 text-[11px] leading-5 [font-family:ui-monospace,SFMono-Regular,Menlo,Monaco,Consolas,monospace] ${
            isError
              ? "border-error/18 bg-error-container/35 text-error"
              : "border-outline-variant/70 bg-surface-container-lowest/75 text-on-surface-variant"
          }`}>
            {payload}
          </pre>
        </div>
      </div>
    </article>
  );
}

function directionDotClass(direction: LogDirection) {
  switch (direction) {
    case "error":
      return "bg-error";
    case "received":
    case "result":
      return "bg-[#2f9b70]";
    case "invoke":
    case "sent":
      return "bg-[#6b87c8]";
    default:
      return "bg-muted";
  }
}

function matchesFilter(entry: ActivityLog, filter: LogFilter) {
  if (filter === "all") {
    return true;
  }
  if (filter === "errors") {
    return entry.direction === "error";
  }
  if (filter === "commands") {
    return entry.direction === "invoke" || entry.direction === "result";
  }
  return entry.direction === "sent" || entry.direction === "received";
}

function getJsonError(value: string) {
  try {
    JSON.parse(value);
    return "";
  } catch (error) {
    if (error instanceof SyntaxError) {
      return error.message.replace(/^JSON\.parse: /, "");
    }
    return "Invalid JSON";
  }
}

function formatJson(value: unknown) {
  if (value === undefined) {
    return "null";
  }

  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

function toErrorPayload(error: unknown) {
  if (error instanceof Error) {
    return { message: error.message, name: error.name };
  }
  if (typeof error === "string") {
    return { message: error };
  }
  return error;
}

function formatTime(date: Date) {
  return new Intl.DateTimeFormat(undefined, {
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

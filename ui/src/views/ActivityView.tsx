import { useCallback, useEffect, useState, type ReactElement } from "react";

import type { Backend } from "../api";
import { text } from "../text";
import { clock } from "../time";
import type { LogEntry, LogTopic } from "../types";
import { Empty } from "./shared-parts";

type Filter = "all" | "rejections" | "temporals" | "alerts";

const FILTERS: Filter[] = ["all", "rejections", "temporals", "alerts"];

/** The window asked of the core and named in the screen: one constant, so they cannot differ. */
export const ACTIVITY_WINDOW_SECONDS = 24 * 3600;

const TOPIC: Record<Exclude<Filter, "all">, LogTopic> = {
  rejections: "rejection",
  temporals: "temporal",
  alerts: "alert",
};

/** Starting up is bookkeeping, not something the person did or needs to see. */
const isBookkeeping = (entry: LogEntry): boolean => entry.kind === "core_started";

/** Filters on the typed topics the core attaches, never on the sentence. */
function matches(entry: LogEntry, filter: Filter): boolean {
  return filter === "all" || (entry.topics ?? []).includes(TOPIC[filter]);
}

interface Label {
  word: string;
  tone: "green" | "amber" | "coral" | "blue" | "soft";
}

function label(entry: LogEntry): Label {
  switch (entry.kind) {
    case "run_rejected":
      return { word: "RECHAZO", tone: "blue" };
    case "run_accepted":
      return (entry.topics ?? []).includes("temporal")
        ? { word: "TEMPORAL", tone: "amber" }
        : { word: "PRINCIPAL", tone: "green" };
    case "lease_warned":
      return { word: "AVISO", tone: "amber" };
    case "lease_extended":
      return { word: "EXTENDIDO", tone: "amber" };
    case "lease_expired":
      return { word: "VENCIÓ", tone: "amber" };
    case "duplicate_detected":
    case "unregistered_seen":
      return { word: "SIN REGISTRAR", tone: "coral" };
    case "adopted":
      return { word: "ADOPTADO", tone: "green" };
    case "kept":
      return { word: "CONSERVADO", tone: "soft" };
    case "restored":
      return { word: "RESTAURADO", tone: "soft" };
    case "reclassified":
      return { word: "CLASIFICADO", tone: "soft" };
    case "stopped":
      return { word: "DETENIDO", tone: "soft" };
    case "stop_failed":
    case "port_stolen":
      return { word: "ALERTA", tone: "coral" };
    default:
      return { word: entry.kind.replaceAll("_", " ").toUpperCase(), tone: "soft" };
  }
}

const TONE_CLASS: Record<Label["tone"], string> = {
  green: "green-text",
  amber: "amber-text",
  coral: "coral-text",
  blue: "blue-text",
  soft: "dim",
};

/** What the screen knows about the log, for the section's subtitle. */
export type ActivityStatus =
  { kind: "loading" } | { kind: "error" } | { kind: "ready"; total: number };

export function activitySubtitle(status: ActivityStatus): string {
  switch (status.kind) {
    case "loading":
      return text.activity.loading;
    case "error":
      return text.activity.unreadable;
    case "ready":
      return text.activity.summary(status.total);
  }
}

/** A rejected call as text; the core's typed storage error keeps its own explanation. */
function failureText(error: unknown): string {
  if (typeof error === "object" && error !== null && "code" in error) {
    if (error.code === "closing") return text.activity.closing;
    if ("message" in error && typeof error.message === "string") return error.message;
  }
  return error instanceof Error ? error.message : String(error);
}

interface Loaded {
  /** Entries of the last successful read; kept while a later read fails. */
  entries: LogEntry[] | null;
  /** Why the latest read failed, if it did. */
  error: string | null;
}

export function ActivityView({
  api,
  revision,
  onStatus,
  filter,
  onFilterChange,
}: {
  api: Backend;
  revision: number;
  onStatus: (status: ActivityStatus) => void;
  filter: Filter;
  onFilterChange: (filter: Filter) => void;
}): ReactElement {
  const [loaded, setLoaded] = useState<Loaded>({ entries: null, error: null });
  const [attempt, setAttempt] = useState(0);
  const retry = useCallback(() => {
    setAttempt((value) => value + 1);
  }, []);

  useEffect(() => {
    let cancelled = false;
    api.log(ACTIVITY_WINDOW_SECONDS).then(
      (read) => {
        if (cancelled) return;
        // Oldest first from the core; newest first on screen.
        setLoaded({
          entries: [...read].filter((entry) => !isBookkeeping(entry)).reverse(),
          error: null,
        });
      },
      (error: unknown) => {
        if (!cancelled)
          setLoaded((previous) => ({ entries: previous.entries, error: failureText(error) }));
      },
    );
    return () => {
      cancelled = true;
    };
  }, [api, revision, attempt]);

  const { entries, error } = loaded;
  useEffect(() => {
    if (entries !== null) onStatus({ kind: "ready", total: entries.length });
    else onStatus(error === null ? { kind: "loading" } : { kind: "error" });
  }, [entries, error, onStatus]);

  const shown = (entries ?? []).filter((entry) => matches(entry, filter));
  const total = entries?.length ?? 0;
  return (
    <>
      <div className="pills" role="group" aria-label="Filtro">
        {FILTERS.map((name) => (
          <button
            key={name}
            type="button"
            className="pill"
            aria-pressed={filter === name}
            onClick={() => {
              onFilterChange(name);
            }}
          >
            {text.activity[name]}
          </button>
        ))}
      </div>
      {error !== null && (
        <div className="card coral" role="alert">
          <div className="soft">
            {entries === null ? text.activity.loadFailed : text.activity.staleFailed}
          </div>
          <div className="soft mono" style={{ marginTop: 4, fontSize: 11 }}>
            {error}
          </div>
          <div className="row" style={{ marginTop: 8, justifyContent: "flex-end" }}>
            <button type="button" className="btn" onClick={retry}>
              {text.activity.retry}
            </button>
          </div>
        </div>
      )}
      {entries === null && error === null && <p role="status">{text.activity.loading}</p>}
      {entries !== null && (
        <>
          <div className="soft" style={{ fontSize: 12 }}>
            {text.activity.window} ·{" "}
            {filter === "all"
              ? text.activity.summary(total)
              : text.activity.of(shown.length, total)}
          </div>
          {total === 0 ? (
            <Empty title={text.activity.noneWindow} body={text.activity.noneWindowBody} />
          ) : shown.length === 0 ? (
            <div className="card">
              <div className="soft">{text.activity.noMatch(total)}</div>
              <div className="row" style={{ marginTop: 8, justifyContent: "flex-end" }}>
                <button
                  type="button"
                  className="btn"
                  onClick={() => {
                    onFilterChange("all");
                  }}
                >
                  {text.activity.showAll}
                </button>
              </div>
            </div>
          ) : (
            <div className="divided">
              {shown.map((entry, index) => {
                const tag = label(entry);
                return (
                  <div className="log-row" key={`${entry.at}-${index}`}>
                    <span className="mono dim">{clock(entry.at)}</span>
                    <span className={`eyebrow ${TONE_CLASS[tag.tone]}`}>{tag.word}</span>
                    <span>{entry.summary}</span>
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}
    </>
  );
}

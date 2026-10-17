import { useEffect, useState, type ReactElement } from "react";

import type { Backend } from "../api";
import { text } from "../text";
import { clock } from "../time";
import type { LogEntry } from "../types";
import { Empty } from "./shared-parts";

type Filter = "all" | "rejections" | "temporals" | "alerts";

const FILTERS: Filter[] = ["all", "rejections", "temporals", "alerts"];

const TEMPORAL = ["lease_warned", "lease_extended", "lease_expired", "stop_failed"];
const ALERT = [
  "duplicate_detected",
  "unregistered_seen",
  "port_stolen",
  "stop_failed",
  "process_gone",
];

function matches(entry: LogEntry, filter: Filter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "rejections":
      return entry.kind === "run_rejected";
    case "temporals":
      return TEMPORAL.includes(entry.kind) || entry.summary.includes("Temporal");
    case "alerts":
      return ALERT.includes(entry.kind);
  }
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
      return entry.summary.includes("Temporal")
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

export function activitySubtitle(count: number | undefined): string {
  return text.activity.summary(count ?? 0);
}

export function ActivityView({
  api,
  revision,
  onCount,
}: {
  api: Backend;
  revision: number;
  onCount: (count: number) => void;
}): ReactElement {
  const [entries, setEntries] = useState<LogEntry[]>([]);
  const [filter, setFilter] = useState<Filter>("all");

  useEffect(() => {
    let cancelled = false;
    void api.log(24 * 3600).then((loaded) => {
      if (cancelled) return;
      // Starting up is bookkeeping, not something the person did or needs to see.
      setEntries([...loaded].filter((entry) => entry.kind !== "core_started").reverse());
      onCount(loaded.length);
    });
    return () => {
      cancelled = true;
    };
  }, [api, revision, onCount]);

  const shown = entries.filter((entry) => matches(entry, filter));
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
              setFilter(name);
            }}
          >
            {text.activity[name]}
          </button>
        ))}
      </div>
      {shown.length === 0 ? (
        <Empty title={text.activity.none} body="" />
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
  );
}

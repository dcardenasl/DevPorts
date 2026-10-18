import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { Backend } from "../api";
import { sampleLog } from "../sample";
import type { LogEntry } from "../types";
import { ACTIVITY_WINDOW_SECONDS, ActivityView, type ActivityStatus } from "./ActivityView";

type Read = () => Promise<LogEntry[]>;

function setup(read: Read, filter: "all" | "rejections" | "temporals" | "alerts" = "all") {
  const log = vi.fn(() => read());
  const api = { log } as unknown as Backend;
  const statuses: ActivityStatus[] = [];
  const onFilterChange = vi.fn();
  const view = (revision: number, current = filter) => (
    <ActivityView
      api={api}
      revision={revision}
      onStatus={(status) => statuses.push(status)}
      filter={current}
      onFilterChange={onFilterChange}
    />
  );
  const rendered = render(view(1));
  return { ...rendered, log, statuses, onFilterChange, view };
}

/** What Tauri rejects with for the core's typed storage error. */
const storage = Object.assign(new Error("disk I/O error"), { code: "storage" });

// covers: UXR-6.3
describe("the activity view", () => {
  it("asks for the window it names and shows loading before the first read", async () => {
    let finish: (entries: LogEntry[]) => void = () => undefined;
    const pending = new Promise<LogEntry[]>((resolve) => {
      finish = resolve;
    });
    const { log, statuses } = setup(() => pending);
    expect(log).toHaveBeenCalledWith(ACTIVITY_WINDOW_SECONDS);
    expect(ACTIVITY_WINDOW_SECONDS).toBe(86_400);
    expect(screen.getByRole("status")).toHaveTextContent("Cargando actividad…");
    expect(screen.queryByText(/Sin actividad/)).toBeNull();
    finish(sampleLog());
    expect(await screen.findByText(/Últimas 24 h/)).toBeInTheDocument();
    expect(statuses[0]).toEqual({ kind: "loading" });
  });

  it("counts only what it shows: startup is bookkeeping and the filter reports a subtotal", async () => {
    const { statuses, rerender, view } = setup(() => Promise.resolve(sampleLog()));
    expect(await screen.findByText(/Últimas 24 h · 4 eventos/)).toBeInTheDocument();
    expect(screen.queryByText("DevPorts se inició")).toBeNull();
    expect(statuses.at(-1)).toEqual({ kind: "ready", total: 4 });
    rerender(view(1, "rejections"));
    expect(await screen.findByText(/1 de 4 eventos/)).toBeInTheDocument();
    expect(statuses.at(-1)).toEqual({ kind: "ready", total: 4 });
    expect(screen.getAllByText(/otra instancia de api/)).toHaveLength(1);
  });

  it("filters on typed topics, never on the words of the sentence", async () => {
    const entries = [
      ...sampleLog(),
      {
        at: new Date().toISOString(),
        kind: "adopted",
        project: "shop",
        service: "api",
        summary: "Adoptaste api; antes era un Temporal de prueba",
        topics: [],
      } satisfies LogEntry,
    ];
    setup(() => Promise.resolve(entries), "temporals");
    expect(await screen.findByText(/1 de 5 eventos/)).toBeInTheDocument();
    expect(screen.queryByText(/Adoptaste api/)).toBeNull();
    expect(screen.getByText(/para «QA login»/)).toBeInTheDocument();
  });

  it("a failed read is an error with its reason and a retry, never an empty list", async () => {
    const reads: Read[] = [() => Promise.reject(storage), () => Promise.resolve(sampleLog())];
    const { statuses } = setup(() => (reads.shift() as Read)());
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText(/No se pudo leer la actividad/)).toBeInTheDocument();
    expect(within(alert).getByText("disk I/O error")).toBeInTheDocument();
    expect(screen.queryByText(/Sin actividad en las últimas 24 h/)).toBeNull();
    expect(statuses.at(-1)).toEqual({ kind: "error" });
    await userEvent.click(within(alert).getByRole("button", { name: "Reintentar" }));
    expect(await screen.findByText(/4 eventos/)).toBeInTheDocument();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps the last good read on screen when a refresh fails", async () => {
    const reads: Read[] = [() => Promise.resolve(sampleLog()), () => Promise.reject(storage)];
    const { rerender, view } = setup(() => (reads.shift() as Read)());
    await screen.findByText(/4 eventos/);
    rerender(view(2));
    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText(/se muestra la última lectura/)).toBeInTheDocument();
    expect(screen.getByText(/otra instancia de api/)).toBeInTheDocument();
  });

  it("tells a really empty window from a filter that matches nothing", async () => {
    const startupOnly = [sampleLog()[0] as LogEntry];
    const first = setup(() => Promise.resolve(startupOnly));
    expect(await screen.findByText("Sin actividad en las últimas 24 h")).toBeInTheDocument();
    first.unmount();

    const second = setup(
      () => Promise.resolve(sampleLog().filter((entry) => entry.kind !== "run_rejected")),
      "rejections",
    );
    expect(
      await screen.findByText(/Ningún evento coincide con este filtro \(hay 3 eventos/),
    ).toBeInTheDocument();
    expect(screen.queryByText("Sin actividad en las últimas 24 h")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Ver todo" }));
    expect(second.onFilterChange).toHaveBeenCalledWith("all");
    await waitFor(() => {
      expect(second.log).toHaveBeenCalled();
    });
  });
});

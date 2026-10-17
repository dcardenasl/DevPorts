import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Backend } from "./api";
import { App } from "./App";
import { sampleLog, sampleSnapshot } from "./sample";
import type { InstanceView, Snapshot } from "./types";

const state = vi.hoisted(() => ({ current: undefined as Backend | undefined }));

vi.mock("./api", () => ({
  backend: () => Promise.resolve(state.current),
}));

function fakeBackend(snapshot: Snapshot): Backend & { calls: Record<string, unknown[][]> } {
  const calls: Record<string, unknown[][]> = {};
  const record =
    (name: string, result?: unknown) =>
    (...args: unknown[]) => {
      (calls[name] ??= []).push(args);
      return Promise.resolve(result);
    };
  return {
    calls,
    snapshot: () => Promise.resolve(snapshot),
    subscribe: () => Promise.resolve(() => undefined),
    stop: record("stop") as Backend["stop"],
    extend: record("extend") as Backend["extend"],
    keep: record("keep") as Backend["keep"],
    adopt: record("adopt", { project: "x", service: "y" }) as Backend["adopt"],
    log: () => Promise.resolve(sampleLog()),
    openUrl: record("openUrl") as Backend["openUrl"],
    panelReady: record("panelReady") as Backend["panelReady"],
  };
}

async function open(snapshot: Snapshot = sampleSnapshot()) {
  const backend = fakeBackend(snapshot);
  state.current = backend;
  const user = userEvent.setup();
  render(<App />);
  await screen.findByRole("heading", { level: 1 });
  return { backend, user };
}

const tab = (name: RegExp) => screen.getByRole("tab", { name });

describe("the panel", () => {
  beforeEach(() => {
    state.current = undefined;
  });

  // covers: UI
  it("opens on Atención with the counter, the official instance and every alert", async () => {
    await open();
    expect(screen.getByRole("heading", { level: 1, name: "Atención" })).toBeInTheDocument();
    expect(screen.getByText("3 cosas esperan tu decisión")).toBeInTheDocument();
    expect(tab(/Atención/)).toHaveTextContent("3");
    expect(screen.getByText("OFICIAL")).toBeInTheDocument();
    expect(screen.getByText("TEMPORAL POR VENCER")).toBeInTheDocument();
    expect(screen.getByText(/0:4\d|0:5\d/)).toBeInTheDocument(); // the countdown
    expect(screen.getByRole("button", { name: "Detener los 2" })).toBeInTheDocument();
  });

  // covers: UI
  it("tells the core what the person decides on an alert", async () => {
    const { backend, user } = await open();
    await user.click(screen.getByRole("button", { name: "Extender 30 min" }));
    await user.click(screen.getByRole("button", { name: "Detener los 2" }));
    await user.click(screen.getByRole("button", { name: "Terminar" }));
    await waitFor(() => {
      expect(backend.calls["extend"]).toEqual([["c3d4"]]);
      expect(backend.calls["stop"]).toEqual([["e5f6"], ["g7h8"], ["c3d4"]]);
    });
  });

  // covers: UI
  it("says everything is in order when nothing waits", async () => {
    await open({ ...sampleSnapshot(), attention: [] });
    expect(screen.getAllByText("Todo en orden").length).toBeGreaterThan(0);
    expect(tab(/Atención/)).not.toHaveTextContent(/\d/);
  });

  // covers: UI
  it("lays projects out as repos and services, one project open at a time", async () => {
    const { user } = await open();
    await user.click(tab(/Proyectos/));
    expect(screen.getByText("2 repos · 2 servicios")).toBeInTheDocument();
    expect(screen.getByText("tienda-api")).toBeInTheDocument();
    expect(screen.getByText("tienda-admin")).toBeInTheDocument();
    expect(screen.getByText("Usa")).toBeInTheDocument();
    expect(screen.getByText("mysql")).toBeInTheDocument();
    // The temporal hangs under its service, with its reason.
    expect(screen.getByText("QA login")).toBeInTheDocument();
    // Opening the other project folds this one.
    await user.click(screen.getByRole("button", { name: /ci4-kickstart/ }));
    expect(screen.queryByText("tienda-api")).not.toBeInTheDocument();
    expect(screen.getByText("ci4-kickstart", { selector: ".mono" })).toBeInTheDocument();
  });

  // covers: UI
  it("asks before stopping a shared service and names the projects that lose it", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Compartidos/));
    await user.click(screen.getByRole("button", { name: "Detener mysql" }));
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByText("¿Detener mysql?")).toBeInTheDocument();
    expect(within(dialog).getByText("tienda-demo")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(backend.calls["stop"]).toBeUndefined();

    await user.click(screen.getByRole("button", { name: "Detener mysql" }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Detener mysql" }),
    );
    await waitFor(() => {
      expect(backend.calls["stop"]).toEqual([["m9n0"]]);
    });
  });

  // covers: UI
  it("adopts a loose process into a new project through the sheet", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    expect(screen.getByText("1 proceso sin dueño")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Adoptar" }));
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    const name = screen.getByLabelText("Nombre del proyecto");
    expect(name).toHaveValue("scratch");
    await user.clear(name);
    await user.type(name, "mi-proyecto");
    await user.click(screen.getByRole("button", { name: "Crear" }));
    await waitFor(() => {
      expect(backend.calls["adopt"]).toEqual([["z9y8", { new: "mi-proyecto" }]]);
    });
  });

  // covers: UI
  it("keeps a process the person chooses to ignore", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    await user.click(screen.getByRole("button", { name: "Adoptar" }));
    await user.click(screen.getByRole("button", { name: /Ignorar este proceso/ }));
    await waitFor(() => {
      expect(backend.calls["keep"]).toEqual([["z9y8"]]);
    });
  });

  // covers: UI
  it("filters the activity and leaves startup noise out", async () => {
    const { user } = await open();
    await user.click(tab(/Actividad/));
    expect(await screen.findByText(/otra instancia de api/)).toBeInTheDocument();
    expect(screen.queryByText("DevPorts se inició")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Rechazos" }));
    expect(screen.getByText(/otra instancia de api/)).toBeInTheDocument();
    expect(screen.queryByText(/duplicado de api/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Alertas" }));
    expect(screen.getByText(/duplicado de api/)).toBeInTheDocument();
  });

  // covers: UI
  it("explains the empty states instead of showing blank tabs", async () => {
    const { user } = await open({
      ...sampleSnapshot(),
      instances: [],
      projects: [],
      attention: [],
    });
    await user.click(tab(/Proyectos/));
    expect(screen.getByText("Todavía no hay proyectos")).toBeInTheDocument();
    await user.click(tab(/Compartidos/));
    expect(screen.getByText("No hay servicios compartidos")).toBeInTheDocument();
    await user.click(tab(/Sin proyecto/));
    expect(screen.getAllByText("Nada suelto")).toHaveLength(2);
  });

  // covers: UI
  it("lists undeclared database containers and declares them in one click", async () => {
    const snapshot = sampleSnapshot();
    snapshot.instances.push({
      ...(snapshot.instances[0] as InstanceView),
      ref: "d1d1",
      role: "unregistered",
      project: null,
      service: null,
      kind: "data",
      container: "mini-redis",
      image: "redis:7",
      shared: false,
      duplicate_of: null,
      registered: false,
      cwd: null,
      argv: [],
      port: 6379,
    });
    const { backend, user } = await open(snapshot);
    await user.click(tab(/Compartidos/));
    expect(screen.getByText("SIN DECLARAR")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Declarar" }));
    await waitFor(() => {
      expect(backend.calls["adopt"]).toEqual([["d1d1", "shared"]]);
    });
  });
});

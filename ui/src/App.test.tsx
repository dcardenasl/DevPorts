import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Backend } from "./api";
import { App } from "./App";
import { sampleLog, sampleSnapshot } from "./sample";
import type { InstanceView, OperationRequest, OperationView, PanelState, Snapshot } from "./types";

const state = vi.hoisted(() => ({ current: undefined as Backend | undefined }));

vi.mock("./api", () => ({
  backend: () => Promise.resolve(state.current),
}));

function fakeBackend(
  snapshot: Snapshot,
  initialState?: PanelState,
  initialOperations: OperationView[] = [],
): Backend & { calls: Record<string, unknown[][]>; getPersisted: () => PanelState } {
  const calls: Record<string, unknown[][]> = {};
  let persisted: PanelState = initialState ?? {
    version: 1,
    tab: "attention",
    expandedProject: null,
    expandedProjectSet: false,
    activityFilter: "all",
    draft: null,
  };
  const history: OperationView[] = [...initialOperations];
  const opListeners = new Set<(operation: OperationView) => void>();
  const record =
    (name: string, result?: unknown) =>
    (...args: unknown[]) => {
      (calls[name] ??= []).push(args);
      return Promise.resolve(result);
    };
  return {
    calls,
    getPersisted: () => persisted,
    snapshot: () => Promise.resolve(snapshot),
    panelState: () => Promise.resolve(persisted),
    savePanelState: (next) => {
      persisted = next;
      return Promise.resolve();
    },
    operations: () => Promise.resolve([...history]),
    operation: (id) => Promise.resolve(history.find((item) => item.id === id) ?? null),
    subscribeOperations: (listener) => {
      opListeners.add(listener);
      return Promise.resolve(() => {
        opListeners.delete(listener);
      });
    },
    startOperation: async (request: OperationRequest) => {
      await Promise.resolve();
      if (calls.operation === undefined) calls.operation = [];
      calls.operation.push([request]);
      const op: OperationView = {
        id: request.id,
        kind: request.kind,
        status: "succeeded",
        revision: 1,
        targets: request.targets.map((target) => ({
          reference: target.reference,
          adoptTarget: target.adoptTarget ?? null,
          status: "succeeded",
          error: null,
          result: request.kind === "adopt" ? { project: "x", service: "y" } : null,
          attempts: 1,
        })),
      };
      history.unshift(op);
      opListeners.forEach((listener) => {
        listener(op);
      });
      return op;
    },
    retryOperation: async (id) => {
      calls.retryOperation = [[id]];
      await Promise.resolve();
      const operation = history.find((item) => item.id === id);
      if (!operation) throw new Error("No existe la operación");
      operation.status = "succeeded";
      operation.revision += 1;
      operation.targets.forEach((target) => {
        if (target.status === "failed") {
          target.status = "succeeded";
          target.error = null;
        }
      });
      opListeners.forEach((listener) => {
        listener(operation);
      });
      return operation;
    },
    subscribe: () => Promise.resolve(() => undefined),
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
  await screen.findByRole("tab", { name: /Atención/ });
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
      const ops = backend.calls.operation?.map(([request]) => request as OperationRequest) ?? [];
      expect(
        ops.map(({ kind, targets }) => [kind, targets.map(({ reference }) => reference)]),
      ).toEqual([
        ["extend", ["c3d4"]],
        ["stop", ["e5f6", "g7h8"]],
        ["stop", ["c3d4"]],
      ]);
    });
  });

  // covers: UXR-1.3
  it("recovers a failed operation from shell history and retries only on request", async () => {
    const failedOperation: OperationView = {
      id: "recover-me",
      kind: "adopt",
      status: "failed",
      revision: 2,
      targets: [
        {
          reference: "z9y8",
          adoptTarget: { project: "tienda" },
          status: "failed",
          error: "No se pudo escribir el manifiesto",
          result: null,
          attempts: 1,
        },
      ],
    };
    const backend = fakeBackend(sampleSnapshot(), undefined, [failedOperation]);
    state.current = backend;
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText(/No se pudo escribir el manifiesto/)).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
    await user.click(screen.getByRole("button", { name: "Reintentar fallidos" }));
    await waitFor(() => {
      expect(backend.calls.retryOperation).toEqual([["recover-me"]]);
    });
    expect(screen.getByText(/z9y8: Completado/)).toBeInTheDocument();
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
    expect(backend.calls.operation).toBeUndefined();

    await user.click(screen.getByRole("button", { name: "Detener mysql" }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Detener mysql" }),
    );
    await waitFor(() => {
      expect(
        backend.calls.operation
          ?.map(([request]) => request as OperationRequest)
          .map(({ kind, targets }) => [kind, targets[0]?.reference]),
      ).toEqual([["stop", "m9n0"]]);
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
      expect(
        backend.calls.operation
          ?.map(([request]) => request as OperationRequest)
          .map(({ kind, targets }) => [kind, targets[0]?.reference, targets[0]?.adoptTarget]),
      ).toEqual([["adopt", "z9y8", { new: "mi-proyecto" }]]);
    });
  });

  // covers: UI
  it("keeps a process the person chooses to ignore", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    await user.click(screen.getByRole("button", { name: "Adoptar" }));
    await user.click(screen.getByRole("button", { name: /Ignorar este proceso/ }));
    await waitFor(() => {
      expect(
        backend.calls.operation
          ?.map(([request]) => request as OperationRequest)
          .map(({ kind, targets }) => [kind, targets[0]?.reference]),
      ).toEqual([["keep", "z9y8"]]);
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
      expect(
        backend.calls.operation
          ?.map(([request]) => request as OperationRequest)
          .map(({ kind, targets }) => [kind, targets[0]?.reference, targets[0]?.adoptTarget]),
      ).toEqual([["adopt", "d1d1", "shared"]]);
    });
  });

  // covers: UI, UXR-1.1
  it("cancels a shared stop with Escape, traps Tab and restores its invoker", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Compartidos/));
    const invoker = screen.getByRole("button", { name: "Detener mysql" });
    await user.click(invoker);
    const dialog = screen.getByRole("alertdialog");
    const cancel = within(dialog).getByRole("button", { name: "Cancelar" });
    const confirm = within(dialog).getByRole("button", { name: "Detener mysql" });
    expect(cancel).toHaveFocus();
    await user.tab({ shift: true });
    expect(confirm).toHaveFocus();
    await user.tab();
    expect(cancel).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(invoker).toHaveFocus();
    expect(backend.calls["stop"]).toBeUndefined();
  });

  // covers: UI, UXR-1.1
  it("returns from both adoption steps without losing the draft or writing anything", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    const invoker = screen.getByRole("button", { name: "Adoptar" });
    await user.click(invoker);
    const dialog = screen.getByRole("dialog");
    expect(dialog.contains(document.activeElement)).toBe(true);
    const create = within(dialog).getByRole("button", { name: /Crear un proyecto nuevo/ });
    await user.click(create);
    const input = screen.getByLabelText("Nombre del proyecto");
    expect(input).toHaveFocus();
    await user.clear(input);
    await user.type(input, "mi borrador");
    await user.click(screen.getByRole("button", { name: "Volver" }));
    expect(screen.getByRole("button", { name: /Crear un proyecto nuevo/ })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: /Otro proyecto/ }));
    expect(screen.getByRole("button", { name: "Como servicio de tienda-demo" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Volver" }));
    expect(screen.getByRole("button", { name: /Otro proyecto/ })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    expect(screen.getByLabelText("Nombre del proyecto")).toHaveValue("mi borrador");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(invoker).toHaveFocus();
    expect(backend.calls["adopt"]).toBeUndefined();
    expect(backend.calls["keep"]).toBeUndefined();
    expect(backend.calls["stop"]).toBeUndefined();
  });

  // covers: UI, UXR-1.1
  it("keeps keyboard navigation inside the adoption form and cancels without submitting", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    const invoker = screen.getByRole("button", { name: "Adoptar" });
    await user.click(invoker);
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    const input = screen.getByLabelText("Nombre del proyecto");
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "Cancelar" })).toHaveFocus();
    await user.tab();
    expect(input).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Cancelar" }));
    expect(invoker).toHaveFocus();
    expect(backend.calls["adopt"]).toBeUndefined();
    expect(backend.calls["keep"]).toBeUndefined();
  });

  // covers: UI, UXR-1.2
  it("restores navigation and the activity filter after the panel is recreated", async () => {
    const backend = fakeBackend(sampleSnapshot());
    state.current = backend;
    const user = userEvent.setup();
    const first = render(<App />);
    await screen.findByRole("tab", { name: /Atención/ });
    await user.click(tab(/Proyectos/));
    await user.click(screen.getByRole("button", { name: /ci4-kickstart/ }));
    await user.click(tab(/Actividad/));
    await user.click(screen.getByRole("button", { name: "Alertas" }));
    await waitFor(() => {
      expect(backend.getPersisted().activityFilter).toBe("alerts");
    });
    first.unmount();

    render(<App />);
    await waitFor(() => expect(tab(/Actividad/)).toHaveAttribute("aria-selected", "true"));
    expect(screen.getByRole("button", { name: "Alertas" })).toHaveAttribute("aria-pressed", "true");
    await user.click(tab(/Proyectos/));
    expect(screen.queryByText("tienda-api")).not.toBeInTheDocument();
    expect(screen.getByText("ci4-kickstart", { selector: ".mono" })).toBeInTheDocument();
    cleanup();
  });

  // covers: UI, UXR-1.2
  it("restores an adoption draft without restoring a destructive confirmation", async () => {
    const backend = fakeBackend(sampleSnapshot(), {
      version: 1,
      tab: "orphans",
      expandedProject: null,
      expandedProjectSet: true,
      activityFilter: "all",
      draft: {
        kind: "adopt",
        reference: "z9y8",
        mode: "new",
        name: "unfinished-project",
        lastStep: "new",
      },
    });
    state.current = backend;
    const user = userEvent.setup();
    render(<App />);
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByLabelText("Nombre del proyecto")).toHaveValue("unfinished-project");
    expect(backend.calls["stop"]).toBeUndefined();
    expect(backend.calls["adopt"]).toBeUndefined();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(backend.getPersisted().draft).toBeNull();
    });
  });

  // covers: UI, UXR-1.2
  it("discards and explains a draft whose process no longer exists", async () => {
    const backend = fakeBackend(sampleSnapshot(), {
      version: 1,
      tab: "orphans",
      expandedProject: null,
      expandedProjectSet: true,
      activityFilter: "all",
      draft: {
        kind: "adopt",
        reference: "gone",
        mode: "new",
        name: "unfinished-project",
        lastStep: null,
      },
    });
    state.current = backend;
    render(<App />);
    expect(await screen.findByText(/ya no está disponible/)).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(backend.getPersisted().draft).toBeNull();
    });
    expect(backend.calls["stop"]).toBeUndefined();
    expect(backend.calls["adopt"]).toBeUndefined();
  });
});

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Backend } from "./api";
import { App } from "./App";
import { mockAdoption } from "./mock-adoption";
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
  let currentSnapshot = snapshot;
  const listeners = new Set<(snapshot: Snapshot) => void>();
  const adoption = mockAdoption(
    () => currentSnapshot,
    (next) => {
      currentSnapshot = { ...next, revision: currentSnapshot.revision + 1 };
      listeners.forEach((listener) => {
        listener(currentSnapshot);
      });
    },
  );
  const record =
    (name: string, result?: unknown) =>
    (...args: unknown[]) => {
      (calls[name] ??= []).push(args);
      return Promise.resolve(result);
    };
  return {
    calls,
    getPersisted: () => persisted,
    snapshot: () => Promise.resolve(currentSnapshot),
    adoptionOptions: adoption.adoptionOptions,
    previewAdoption: (reference, target) => {
      void record("previewAdoption")(reference, target);
      return adoption.previewAdoption(reference, target);
    },
    validateAdoption: adoption.validateAdoption,
    previewProjectStop: (project) => {
      void record("previewProjectStop")(project);
      return Promise.resolve(
        snapshot.instances.filter(
          (view) =>
            view.project === project &&
            view.kind === "app" &&
            !view.shared &&
            ["healthy", "unhealthy", "forgotten"].includes(view.state),
        ),
      );
    },
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
          stopExpectation: target.stopExpectation ?? null,
          status: "succeeded",
          error: null,
          result:
            request.kind === "adopt" && target.adoption
              ? adoption.apply(target.adoption.id, target.adoption.edits)
              : null,
          adoptionReview: target.adoption?.id ?? null,
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
    subscribe: (listener) => {
      listeners.add(listener);
      return Promise.resolve(() => {
        listeners.delete(listener);
      });
    },
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
    // jsdom has no layout engine; highlighting a card scrolls it into view.
    Element.prototype.scrollIntoView = vi.fn();
  });

  // covers: UI
  it("opens on Atención with the counter, the official instance and every alert", async () => {
    await open();
    expect(screen.getByRole("heading", { level: 1, name: "Atención" })).toBeInTheDocument();
    expect(screen.getByText("3 cosas esperan tu decisión")).toBeInTheDocument();
    expect(tab(/Atención/)).toHaveTextContent("3");
    expect(screen.getByText("OFICIAL")).toBeInTheDocument();
    expect(screen.getByText("TEMPORAL POR VENCER")).toBeInTheDocument();
    expect(screen.getByText(/^0:[45]\d$/)).toBeInTheDocument(); // the countdown
    expect(screen.getByRole("button", { name: "Detener los 2" })).toBeInTheDocument();
  });

  // covers: UI
  it("tells the core what the person decides on an alert", async () => {
    const { backend, user } = await open();
    await user.click(screen.getByRole("button", { name: "Extender 30 min" }));
    await user.click(screen.getByRole("button", { name: "Detener los 2" }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", {
        name: "Detener seleccionados",
      }),
    );
    await user.click(screen.getByRole("button", { name: "Terminar" }));
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Detener api" }),
    );
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

  // covers: UXR-2.3
  it("uses actual health in official context and preserves unknown shared states", async () => {
    const snapshot = sampleSnapshot();
    snapshot.instances = snapshot.instances.map((view) =>
      view.ref === "a1b2"
        ? { ...view, state: "unhealthy" }
        : view.ref === "m9n0"
          ? { ...view, state: "unknown", ports: [] }
          : view,
    );
    const { user } = await open(snapshot);
    const official = screen.getByText("OFICIAL").closest(".card");
    expect(official?.querySelector(".soft.mono")).toHaveTextContent("Colgada · no responde");
    expect(official?.querySelector(".soft.mono")).not.toHaveTextContent("Sana");
    await user.click(tab(/Compartidos/));
    expect(
      screen.getByText(/Desconocido · sin observación actual/, { selector: ".soft.mono" }),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Detener mysql" })).not.toBeInTheDocument();
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
    expect(within(dialog).getByText(/Contenedor mysql/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Puerto 3306/)).toBeInTheDocument();
    expect(within(dialog).getByText("Recurso compartido")).toBeInTheDocument();
    expect(
      within(dialog).getByText(
        "Se detendrá el contenedor de datos. DevPorts no borrará el contenedor ni sus volúmenes.",
      ),
    ).toBeInTheDocument();
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

  // covers: UXR-2.1
  it("requires a fresh confirmation when a target changes while its dialog is open", async () => {
    const snapshot = sampleSnapshot();
    const { backend, user } = await open(snapshot);
    await user.click(tab(/Compartidos/));
    await user.click(screen.getByRole("button", { name: "Detener mysql" }));
    const mysql = snapshot.instances.find((view) => view.ref === "m9n0");
    if (mysql === undefined) throw new Error("mysql fixture missing");
    snapshot.instances = snapshot.instances.map((view) =>
      view.ref === "m9n0" ? { ...view, port: 3307 } : view,
    );
    await user.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Detener mysql" }),
    );
    expect(backend.calls.operation).toBeUndefined();
    expect(screen.getByText(/La instancia cambió desde la confirmación/)).toBeInTheDocument();
  });

  // covers: UXR-2.1
  it("offers an unregistered server a separate confirmed stop beside adoption", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    expect(screen.getByRole("button", { name: "Adoptar" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Detener" }));
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByText(/Proceso pid 6012/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Puerto 7000/)).toBeInTheDocument();
    expect(within(dialog).getByText("Sin proyecto")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UXR-2.2
  it("previews an exact project stop, preserves data and does not include later arrivals", async () => {
    const snapshot = sampleSnapshot();
    const original = snapshot.instances.find((view) => view.ref === "a1b2");
    if (original === undefined) throw new Error("principal fixture missing");
    snapshot.instances.push({ ...original, ref: "local-db", kind: "data", service: "local-db" });
    const { backend, user } = await open(snapshot);
    await user.click(tab(/Proyectos/));
    const invoke = screen.getByRole("button", { name: "Detener servicios del proyecto" });
    await user.click(invoke);
    let dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("¿Detener aplicaciones de tienda-demo?")).toBeInTheDocument();
    expect(within(dialog).queryByText("mysql")).not.toBeInTheDocument();
    expect(within(dialog).queryByText("local-db")).not.toBeInTheDocument();
    expect(
      within(dialog).getByText(/Los datos y compartidos seguirán activos/),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(backend.calls.operation).toBeUndefined();
    await user.click(invoke);
    dialog = await screen.findByRole("alertdialog");
    const expected = snapshot.instances
      .filter(
        (view) =>
          view.project === "tienda-demo" &&
          view.kind === "app" &&
          !view.shared &&
          view.state !== "down",
      )
      .map((view) => view.ref);
    snapshot.instances.push({ ...original, ref: "new-arrival", pid: 99999 });
    // Send original expectations to the core: one stale member must not suppress other outcomes.
    snapshot.instances = snapshot.instances.map((view) =>
      view.ref === "a1b2" ? { ...view, project: "moved-project" } : view,
    );
    await user.click(within(dialog).getByRole("button", { name: "Detener seleccionados" }));
    await waitFor(() => {
      const request = backend.calls.operation?.[0]?.[0] as OperationRequest | undefined;
      expect(request?.kind).toBe("stopProject");
      expect(request?.targets.map((target) => target.reference)).toEqual(expected);
      expect(
        request?.targets.every((target) => target.stopExpectation?.project === "tienda-demo"),
      ).toBe(true);
    });
  });

  // covers: UXR-2.2
  it("handles empty project previews and read failures without sending a stop", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Proyectos/));
    backend.previewProjectStop = () => Promise.resolve([]);
    await user.click(screen.getByRole("button", { name: "Detener servicios del proyecto" }));
    expect(
      await screen.findByText(/No hay aplicaciones activas que se puedan detener/),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    backend.previewProjectStop = () => Promise.reject(new Error("núcleo no disponible"));
    await user.click(screen.getByRole("button", { name: "Detener servicios del proyecto" }));
    expect(await screen.findByText("núcleo no disponible")).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UXR-2.2
  it("recovers partial project results and retries the stored operation without a new selection", async () => {
    const result: OperationView = {
      id: "project-partial",
      kind: "stopProject",
      status: "partial",
      revision: 4,
      targets: [
        {
          reference: "a1b2",
          adoptTarget: null,
          status: "succeeded",
          error: null,
          result: null,
          attempts: 1,
        },
        {
          reference: "c3d4",
          adoptTarget: null,
          status: "failed",
          error: "La instancia cambió; revisa el objetivo",
          result: null,
          attempts: 1,
        },
      ],
    };
    const backend = fakeBackend(sampleSnapshot(), undefined, [result]);
    state.current = backend;
    const user = userEvent.setup();
    render(<App />);
    expect(await screen.findByText(/a1b2: Completado/)).toBeInTheDocument();
    expect(screen.getByText(/c3d4: La instancia cambió/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reintentar fallidos" }));
    await waitFor(() => {
      expect(backend.calls.retryOperation).toEqual([["project-partial"]]);
    });
    expect(backend.calls.operation).toBeUndefined();
    expect(backend.calls.previewProjectStop).toBeUndefined();
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
    await user.click(screen.getByRole("button", { name: "Revisar" }));
    expect(backend.calls.operation).toBeUndefined();
    await screen.findByLabelText("Nombre del servicio");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(
        backend.calls.operation
          ?.map(([request]) => request as OperationRequest)
          .map(({ kind, targets }) => [kind, targets[0]?.reference, targets[0]?.adoptTarget]),
      ).toEqual([["adopt", "z9y8", { new: "mi-proyecto" }]]);
    });
  });

  // covers: UI, UXR-3.2
  it("keeps the edited fields after a rejected value and sends only the corrected review", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    await user.click(screen.getByRole("button", { name: "Adoptar" }));
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    await user.click(screen.getByRole("button", { name: "Revisar" }));
    const name = await screen.findByLabelText("Nombre del servicio");
    await user.clear(name);
    await user.type(name, "Nombre Malo");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    expect(await screen.findByText(/nombre de servicio válido/)).toBeInTheDocument();
    expect(screen.getByLabelText("Nombre del servicio")).toHaveValue("Nombre Malo");
    expect(backend.calls.operation).toBeUndefined();
    await user.clear(screen.getByLabelText("Nombre del servicio"));
    await user.type(screen.getByLabelText("Nombre del servicio"), "web");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(backend.calls.operation).toHaveLength(1);
    });
    const sent = (backend.calls.operation?.[0]?.[0] as OperationRequest).targets[0];
    expect(sent?.adoption?.edits.name).toBe("web");
  });

  // covers: UI, UXR-3.2
  it("recovers the saved result after the panel is recreated and links to the new card", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    await user.click(screen.getByRole("button", { name: "Adoptar" }));
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    await user.click(screen.getByRole("button", { name: "Revisar" }));
    await screen.findByLabelText("Nombre del servicio");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(backend.calls.operation).toHaveLength(1);
    });
    cleanup();

    render(<App />);
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText(/Agregado a/)).toBeInTheDocument();
    expect(backend.calls.operation).toHaveLength(1);
    await user.click(within(dialog).getByRole("button", { name: "Ver proyecto" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(tab(/Proyectos/)).toHaveAttribute("aria-selected", "true");
    expect(backend.calls.operation).toHaveLength(1);
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
    await user.click(await screen.findByRole("button", { name: "Como servicio compartido" }));
    expect(backend.calls.operation).toBeUndefined();
    await screen.findByLabelText("Nombre del servicio");
    await user.click(screen.getByRole("button", { name: "Guardar" }));
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
    expect(backend.calls.operation).toBeUndefined();
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
    expect(screen.getByRole("button", { name: "Como servicio de tienda-demo" })).toBeDisabled();
    expect(screen.getAllByText(/fuera de la raíz/).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: "Volver" }));
    expect(screen.getByRole("button", { name: /Otro proyecto/ })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: /Crear un proyecto nuevo/ }));
    expect(screen.getByLabelText("Nombre del proyecto")).toHaveValue("mi borrador");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(invoker).toHaveFocus();
    expect(backend.calls["adopt"]).toBeUndefined();
    expect(backend.calls["keep"]).toBeUndefined();
    expect(backend.calls.operation).toBeUndefined();
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
    expect(backend.calls.operation).toBeUndefined();
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
    expect(backend.calls.operation).toBeUndefined();
    expect(backend.calls["adopt"]).toBeUndefined();
  });
});

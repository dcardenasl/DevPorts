import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Backend } from "./api";
import { App } from "./App";
import { mockAdoption } from "./mock-adoption";
import { mockClassify } from "./mock-classify";
import { mockConsumers } from "./mock-consumers";
import { mockIgnored } from "./mock-ignored";
import { mockPreferences } from "./mock-preferences";
import { mockSettings } from "./mock-settings";
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
    version: 2,
    tab: "attention",
    openProjects: [],
    closedProjects: [],
    activityFilter: "all",
    draft: null,
  };
  const history: OperationView[] = [...initialOperations];
  const opListeners = new Set<(operation: OperationView) => void>();
  let currentSnapshot = snapshot;
  const listeners = new Set<(snapshot: Snapshot) => void>();
  const publish = (next: Snapshot): void => {
    currentSnapshot = { ...next, revision: currentSnapshot.revision + 1 };
    listeners.forEach((listener) => {
      listener(currentSnapshot);
    });
  };
  const adoption = mockAdoption(() => currentSnapshot, publish);
  const consumers = mockConsumers(() => currentSnapshot, publish);
  const ignored = mockIgnored(() => currentSnapshot, publish);
  const classify = mockClassify(() => currentSnapshot, publish);
  const settings = mockSettings();
  const preferences = mockPreferences();
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
    sharedConsumers: consumers.sharedConsumers,
    rulePreview: ignored.rulePreview,
    ignoredRules: ignored.ignoredRules,
    preferences: preferences.preferences,
    setPreferences: preferences.setPreferences,
    scanFolders: settings.scanFolders,
    previewScanFolders: settings.previewScanFolders,
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
      const settled =
        request.kind === "setConsumers"
          ? consumers.settle(request)
          : request.kind === "setScanFolders"
            ? settings.settle(request)
            : [];
      for (const target of request.targets) {
        if (request.kind === "keep") ignored.keep(target.reference);
        if (request.kind === "restore") ignored.restore(target.reference);
        if (request.kind === "classify")
          classify.set(target.reference, target.classification?.class ?? null);
      }
      const op: OperationView = {
        id: request.id,
        kind: request.kind,
        status: settled.some((item) => item.status === "failed") ? "failed" : "succeeded",
        revision: 1,
        targets: request.targets.map((target, index) => ({
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
          ...settled[index],
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
    expect(screen.getByText(/python3 \(:\d+\): Completado/)).toBeInTheDocument();
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
  it("opens active projects by default and lists their ports even when folded", async () => {
    const { user } = await open();
    await user.click(tab(/Proyectos/));
    const active = screen.getByRole("button", { name: /tienda-demo/ });
    const quiet = screen.getByRole("button", { name: /ci4-kickstart/ });
    expect(active).toHaveAttribute("aria-expanded", "true");
    expect(quiet).toHaveAttribute("aria-expanded", "false");
    // Name and folder share the header row; a repo with one service has no subheader of
    // its own: its branch rides in the service row.
    expect(active).toHaveTextContent("tienda-demo");
    expect(active).toHaveTextContent("work/tienda-demo");
    expect(screen.queryByText("tienda-api")).not.toBeInTheDocument();
    expect(screen.getByText("develop")).toBeInTheDocument();
    expect(screen.getByText("Usa")).toBeInTheDocument();
    expect(screen.getByText("QA login")).toBeInTheDocument();
    // Folding keeps the occupied ports visible in the header.
    await user.click(active);
    expect(active).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("develop")).not.toBeInTheDocument();
    expect(active).toHaveTextContent(":8080");
    expect(quiet).toHaveTextContent("sin servicios activos");
  });

  // covers: UI
  it("keeps several projects open at once and folds or opens all of them", async () => {
    const { user } = await open();
    await user.click(tab(/Proyectos/));
    const quiet = screen.getByRole("button", { name: /ci4-kickstart/ });
    await user.click(quiet);
    expect(quiet).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: /tienda-demo/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    // One repo: no repo subheader, the branch rides in the header.
    expect(quiet).toHaveTextContent("main");
    expect(screen.queryByText("ci4-kickstart", { selector: ".mono" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Plegar todo" }));
    for (const name of [/tienda-demo/, /ci4-kickstart/]) {
      expect(screen.getByRole("button", { name })).toHaveAttribute("aria-expanded", "false");
    }
    await user.click(screen.getByRole("button", { name: "Abrir todo" }));
    expect(screen.getByRole("button", { name: /tienda-demo/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });

  // covers: UI
  it("lists duplicates compactly and stops them all from the project", async () => {
    const { user } = await open();
    await user.click(tab(/Proyectos/));
    expect(screen.getByText("2 sin registrar")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Detener los 2" }));
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getAllByText("api")).toHaveLength(2);
    await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));
  });

  // covers: UI
  it("opens a service's details from its own row", async () => {
    const { user } = await open();
    await user.click(tab(/Proyectos/));
    const row = screen.getByRole("button", { name: /Detalles de api · ref a1b2/ });
    expect(row).toHaveAttribute("aria-expanded", "false");
    await user.click(row);
    expect(row).toHaveAttribute("aria-expanded", "true");
    const facts = document.getElementById(row.getAttribute("aria-controls") ?? "");
    expect(facts).not.toBeNull();
    expect(facts).toHaveTextContent("Comando (valores ocultos)");
    await user.click(row);
    expect(document.getElementById(row.getAttribute("aria-controls") ?? "")).toBeNull();
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
    expect(await screen.findByText(/api \(:8080\): Completado/)).toBeInTheDocument();
    expect(screen.getByText(/api \(:52814\): La instancia cambió/)).toBeInTheDocument();
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

  // covers: UI, UXR-5.1
  it("shows the exact rule before ignoring, keeps it apart from adopting and restores it", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    // Adopting no longer hides a process behind a different promise.
    await user.click(screen.getByRole("button", { name: "Adoptar" }));
    expect(screen.queryByRole("button", { name: /Ignorar/ })).toBeNull();
    await user.keyboard("{Escape}");

    await user.click(screen.getByRole("button", { name: "Ignorar" }));
    const confirm = screen.getByRole("dialog");
    expect(await within(confirm).findByText("/tmp/scratch")).toBeInTheDocument();
    expect(within(confirm).getByText(/misma carpeta y el mismo comando/)).toBeInTheDocument();
    expect(within(confirm).getByText(/No lo detiene ni lo adopta/)).toBeInTheDocument();
    expect(within(confirm).queryByText(/7000/)).toBeNull();
    await user.click(within(confirm).getByRole("button", { name: "Cancelar" }));
    expect(backend.calls.operation).toBeUndefined();

    await user.click(screen.getByRole("button", { name: "Ignorar" }));
    await user.click(await screen.findByRole("button", { name: "Ignorar este servicio" }));
    await waitFor(() => {
      expect(
        backend.calls.operation
          ?.map(([request]) => request as OperationRequest)
          .map(({ kind, targets }) => [kind, targets[0]?.reference]),
      ).toEqual([["keep", "z9y8"]]);
    });
    await waitFor(() => {
      expect(screen.queryByRole("button", { name: "Ignorar" })).toBeNull();
    });

    await user.click(screen.getByRole("button", { name: "Ver ignorados" }));
    const list = screen.getByRole("dialog");
    expect(await within(list).findByText("/tmp/scratch")).toBeInTheDocument();
    await user.click(within(list).getByRole("button", { name: "Restaurar" }));
    await waitFor(() => {
      expect(within(list).getByText("No hay servicios ignorados.")).toBeInTheDocument();
    });
    const restores = backend.calls.operation
      ?.map(([request]) => request as OperationRequest)
      .filter(({ kind }) => kind === "restore");
    expect(restores).toHaveLength(1);
    expect(restores?.[0]?.targets[0]?.reference).toMatch(/^rule-/);
    await user.click(within(list).getByRole("button", { name: "Cerrar" }));
    expect(screen.getByRole("button", { name: "Ignorar" })).toBeInTheDocument();
  });

  // covers: UI, UXR-5.1
  it("reaches the ignored list with nothing loose and explains that it is empty", async () => {
    const empty = sampleSnapshot();
    empty.instances = empty.instances.filter((view) => view.ref !== "z9y8");
    const { user } = await open(empty);
    await user.click(tab(/Sin sin proyecto|Sin proyecto/));
    await user.click(screen.getByRole("button", { name: "Ver ignorados" }));
    expect(await screen.findByText("No hay servicios ignorados.")).toBeInTheDocument();
  });

  // covers: UI, UXR-5.1
  it("lists earlier-build and unreadable rules, labelled, and still restores them", async () => {
    const { backend, user } = await open();
    const rules = [
      {
        id: "a".repeat(32),
        kind: "process" as const,
        folder: "/work/old",
        command: ["node", "server.js", "--token=abc"],
        container: null,
        context: null,
        legacy: true,
        createdAt: "2026-10-01T10:00:00Z",
      },
      {
        id: "b".repeat(32),
        kind: "unknown" as const,
        folder: null,
        command: [],
        container: null,
        context: null,
        legacy: false,
        createdAt: null,
      },
    ];
    backend.ignoredRules = () => Promise.resolve(rules);
    await user.click(tab(/Sin proyecto/));
    await user.click(screen.getByRole("button", { name: "Ver ignorados" }));
    const list = screen.getByRole("dialog");
    expect(await within(list).findByText("regla anterior")).toBeInTheDocument();
    expect(within(list).getByText("/work/old")).toBeInTheDocument();
    expect(within(list).queryByText(/abc/)).toBeNull();
    expect(within(list).getByText(/no sabe leer/)).toBeInTheDocument();
    await user.click(within(list).getAllByRole("button", { name: "Restaurar" })[1] as HTMLElement);
    await waitFor(() => {
      expect(
        backend.calls.operation?.map(([r]) => (r as OperationRequest).targets[0]?.reference),
      ).toEqual(["b".repeat(32)]);
    });
  });

  // covers: UI, UXR-5.1
  it("keeps the rule listed and says why when a restore fails", async () => {
    const { backend, user } = await open();
    backend.ignoredRules = () =>
      Promise.resolve([
        {
          id: "c".repeat(32),
          kind: "container" as const,
          folder: null,
          command: [],
          container: "scratch-db",
          context: "colima",
          legacy: false,
          createdAt: null,
        },
      ]);
    backend.startOperation = (request) =>
      Promise.resolve({
        id: request.id,
        kind: "restore",
        status: "failed",
        revision: 1,
        targets: [
          {
            reference: "c".repeat(32),
            adoptTarget: null,
            status: "failed",
            error: "la base de datos está ocupada",
            result: null,
            attempts: 1,
          },
        ],
      });
    await user.click(tab(/Sin proyecto/));
    await user.click(screen.getByRole("button", { name: "Ver ignorados" }));
    const list = screen.getByRole("dialog");
    expect(await within(list).findByText("scratch-db")).toBeInTheDocument();
    expect(within(list).getByText("colima")).toBeInTheDocument();
    await user.click(within(list).getByRole("button", { name: "Restaurar" }));
    expect(await within(list).findByText("la base de datos está ocupada")).toBeInTheDocument();
    expect(within(list).getByText("scratch-db")).toBeInTheDocument();
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
    await user.click(screen.getByRole("button", { name: /tienda-demo/ }));
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
    expect(screen.getByRole("button", { name: /ci4-kickstart/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(screen.getByRole("button", { name: /tienda-demo/ })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
    cleanup();
  });

  // covers: UI, UXR-1.2
  it("restores an adoption draft without restoring a destructive confirmation", async () => {
    const backend = fakeBackend(sampleSnapshot(), {
      version: 2,
      tab: "orphans",
      openProjects: [],
      closedProjects: [],
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
      version: 2,
      tab: "orphans",
      openProjects: [],
      closedProjects: [],
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

  // covers: UI, UXR-4.2
  it("associates projects with a shared service only after reviewing the files", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Compartidos/));
    await user.click(screen.getByRole("button", { name: "Gestionar proyectos de mysql" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByRole("checkbox", { name: "tienda-demo" })).toBeChecked();
    expect(within(dialog).getByText(/El uso real puede no estar declarado/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("checkbox", { name: "ci4-kickstart" }));
    await user.click(within(dialog).getByRole("button", { name: "Revisar cambios" }));
    expect(within(dialog).getByText("/work/ci4-kickstart/.devports.yml")).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
    await user.click(within(dialog).getByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(backend.calls.operation).toHaveLength(1);
    });
    const sent = backend.calls.operation?.[0]?.[0] as OperationRequest;
    expect(sent.kind).toBe("setConsumers");
    expect(sent.targets).toHaveLength(1);
    expect(sent.targets[0]).toMatchObject({
      reference: "ci4-kickstart",
      consumers: { shared: "mysql", include: true },
    });
    expect(await within(dialog).findByText("Guardado")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Listo" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByText("ci4-kickstart", { selector: ".tag" })).toBeInTheDocument();
    expect(backend.getPersisted().draft).toBeNull();
  });

  // covers: UI, UXR-4.2
  it("accepts zero consumers and says so instead of claiming nobody uses the resource", async () => {
    const { user } = await open();
    await user.click(tab(/Compartidos/));
    await user.click(screen.getByRole("button", { name: "Gestionar proyectos de mysql" }));
    const dialog = screen.getByRole("dialog");
    await user.click(await within(dialog).findByRole("checkbox", { name: "tienda-demo" }));
    expect(within(dialog).getByText(/Sin proyectos asociados/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Revisar cambios" }));
    await user.click(within(dialog).getByRole("button", { name: "Guardar" }));
    await user.click(await within(dialog).findByRole("button", { name: "Listo" }));
    expect(screen.getByText("Sin proyectos asociados")).toBeInTheDocument();
    expect(screen.getByText(/El uso real puede no estar declarado/)).toBeInTheDocument();
  });

  // covers: UI, UXR-4.2
  it("shows a partial save per project and retries only what is pending", async () => {
    const { backend, user } = await open();
    const target = (reference: string, failed: boolean) => ({
      reference,
      adoptTarget: null,
      status: failed ? ("failed" as const) : ("succeeded" as const),
      error: failed ? "disk full" : null,
      usesError: failed ? { code: "io" as const, message: "disk full" } : null,
      result: null,
      attempts: 1,
    });
    let operation: OperationView | undefined;
    backend.startOperation = (request) => {
      operation = {
        id: request.id,
        kind: "setConsumers",
        status: "partial",
        revision: 1,
        targets: [target("ci4-kickstart", false), target("tienda-demo", true)],
      };
      return Promise.resolve(operation);
    };
    backend.retryOperation = (id) => {
      backend.calls["retryOperation"] = [[id]];
      return Promise.resolve({
        id,
        kind: "setConsumers",
        status: "succeeded",
        revision: 2,
        targets: [target("ci4-kickstart", false), { ...target("tienda-demo", false) }],
      });
    };
    await user.click(tab(/Compartidos/));
    await user.click(screen.getByRole("button", { name: "Gestionar proyectos de mysql" }));
    const dialog = screen.getByRole("dialog");
    await user.click(await within(dialog).findByRole("checkbox", { name: "ci4-kickstart" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "tienda-demo" }));
    await user.click(within(dialog).getByRole("button", { name: "Revisar cambios" }));
    await user.click(within(dialog).getByRole("button", { name: "Guardar" }));
    expect(await within(dialog).findByText(/Se guardaron algunos proyectos/)).toBeInTheDocument();
    expect(within(dialog).getByText("disk full")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Reintentar pendientes" }));
    await waitFor(() => {
      expect(backend.calls["retryOperation"]).toEqual([[operation?.id]]);
    });
    expect(await within(dialog).findByRole("button", { name: "Listo" })).toBeInTheDocument();
  });

  // covers: UI, UXR-4.2
  it("opens the same sheet from a project, focused on that project", async () => {
    const { user } = await open();
    await user.click(tab(/Proyectos/));
    await user.click(screen.getByRole("button", { name: /ci4-kickstart/ }));
    await user.click(screen.getByRole("button", { name: "Asociar mysql" }));
    const dialog = screen.getByRole("dialog");
    const own = await within(dialog).findByRole("checkbox", { name: "ci4-kickstart" });
    await waitFor(() => {
      expect(own).toHaveFocus();
    });
    expect(within(dialog).getByRole("checkbox", { name: "tienda-demo" })).toBeChecked();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  // covers: UI, UXR-4.2
  it("restores a consumers draft at its review step and discards one whose resource vanished", async () => {
    const draft = (shared: string) => ({
      kind: "consumers" as const,
      shared,
      selected: ["tienda-demo", "ci4-kickstart"],
      reviewing: true,
      focus: null,
    });
    const base = {
      version: 2 as const,
      tab: "shared" as const,
      openProjects: [],
      closedProjects: [],
      activityFilter: "all" as const,
    };
    const restored = fakeBackend(sampleSnapshot(), { ...base, draft: draft("mysql") });
    state.current = restored;
    render(<App />);
    const dialog = await screen.findByRole("dialog");
    expect(
      await within(dialog).findByText("/work/ci4-kickstart/.devports.yml"),
    ).toBeInTheDocument();
    expect(restored.calls.operation).toBeUndefined();
    cleanup();

    const gone = fakeBackend(sampleSnapshot(), { ...base, draft: draft("ghost") });
    state.current = gone;
    render(<App />);
    expect(await screen.findByText(/ya no está disponible/)).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(gone.getPersisted().draft).toBeNull();
    });
  });

  // covers: UI, UXR-4.2
  it("offers a start command only for a stopped resource and never invents one", async () => {
    const stopped = sampleSnapshot();
    const mysql = stopped.instances.find((view) => view.service === "mysql" && view.shared);
    if (mysql) mysql.state = "down";
    const first = await open(stopped);
    await first.user.click(tab(/Compartidos/));
    await first.user.click(screen.getByRole("button", { name: "Copiar comando de inicio" }));
    await waitFor(async () => {
      expect(await navigator.clipboard.readText()).toBe("docker start mysql");
    });
    cleanup();

    const unknown = sampleSnapshot();
    const unknownMysql = unknown.instances.find((view) => view.service === "mysql" && view.shared);
    if (unknownMysql) unknownMysql.state = "unknown";
    unknown.shared = [{ name: "mysql", start: null, endpoint: null }];
    const second = await open(unknown);
    await second.user.click(tab(/Compartidos/));
    expect(screen.queryByRole("button", { name: "Copiar comando de inicio" })).toBeNull();
    expect(screen.queryByText(/Sin comando de inicio/)).toBeNull();
    cleanup();

    const bare = sampleSnapshot();
    const bareMysql = bare.instances.find((view) => view.service === "mysql" && view.shared);
    if (bareMysql) bareMysql.state = "down";
    bare.shared = [{ name: "mysql", start: null, endpoint: null }];
    const third = await open(bare);
    await third.user.click(tab(/Compartidos/));
    expect(screen.getByText(/Sin comando de inicio declarado/)).toBeInTheDocument();
  });

  // covers: UI, UXR-4.2
  it("opens a browser only for an endpoint the resource declares", async () => {
    const withEndpoint = sampleSnapshot();
    withEndpoint.shared = [{ name: "mysql", start: null, endpoint: "http://localhost:3306/ready" }];
    const first = await open(withEndpoint);
    await first.user.click(tab(/Compartidos/));
    await first.user.click(screen.getByRole("button", { name: "Abrir" }));
    await waitFor(() => {
      expect(first.backend.calls["openUrl"]).toEqual([["http://localhost:3306/ready"]]);
    });
    cleanup();

    const second = await open();
    await second.user.click(tab(/Compartidos/));
    expect(screen.queryByRole("button", { name: "Abrir" })).toBeNull();
  });

  // covers: UI, UXR-5.2
  it("groups loose processes by evidence, keeps auxiliaries folded but reachable and unchanged in actions", async () => {
    const snapshot = sampleSnapshot();
    const loose = snapshot.instances.find((view) => view.ref === "z9y8") as InstanceView;
    const fpm: InstanceView = {
      ...loose,
      ref: "f1f1",
      pid: 7001,
      port: 9100,
      ports: [9100],
      target_identity: "fpm-target",
      cwd: "/opt/homebrew/var",
      argv: ["/opt/homebrew/opt/php@8.3/sbin/php-fpm", "--nodaemonize"],
    };
    const server: InstanceView = {
      ...loose,
      ref: "s2s2",
      pid: 7002,
      port: 18080,
      ports: [18080],
      target_identity: "server-target",
      cwd: "/work/shop-wt",
      worktree: "/work/shop-wt",
      branch: "feature",
      argv: ["php", "-S", "localhost:18080"],
    };
    snapshot.instances.push(fpm, server);
    snapshot.classifications.push(
      {
        instanceRef: "f1f1",
        class: "auxiliary",
        detected: "auxiliary",
        program: "php-fpm",
        origin: "Homebrew",
        signals: ["homebrewDaemon"],
        corrected: false,
      },
      {
        instanceRef: "s2s2",
        class: "project",
        detected: "project",
        program: "php",
        origin: null,
        signals: ["gitWorktree"],
        corrected: false,
      },
    );
    const { backend, user } = await open(snapshot);
    await user.click(tab(/Sin proyecto/));
    expect(screen.getByText("3 procesos sin dueño · 1 posible auxiliar")).toBeInTheDocument();
    expect(screen.getByText(/Servidores de desarrollo · 1/)).toBeInTheDocument();
    expect(screen.getByText(/Sin clasificar · 1/)).toBeInTheDocument();
    const group = screen.getByRole("button", { name: /Posibles herramientas auxiliares · 1/ });
    expect(group).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Homebrew")).toBeNull();
    await user.click(group);
    expect(group).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Homebrew")).toBeInTheDocument();
    expect(screen.getByText("php-fpm", { selector: ".tag" })).toBeInTheDocument();
    expect(screen.getByText(/No es un hecho|una pista, no un hecho/)).toBeInTheDocument();
    // Folding or classifying hides nothing: the same safe actions are on the card.
    const card = screen.getByText("php-fpm", { selector: ".tag" }).closest(".card") as HTMLElement;
    expect(within(card).getByRole("button", { name: "Detener" })).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Ignorar" })).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Adoptar" })).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UI, UXR-5.2
  it("corrects a classification with its exact scope shown, then resets it to automatic", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    expect(screen.getByText(/Sin clasificar · 1/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reclasificar" }));
    const dialog = screen.getByRole("dialog");
    expect(await within(dialog).findByText("/tmp/scratch")).toBeInTheDocument();
    expect(within(dialog).getByText(/Sin indicios/)).toBeInTheDocument();
    expect(
      within(dialog).getByText(/No lo ignora, adopta, detiene ni reinicia/),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(/solo para esta carpeta y este comando exactos/),
    ).toBeInTheDocument();
    await user.click(within(dialog).getByRole("radio", { name: "Herramienta auxiliar" }));
    await user.click(within(dialog).getByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(backend.calls.operation).toHaveLength(1);
    });
    const sent = backend.calls.operation?.[0]?.[0] as OperationRequest;
    expect(sent.kind).toBe("classify");
    expect(sent.targets[0]).toMatchObject({
      reference: "z9y8",
      classification: { class: "auxiliary" },
    });
    // The process moved to the folded group; it was not ignored or stopped.
    const group = await screen.findByRole("button", {
      name: /Posibles herramientas auxiliares · 1/,
    });
    expect(screen.queryByText(/Sin clasificar · /)).toBeNull();
    await user.click(group);
    expect(screen.getByText("corregida")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reclasificar" }));
    const again = screen.getByRole("dialog");
    expect(within(again).getByRole("radio", { name: "Herramienta auxiliar" })).toBeChecked();
    await user.click(within(again).getByRole("radio", { name: /Automática/ }));
    await user.click(within(again).getByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(backend.calls.operation).toHaveLength(2);
    });
    expect(
      (backend.calls.operation?.[1]?.[0] as OperationRequest).targets[0]?.classification,
    ).toEqual({ class: null });
    expect(await screen.findByText(/Sin clasificar · 1/)).toBeInTheDocument();
  });

  // covers: UI, UXR-6.1
  it("adds a scan folder only after reviewing its effect, then recovers the saved result", async () => {
    const { backend, user } = await open();
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    const first = await within(dialog).findByLabelText("Carpeta 1");
    expect(first).toHaveValue("~/developer");
    await waitFor(() => {
      expect(first).toHaveFocus();
    });
    expect(within(dialog).getByText(/existe · 2 proyectos/)).toBeInTheDocument();
    expect(within(dialog).getByText(/no borra archivos ni detiene procesos/)).toBeInTheDocument();
    expect(within(dialog).getByText("/Users/demo/.config/devports/config.yml")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Añadir carpeta" }));
    await user.type(within(dialog).getByLabelText("Carpeta 2"), "/Users/demo/extra");
    await user.click(within(dialog).getByRole("button", { name: "Revisar efecto" }));
    expect(await within(dialog).findByText("extra-app")).toBeInTheDocument();
    expect(within(dialog).getByText("Empezarán a listarse")).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
    await user.click(within(dialog).getByRole("button", { name: "Volver" }));
    expect(within(dialog).getByLabelText("Carpeta 2")).toHaveValue("/Users/demo/extra");
    await user.click(within(dialog).getByRole("button", { name: "Revisar efecto" }));
    await user.click(await within(dialog).findByRole("button", { name: "Guardar" }));
    await waitFor(() => {
      expect(backend.calls.operation).toHaveLength(1);
    });
    const sent = backend.calls.operation?.[0]?.[0] as OperationRequest;
    expect(sent.kind).toBe("setScanFolders");
    expect(sent.targets[0]).toMatchObject({
      reference: "config",
      scanFolders: { roots: ["~/developer", "/Users/demo/extra"], base: "v1" },
    });
    expect(await within(dialog).findByText(/Configuración guardada/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Listo" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.getPersisted().draft).toBeNull();
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    expect(await screen.findByLabelText("Carpeta 2")).toHaveValue("/Users/demo/extra");
  });

  // covers: UI, UXR-6.1
  it("says which projects stop being listed when a folder is removed", async () => {
    const { backend, user } = await open();
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    await within(dialog).findByLabelText("Carpeta 1");
    await user.click(within(dialog).getByRole("button", { name: "Añadir carpeta" }));
    await user.type(within(dialog).getByLabelText("Carpeta 2"), "/Users/demo/work");
    await user.click(within(dialog).getByRole("button", { name: "Quitar ~/developer" }));
    await user.click(within(dialog).getByRole("button", { name: "Revisar efecto" }));
    expect(await within(dialog).findByText("Dejarán de listarse")).toBeInTheDocument();
    expect(within(dialog).getByText("tienda-demo")).toBeInTheDocument();
    expect(within(dialog).getByText("ci4-kickstart")).toBeInTheDocument();
    expect(within(dialog).getByText("work-api")).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UI, UXR-6.1
  it("keeps every typed folder and explains a rejected one beside it", async () => {
    const { backend, user } = await open();
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    await within(dialog).findByLabelText("Carpeta 1");
    await user.click(within(dialog).getByRole("button", { name: "Añadir carpeta" }));
    await user.type(within(dialog).getByLabelText("Carpeta 2"), "/Users/demo/missing");
    await user.click(within(dialog).getByRole("button", { name: "Revisar efecto" }));
    expect(await within(dialog).findByText("la carpeta no existe")).toBeInTheDocument();
    expect(within(dialog).getByLabelText("Carpeta 1")).toHaveValue("~/developer");
    expect(within(dialog).getByLabelText("Carpeta 2")).toHaveValue("/Users/demo/missing");
    expect(within(dialog).getByLabelText("Carpeta 2")).toHaveAttribute("aria-invalid", "true");
    await user.click(within(dialog).getByRole("button", { name: "Quitar /Users/demo/missing" }));
    await user.click(within(dialog).getByRole("button", { name: "Quitar ~/developer" }));
    await user.click(within(dialog).getByRole("button", { name: "Añadir carpeta" }));
    await user.click(within(dialog).getByRole("button", { name: "Revisar efecto" }));
    expect(await within(dialog).findByText("escribe una carpeta")).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UI, UXR-6.1
  it("restores a settings draft at its review step without saving, and cancels without effects", async () => {
    const draft = {
      kind: "settings" as const,
      roots: ["~/developer", "/Users/demo/extra"],
      reviewing: true,
    };
    const backend = fakeBackend(sampleSnapshot(), {
      version: 2,
      tab: "attention",
      openProjects: [],
      closedProjects: [],
      activityFilter: "all",
      draft,
    });
    state.current = backend;
    const user = userEvent.setup();
    render(<App />);
    const dialog = await screen.findByRole("dialog");
    expect(await within(dialog).findByText("extra-app")).toBeInTheDocument();
    expect(backend.calls.operation).toBeUndefined();
    await user.click(within(dialog).getByRole("button", { name: "Cancelar" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => {
      expect(backend.getPersisted().draft).toBeNull();
    });
    expect(backend.calls.operation).toBeUndefined();
  });

  // covers: UI, UXR-6.1
  it("asks to read the configuration again after a conflict and keeps what was typed", async () => {
    const { backend, user } = await open();
    backend.startOperation = (request) =>
      Promise.resolve({
        id: request.id,
        kind: "setScanFolders",
        status: "failed",
        revision: 1,
        targets: [
          {
            reference: "config",
            adoptTarget: null,
            status: "failed",
            error: "cambió",
            settingsError: {
              code: "documentChanged",
              path: "/Users/demo/.config/devports/config.yml",
            },
            result: null,
            attempts: 1,
          },
        ],
      });
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    await within(dialog).findByLabelText("Carpeta 1");
    await user.click(within(dialog).getByRole("button", { name: "Añadir carpeta" }));
    await user.type(within(dialog).getByLabelText("Carpeta 2"), "/Users/demo/extra");
    await user.click(within(dialog).getByRole("button", { name: "Revisar efecto" }));
    await user.click(await within(dialog).findByRole("button", { name: "Guardar" }));
    expect(await within(dialog).findByText(/cambió desde que lo revisaste/)).toBeInTheDocument();
    await user.click(
      within(dialog).getByRole("button", { name: "Volver a leer la configuración" }),
    );
    expect(await within(dialog).findByLabelText("Carpeta 2")).toHaveValue("/Users/demo/extra");
  });

  describe("when the core is not reachable", () => {
    // covers: UI, UXR-6.2
    it("gives up after the bounded attempts, offers Reintentar and recovers without a restart", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        const backend = fakeBackend(sampleSnapshot());
        const working = backend.snapshot.bind(backend);
        backend.snapshot = () => Promise.reject(new Error("el núcleo no responde"));
        state.current = backend;
        const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) });
        render(<App />);
        expect(
          await screen.findByText(/Conectando con DevPorts… intento 1 de 5/),
        ).toBeInTheDocument();
        await act(async () => {
          await vi.advanceTimersByTimeAsync(40_000);
        });
        const alert = await screen.findByRole("alert");
        expect(within(alert).getByText(/Comprueba que la app sigue abierta/)).toBeInTheDocument();
        expect(within(alert).getByText(/no se reenvían/)).toBeInTheDocument();
        expect(screen.getByText("No se pudo conectar")).toBeInTheDocument();
        backend.snapshot = working;
        await user.click(within(alert).getByRole("button", { name: "Reintentar" }));
        expect(await screen.findByRole("tab", { name: /Atención/ })).toBeInTheDocument();
        expect(backend.calls.operation).toBeUndefined();
      } finally {
        vi.useRealTimers();
      }
    });

    // covers: UI, UXR-6.2
    it("keeps the data, sheets and context on screen when the link drops, and recovers by itself", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      try {
        const backend = fakeBackend(sampleSnapshot());
        state.current = backend;
        const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) });
        render(<App />);
        await screen.findByRole("tab", { name: /Atención/ });
        await user.click(screen.getByRole("tab", { name: /Proyectos/ }));
        const working = backend.snapshot.bind(backend);
        backend.snapshot = () => Promise.reject(new Error("el núcleo no responde"));
        await act(async () => {
          await vi.advanceTimersByTimeAsync(10_500);
        });
        const notice = await screen.findByText(/Sin conexión con DevPorts · reintentando \(1\/5\)/);
        expect(notice).toBeInTheDocument();
        expect(screen.getByRole("tab", { name: /Proyectos/ })).toHaveAttribute(
          "aria-selected",
          "true",
        );
        expect(screen.getAllByText("tienda-demo").length).toBeGreaterThan(0);
        backend.snapshot = working;
        await user.click(screen.getByRole("button", { name: "Reintentar ahora" }));
        await waitFor(() => {
          expect(screen.queryByText(/Sin conexión con DevPorts/)).toBeNull();
        });
        expect(screen.getAllByText("tienda-demo").length).toBeGreaterThan(0);
        expect(backend.calls.operation).toBeUndefined();
      } finally {
        vi.useRealTimers();
      }
    });
  });

  // covers: UI, UXR-6.2
  it("says Docker is unreachable on every tab and never shows its containers as stopped", async () => {
    const snapshot = sampleSnapshot();
    snapshot.docker_reachable = false;
    snapshot.docker_error = "no se pudo conectar con /var/run/docker.sock";
    const mysql = snapshot.instances.find((view) => view.service === "mysql" && view.shared);
    if (mysql) mysql.state = "unknown";
    const { user } = await open(snapshot);
    expect(screen.getByText("Docker no responde")).toBeInTheDocument();
    expect(screen.getByText(/no como detenidos/)).toBeInTheDocument();
    await user.click(tab(/Compartidos/));
    expect(screen.getAllByText("Docker no responde")).toHaveLength(1);
    expect(screen.queryByText("Detenido")).toBeNull();
    expect(screen.getAllByText(/Desconocido/).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Copiar comando de inicio" })).toBeNull();
  });

  // covers: UI, UXR-6.2
  it("points to the manifest and configuration warnings and lists them in Configuración", async () => {
    const snapshot = sampleSnapshot();
    snapshot.problems = [
      "/work/blog/.devports.yml: línea 4: «api» necesita «cwd»",
      "/work/old/.devports.yml: versión 2 no soportada",
    ];
    const { user } = await open(snapshot);
    expect(screen.getByText("2 avisos de configuración o manifiestos")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ver detalle" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/«api» necesita «cwd»/)).toBeInTheDocument();
    expect(within(dialog).getByText(/versión 2 no soportada/)).toBeInTheDocument();
    expect(within(dialog).getByText("Conectado")).toBeInTheDocument();
    expect(within(dialog).getByText("Responde")).toBeInTheDocument();
    // The sheet still edits the folders: a warning is not a dead end.
    expect(await within(dialog).findByLabelText("Carpeta 1")).toBeInTheDocument();
  });

  // covers: UI, UXR-6.3
  it("says the activity could not be read instead of claiming there was none, and keeps the filter", async () => {
    const { backend, user } = await open();
    const original = backend.log.bind(backend);
    backend.log = () =>
      Promise.reject(Object.assign(new Error("disk I/O error"), { code: "storage" }));
    await user.click(tab(/Actividad/));
    await user.click(screen.getByRole("button", { name: "Alertas" }));
    expect(await screen.findByText("No se pudo leer")).toBeInTheDocument();
    expect(screen.queryByText(/Sin actividad/)).toBeNull();
    backend.log = original;
    await user.click(screen.getByRole("tab", { name: /Proyectos/ }));
    await user.click(tab(/Actividad/));
    expect(screen.getByRole("button", { name: "Alertas" })).toHaveAttribute("aria-pressed", "true");
    expect(await screen.findByText(/1 de 4 eventos/)).toBeInTheDocument();
  });

  // covers: UI, UXR-7.1
  it("exposes the sections as tabs wired to their panel and moves between them with the arrows", async () => {
    const { user } = await open();
    const list = screen.getByRole("tablist", { name: "Secciones" });
    const tabs = within(list).getAllByRole("tab");
    expect(tabs).toHaveLength(5);
    // Roving tabindex: only the selected tab is in the tab order.
    expect(tabs.map((entry) => entry.tabIndex)).toEqual([0, -1, -1, -1, -1]);
    const panel = screen.getByRole("tabpanel");
    expect(tabs[0]).toHaveAttribute("aria-controls", panel.id);
    expect(panel).toHaveAttribute("aria-labelledby", tabs[0]?.id);
    tabs[0]?.focus();
    await user.keyboard("{ArrowRight}");
    expect(tabs[1]).toHaveFocus();
    expect(tabs[1]).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveAttribute("aria-labelledby", tabs[1]?.id);
    expect(tabs.map((entry) => entry.tabIndex)).toEqual([-1, 0, -1, -1, -1]);
    await user.keyboard("{End}");
    expect(tabs[4]).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(tabs[0]).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(tabs[4]).toHaveFocus();
    await user.keyboard("{Home}");
    expect(tabs[0]).toHaveAttribute("aria-selected", "true");
    // The tab bar is not a navigation landmark pretending to be a tab list.
    expect(list.tagName).toBe("DIV");
  });

  // covers: UI, UXR-7.1
  it("keeps the folded group's control pointing at an element that always exists", async () => {
    const snapshot = sampleSnapshot();
    const loose = snapshot.instances.find((view) => view.ref === "z9y8") as InstanceView;
    snapshot.instances.push({ ...loose, ref: "f1f1", pid: 7001, port: 9100, ports: [9100] });
    snapshot.classifications = [
      {
        instanceRef: "f1f1",
        class: "auxiliary",
        detected: "auxiliary",
        program: "php-fpm",
        origin: "Homebrew",
        signals: ["homebrewDaemon"],
        corrected: false,
      },
    ];
    const { user } = await open(snapshot);
    await user.click(tab(/Sin proyecto/));
    const toggle = screen.getByRole("button", { name: /Posibles herramientas auxiliares/ });
    const target = document.getElementById(toggle.getAttribute("aria-controls") ?? "");
    expect(target).not.toBeNull();
    expect(target).toHaveAttribute("hidden");
    await user.click(toggle);
    expect(document.getElementById(toggle.getAttribute("aria-controls") ?? "")).not.toHaveAttribute(
      "hidden",
    );
  });

  // covers: UI, UXR-7.1
  it("completes the new sheets from the keyboard alone and returns focus to where it started", async () => {
    const { backend, user } = await open();
    await user.click(tab(/Sin proyecto/));
    // Ignore: open, land on Cancelar, reach the confirmation by Tab and activate it with Enter.
    const ignore = screen.getByRole("button", { name: "Ignorar" });
    ignore.focus();
    await user.keyboard("{Enter}");
    const dialog = await screen.findByRole("dialog");
    await within(dialog).findByText("/tmp/scratch");
    expect(within(dialog).getByRole("button", { name: "Cancelar" })).toHaveFocus();
    await user.tab();
    expect(within(dialog).getByRole("button", { name: "Ignorar este servicio" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(backend.calls.operation).toHaveLength(1);
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    // Settings: Escape leaves without effects and focus returns to the gear.
    const gear = screen.getByRole("button", { name: "Configuración" });
    gear.focus();
    await user.keyboard("{Enter}");
    await screen.findByLabelText("Carpeta 1");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(gear).toHaveFocus();
    expect(backend.calls.operation).toHaveLength(1);
  });

  // covers: UI, UXR-7.1
  it("announces errors and ties them to their field", async () => {
    const { user } = await open();
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    await within(dialog).findByLabelText("Carpeta 1");
    await user.click(within(dialog).getByRole("button", { name: "Añadir carpeta" }));
    const field = within(dialog).getByLabelText("Carpeta 2");
    await user.type(field, "/Users/demo/missing");
    await user.click(within(dialog).getByRole("button", { name: "Revisar efecto" }));
    const message = await within(dialog).findByText("la carpeta no existe");
    expect(field).toHaveAttribute("aria-invalid", "true");
    expect(field).toHaveAttribute("aria-describedby", message.id);
  });
});

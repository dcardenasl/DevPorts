import { fakeBackend, open, resetBackend, state, tab } from "./test-utils";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { sampleSnapshot } from "./sample";
import type { OperationRequest, OperationView } from "./types";

describe("the panel: attention, projects and stops", () => {
  beforeEach(resetBackend);

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
    const official = screen.getByText("OFICIAL").closest(".section");
    expect(official?.querySelector(".item-sub.mono")).toHaveTextContent("Colgada · no responde");
    expect(official?.querySelector(".item-sub.mono")).not.toHaveTextContent("Sana");
    await user.click(tab(/Compartidos/));
    expect(screen.getByText(/Sin confirmar/, { selector: ".item-sub" })).toBeVisible();
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
});

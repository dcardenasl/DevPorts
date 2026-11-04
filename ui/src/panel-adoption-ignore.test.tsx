import { open, resetBackend, tab } from "./test-utils";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { sampleSnapshot } from "./sample";
import type { InstanceView, OperationRequest } from "./types";

describe("the panel: adoption, ignored servers and empty states", () => {
  beforeEach(resetBackend);

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

  // covers: ROB-5.3
  it("says where the manifest is when a loose server runs inside a project nobody searches", async () => {
    const snapshot = sampleSnapshot();
    snapshot.unscanned = [{ instance_ref: "z9y8", manifest: "/Users/demo/blog/.devports.yml" }];
    const { user } = await open(snapshot);
    await user.click(tab(/Sin proyecto/));
    expect(await screen.findByText(/\/Users\/demo\/blog\/\.devports\.yml/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Abrir Ajustes" }));
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
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
    expect(await screen.findByText(/Usa de 1 a 63 letras minúsculas/)).toBeInTheDocument();
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
});

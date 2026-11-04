import { fakeBackend, open, resetBackend, state } from "./test-utils";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { sampleSnapshot } from "./sample";
import type { OperationRequest } from "./types";

describe("the panel: scan folders", () => {
  beforeEach(resetBackend);

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
      version: 3,
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
});

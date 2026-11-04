import { fakeBackend, open, resetBackend, state, tab } from "./test-utils";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { sampleSnapshot } from "./sample";
import type { InstanceView } from "./types";

describe("when the core is not reachable", () => {
  beforeEach(resetBackend);

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

describe("the panel: connection notices and copy", () => {
  beforeEach(resetBackend);

  // covers: UI, UXR-6.2
  it("says Docker is unreachable on every tab and never shows its containers as stopped", async () => {
    const snapshot = sampleSnapshot();
    snapshot.docker_reachable = false;
    snapshot.docker_error = {
      code: "unreachable",
      detail: "no se pudo conectar con /var/run/docker.sock",
    };
    const mysql = snapshot.instances.find((view) => view.service === "mysql" && view.shared);
    if (mysql) mysql.state = "unknown";
    const { user } = await open(snapshot);
    expect(screen.getByText("Docker no responde")).toBeInTheDocument();
    expect(screen.getByText(/no como detenidos/)).toBeInTheDocument();
    // The strip says what to do; the technical reason lives only in Configuración.
    expect(screen.queryByText(/no se pudo conectar/)).toBeNull();
    await user.click(tab(/Compartidos/));
    expect(screen.getAllByText("Docker no responde")).toHaveLength(1);
    expect(screen.queryByText("Detenido")).toBeNull();
    expect(screen.getAllByText(/Sin confirmar/).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Copiar comando de inicio" })).toBeNull();
  });

  // covers: UI, UXR-11.5
  it("gives each project row its full folder on hover", async () => {
    const { user } = await open();
    await user.click(tab(/Proyectos/));
    expect(screen.getByTitle("/work/tienda-demo")).toHaveTextContent("work/tienda-demo");
  });

  // covers: UI, UXR-11.1
  it("opens the Docker reason in Configuración from the strip's detail button", async () => {
    const snapshot = sampleSnapshot();
    snapshot.docker_reachable = false;
    snapshot.docker_error = {
      code: "unreachable",
      detail: "no se pudo conectar con /var/run/docker.sock",
    };
    const mysql = snapshot.instances.find((view) => view.service === "mysql" && view.shared);
    if (mysql) mysql.state = "unknown";
    const { user } = await open(snapshot);
    expect(screen.getByText(/1 servicio usa Docker y no se puede comprobar/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ver detalle" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/no se pudo conectar/)).toBeInTheDocument();
  });

  // covers: UI, UXR-12.1, UXR-12.2
  it("says nothing about Docker when no declared service uses it, and says why in Configuración", async () => {
    const snapshot = sampleSnapshot();
    snapshot.docker_reachable = false;
    snapshot.docker_error = {
      code: "unreachable",
      detail: "no se pudo conectar con /var/run/docker.sock",
    };
    snapshot.instances = snapshot.instances.map((view) =>
      view.state === "unknown" ? { ...view, state: "down" } : view,
    );
    const { user } = await open(snapshot);
    for (const name of [/Atención/, /Proyectos/, /Compartidos/, /Sin proyecto/, /Actividad/]) {
      await user.click(tab(name));
      expect(screen.queryByText("Docker no responde")).toBeNull();
    }
    await user.click(screen.getByRole("button", { name: "Configuración" }));
    const dialog = screen.getByRole("dialog");
    expect(
      within(dialog).getByText("No responde · ningún servicio declarado lo usa"),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText(/no se pudo conectar/)).toBeNull();
  });

  // covers: UI, UXR-12.1
  it("shows the Docker notice once a declared service becomes unknown and drops it when Docker answers", async () => {
    const snapshot = sampleSnapshot();
    snapshot.docker_reachable = false;
    snapshot.docker_error = { code: "connectionLost" };
    snapshot.instances = snapshot.instances.map((view) =>
      view.service === "mysql" && view.shared ? { ...view, state: "unknown" } : view,
    );
    const { backend } = await open(snapshot);
    expect(screen.getByText("Docker no responde")).toBeInTheDocument();
    await act(async () => {
      backend.publishSnapshot({
        ...snapshot,
        docker_reachable: true,
        docker_error: null,
        instances: snapshot.instances.map((view) =>
          view.state === "unknown" ? { ...view, state: "healthy" } : view,
        ),
      });
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(screen.queryByText("Docker no responde")).toBeNull();
    });
  });

  // covers: UI, UXR-6.2
  it("points to the manifest and configuration warnings and lists them in Configuración", async () => {
    const snapshot = sampleSnapshot();
    snapshot.problems = [
      {
        code: "manifestUnusable",
        problem: {
          code: "invalid",
          path: "/work/blog/.devports.yml",
          diagnostics: [
            { severity: "error", line: 4, kind: { code: "noCwdNorDocker", service: "api" } },
          ],
        },
      },
      {
        code: "manifestUnusable",
        problem: {
          code: "invalid",
          path: "/work/old/.devports.yml",
          diagnostics: [
            { severity: "error", line: null, kind: { code: "unsupportedVersion", version: "2" } },
          ],
        },
      },
    ];
    const { user } = await open(snapshot);
    expect(screen.getByText("2 avisos de configuración o manifiestos")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ver detalle" }));
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByText(/«api» necesita «cwd»/)).toBeInTheDocument();
    expect(within(dialog).getByText(/Versión 2 no soportada/)).toBeInTheDocument();
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

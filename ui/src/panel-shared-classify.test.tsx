import { fakeBackend, open, resetBackend, state, tab } from "./test-utils";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";
import { sampleSnapshot } from "./sample";
import type { InstanceView, OperationRequest, OperationView } from "./types";

describe("the panel: shared services and classification", () => {
  beforeEach(resetBackend);

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
      usesError: failed ? { code: "io" as const, detail: "disk full" } : null,
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
    expect(within(dialog).getByText("No se pudo leer o escribir: disk full")).toBeInTheDocument();
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
      version: 3 as const,
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
    const card = screen
      .getByText("php-fpm", { selector: ".tag" })
      .closest(".group > div") as HTMLElement;
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
});

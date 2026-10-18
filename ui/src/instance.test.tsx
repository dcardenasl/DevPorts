import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { es } from "./i18n/es";
import {
  canRequestStop,
  commandLine,
  dotFor,
  instanceTitle,
  isActiveInstance,
  stateLabel,
} from "./instance";
import { sampleSnapshot } from "./sample";
import type { InstanceView } from "./types";
import { InstanceDetails } from "./views/InstanceDetails";

function fixture(): InstanceView {
  const instance = sampleSnapshot().instances[0];
  if (instance === undefined) throw new Error("missing fixture");
  return instance;
}

describe("instance diagnostics", () => {
  // covers: UXR-2.3
  it("separates actual state, role and stop availability across every lifecycle", () => {
    const labels = {
      healthy: "Sana",
      down: "Detenido",
      unhealthy: "Colgada · no responde",
      unknown: "Desconocido · sin observación actual",
      starting: "Iniciando",
      stopping: "Deteniendo",
      forgotten: "Olvidada · sigue activa",
    } as const;
    for (const state of Object.keys(labels) as (keyof typeof labels)[]) {
      expect(stateLabel(state, es)).toBe(labels[state]);
      expect(canRequestStop({ ...fixture(), state })).toBe(
        ["healthy", "unhealthy", "forgotten"].includes(state),
      );
    }
    expect(isActiveInstance({ ...fixture(), state: "unknown" })).toBe(false);
    expect(isActiveInstance({ ...fixture(), state: "stopping" })).toBe(true);
    expect(dotFor("unknown")).not.toBe("green");
    expect(dotFor("stopping")).toBe("amber");
  });

  // covers: UXR-2.3
  it("masks positional values, assignments, compact options, credentials and embedded scripts", () => {
    const command = commandLine(
      [
        "/bin/node",
        "--api-key=top-secret",
        "--password",
        "secret-value",
        "-pCompactSecret",
        "https://user:password@localhost",
        "-e",
        "process.env.TOKEN='another-secret'",
      ],
      es,
    );
    for (const secret of [
      "top-secret",
      "secret-value",
      "CompactSecret",
      "user:password",
      "another-secret",
    ])
      expect(command).not.toContain(secret);
    expect(command).toContain("--api-key=[oculto]");
    expect(command).toContain("--password");
    expect(commandLine([], es)).toBe("No disponible");
    expect(
      instanceTitle(
        {
          ...fixture(),
          service: null,
          argv: ["/a/very/long/path/node", "--token=secret"],
        },
        es,
      ),
    ).toBe("node");
    // A daemon that rewrites its title is named by the program, not by the last path in it.
    const title = "php-fpm: master process (/opt/homebrew/etc/php/8.2/php-fpm.conf)";
    expect(instanceTitle({ ...fixture(), service: null, argv: [title] }, es)).toBe("php-fpm");
    expect(commandLine([title], es)).toBe("php-fpm");
  });

  // covers: UXR-2.3
  it("expands long paths and known origin without inventing missing facts", async () => {
    const path = `/work/${"long-project/".repeat(30)}server`;
    const view = {
      ...fixture(),
      cwd: path,
      ports: [8080, 9090],
      stale: "arrancó antes del último commit o checkout (12:00)",
      launcher_chain: [{ pid: 1234, name: "zsh", label: null }],
    };
    render(<InstanceDetails view={view} />);
    const summary = screen.getByText(/Detalles de api/);
    await userEvent.setup().click(summary);
    expect(summary.closest("details")).toHaveAttribute("open");
    expect(screen.getByText(path)).toBeVisible();
    expect(screen.getByText("8080, 9090")).toBeVisible();
    expect(screen.getByText(/zsh · PID 1234/)).toBeVisible();
    expect(screen.getByText(/Es un indicio basado en fechas de Git/)).toBeVisible();
    expect(within(summary).getByText(/Posible código anterior/)).toBeVisible();
    expect(screen.getByText("api-process-target")).toBeVisible();
  });

  // covers: UXR-2.3
  it("explains unavailable ancestry and ports without turning a configured port into an observation", async () => {
    const view = {
      ...fixture(),
      state: "unknown" as const,
      ports: [],
      launcher_chain: [],
      cwd: null,
    };
    render(<InstanceDetails view={view} />);
    await userEvent.setup().click(screen.getByText("Detalles de api"));
    expect(screen.getByText("Lanzador no disponible; no se infiere su origen")).toBeVisible();
    expect(screen.getByText("Sin puertos observados en la instantánea actual")).toBeVisible();
    expect(screen.getByText("Desconocido · sin observación actual")).toBeVisible();
    expect(screen.queryByText("Detenido")).not.toBeInTheDocument();
    expect(screen.getByText("8080")).toBeVisible();
  });
});

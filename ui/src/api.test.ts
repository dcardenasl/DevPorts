import { afterEach, describe, expect, it, vi } from "vitest";

import { createTauriBackend } from "./api";
import type { OperationRequest, OperationView } from "./types";

describe("operation command recovery", () => {
  // covers: UXR-1.3
  it("queries the shell after a lost start response instead of sending again", async () => {
    const request: OperationRequest = {
      id: "op-1",
      kind: "stop",
      targets: [{ reference: "instance-1" }],
    };
    const accepted: OperationView = {
      id: request.id,
      kind: request.kind,
      status: "running",
      revision: 1,
      targets: [
        {
          reference: "instance-1",
          adoptTarget: null,
          status: "running",
          error: null,
          result: null,
          attempts: 1,
        },
      ],
    };
    const calls: string[] = [];
    const backend = createTauriBackend({
      core: {
        invoke: <T>(command: string): Promise<T> => {
          calls.push(command);
          if (command === "start_operation")
            return Promise.reject(new Error("response lost after acceptance"));
          return Promise.resolve(accepted as T);
        },
      },
      event: { listen: () => Promise.resolve(() => undefined) },
    });

    await expect(backend.startOperation(request)).resolves.toEqual(accepted);
    expect(calls).toEqual(["start_operation", "get_operation"]);
  });
});

describe("the backend lookup", () => {
  afterEach(() => {
    Reflect.deleteProperty(window, "__TAURI__");
    vi.resetModules();
  });

  // covers: UXR-6.2
  it("does not remember a failed attempt, so a retry can succeed", async () => {
    vi.resetModules();
    const { backend } = await import("./api");
    let reads = 0;
    Object.defineProperty(window, "__TAURI__", {
      configurable: true,
      get: () => {
        reads += 1;
        if (reads === 1) throw new Error("the bridge is not ready");
        return {
          core: { invoke: () => Promise.resolve(undefined) },
          event: { listen: () => Promise.resolve(() => undefined) },
        };
      },
    });
    await expect(backend()).rejects.toThrow("the bridge is not ready");
    const second = await backend();
    expect(typeof second.snapshot).toBe("function");
    expect(await backend()).toBe(second);
  });
});

describe("the preferences commands", () => {
  // covers: UXR-9.1
  it("reads and saves through the two commands and sends the preferences as one value", async () => {
    const calls: [string, Record<string, unknown> | undefined][] = [];
    const backend = createTauriBackend({
      core: {
        invoke: <T>(command: string, args?: Record<string, unknown>): Promise<T> => {
          calls.push([command, args]);
          return Promise.resolve({ theme: "dark", language: "en" } as T);
        },
      },
      event: { listen: () => Promise.resolve(() => undefined) },
    });

    await expect(backend.preferences()).resolves.toEqual({ theme: "dark", language: "en" });
    await backend.setPreferences({ theme: "dark", language: "en" });

    expect(calls).toEqual([
      ["get_preferences", undefined],
      ["set_preferences", { preferences: { theme: "dark", language: "en" } }],
    ]);
  });
});

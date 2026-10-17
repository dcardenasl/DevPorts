import { describe, expect, it } from "vitest";

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

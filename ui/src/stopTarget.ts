import type { InstanceView, StopExpectation } from "./types";

export function stopExpectation(view: InstanceView): StopExpectation {
  return {
    targetIdentity: view.target_identity,
    pid: view.pid,
    container: view.container,
    port: view.port,
    project: view.project,
    service: view.service,
    kind: view.kind,
    role: view.role,
    shared: view.shared,
    startedAt: view.started_at,
  };
}

export function sameStopTarget(current: InstanceView, confirmed: InstanceView): boolean {
  return JSON.stringify(stopExpectation(current)) === JSON.stringify(stopExpectation(confirmed));
}

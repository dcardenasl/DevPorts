// A fake core for developing the panel in a browser. Keeps its own state so the
// actions visibly work (stopping removes a row, extending adds time).

import { mockAdoption } from "./mock-adoption";
import { adoptionFailure, adoptionMessage } from "./adoption";
import type { Backend } from "./api";
import { sampleLog, sampleSnapshot } from "./sample";
import type { InstanceView, OperationRequest, OperationView, PanelState, Snapshot } from "./types";

export function mockBackend(): Backend {
  let state: Snapshot = sampleSnapshot();
  let panelState: PanelState = {
    version: 1,
    tab: "attention",
    expandedProject: null,
    expandedProjectSet: false,
    activityFilter: "all",
    draft: null,
  };
  const listeners = new Set<(snapshot: Snapshot) => void>();
  const opListeners = new Set<(operation: OperationView) => void>();
  const history: OperationView[] = [];
  const publish = (next: Snapshot): void => {
    state = { ...next, revision: state.revision + 1 };
    for (const listener of listeners) listener(state);
  };
  const without = (ref: string): Snapshot => ({
    ...state,
    instances: state.instances.filter((view) => view.ref !== ref),
    attention: state.attention.filter((item) => item.instance_ref !== ref),
  });
  const find = (ref: string): InstanceView => {
    const view = state.instances.find((candidate) => candidate.ref === ref);
    if (view === undefined) throw new Error(`no existe la instancia «${ref}»`);
    return view;
  };

  const adoption = mockAdoption(() => state, publish);
  return {
    adoptionOptions: adoption.adoptionOptions,
    previewAdoption: adoption.previewAdoption,
    validateAdoption: adoption.validateAdoption,
    snapshot: () => Promise.resolve(state),
    previewProjectStop: (project) =>
      Promise.resolve(
        state.instances.filter(
          (view) =>
            view.project === project &&
            view.kind === "app" &&
            !view.shared &&
            view.target_identity !== null &&
            ["healthy", "unhealthy", "forgotten"].includes(view.state),
        ),
      ),
    panelState: () => Promise.resolve(panelState),
    savePanelState: (next) => {
      panelState = next;
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
      const prior = history.find((item) => item.id === request.id);
      if (prior) return prior;
      const op: OperationView = {
        id: request.id,
        kind: request.kind,
        status: "running",
        revision: 1,
        targets: request.targets.map((t) => ({
          reference: t.reference,
          adoptTarget: t.adoptTarget ?? null,
          stopExpectation: t.stopExpectation ?? null,
          status: "running",
          error: null,
          result: null,
          attempts: 1,
        })),
      };
      history.unshift(op);
      opListeners.forEach((listener) => {
        listener(op);
      });
      for (const target of request.targets) {
        const ref = target.reference;
        try {
          if (request.kind === "stop" || request.kind === "stopProject") {
            find(ref);
            publish(without(ref));
          } else if (request.kind === "keep") {
            find(ref);
            publish({
              ...state,
              instances: state.instances.map((candidate) =>
                candidate.ref === ref ? { ...candidate, ignored: true } : candidate,
              ),
              attention: state.attention.filter((item) => item.instance_ref !== ref),
            });
          } else if (request.kind === "extend") {
            const view = find(ref);
            const end = Date.parse(view.expires_at ?? "") + state.extend_minutes * 60_000;
            publish({
              ...state,
              instances: state.instances.map((candidate) =>
                candidate.ref === ref
                  ? { ...candidate, expires_at: new Date(end).toISOString() }
                  : candidate,
              ),
              attention: state.attention.filter((item) => item.instance_ref !== ref),
            });
          } else if (target.adoption) {
            const targetResult = op.targets.find((item) => item.reference === ref);
            if (targetResult) {
              targetResult.result = adoption.apply(target.adoption.id, target.adoption.edits);
              targetResult.adoptionReview = target.adoption.id;
            }
          } else throw new Error("Falta revisar la adopción");
          const targetResult = op.targets.find((item) => item.reference === ref);
          if (targetResult) targetResult.status = "succeeded";
        } catch (error) {
          const result = op.targets.find((item) => item.reference === ref);
          if (result) {
            result.status = "failed";
            result.adoptionError = adoptionFailure(error);
            result.error = adoptionMessage(result.adoptionError);
          }
        }
      }
      op.status = op.targets.some((target) => target.status === "failed")
        ? op.targets.some((target) => target.status === "succeeded")
          ? "partial"
          : "failed"
        : "succeeded";
      op.revision++;
      opListeners.forEach((listener) => {
        listener(op);
      });
      return op;
    },
    retryOperation: async (id) => {
      await Promise.resolve();
      const op = history.find((item) => item.id === id);
      if (!op) throw new Error("No existe la operación");
      return op;
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return Promise.resolve(() => {
        listeners.delete(listener);
      });
    },
    log: () => Promise.resolve(sampleLog()),
    openUrl: () => Promise.resolve(),
    panelReady: () => Promise.resolve(),
  };
}

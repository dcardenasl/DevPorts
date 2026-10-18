// A fake core for developing the panel in a browser. Keeps its own state so the
// actions visibly work (stopping removes a row, extending adds time).

import { mockAdoption } from "./mock-adoption";
import { mockClassify } from "./mock-classify";
import { mockConsumers } from "./mock-consumers";
import { mockIgnored } from "./mock-ignored";
import { es } from "./i18n/es";
import { mockPreferences } from "./mock-preferences";
import { mockSettings } from "./mock-settings";
import { adoptionFailure, adoptionMessage } from "./adoption";
import type { Backend } from "./api";
import { sampleLog, sampleSnapshot } from "./sample";
import type { InstanceView, OperationRequest, OperationView, PanelState, Snapshot } from "./types";

export function mockBackend(): Backend {
  let state: Snapshot = sampleSnapshot();
  let panelState: PanelState = {
    version: 2,
    tab: "attention",
    openProjects: [],
    closedProjects: [],
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
  const consumers = mockConsumers(() => state, publish);
  const ignored = mockIgnored(() => state, publish);
  const classify = mockClassify(() => state, publish);
  const settings = mockSettings();
  const preferences = mockPreferences();
  return {
    adoptionOptions: adoption.adoptionOptions,
    previewAdoption: adoption.previewAdoption,
    validateAdoption: adoption.validateAdoption,
    sharedConsumers: consumers.sharedConsumers,
    rulePreview: ignored.rulePreview,
    ignoredRules: ignored.ignoredRules,
    preferences: preferences.preferences,
    setPreferences: preferences.setPreferences,
    scanFolders: settings.scanFolders,
    previewScanFolders: settings.previewScanFolders,
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
      if (request.kind === "setScanFolders") {
        settings.settle(request).forEach((settled, index) => {
          const result = op.targets[index];
          if (result) Object.assign(result, settled);
        });
        op.status = op.targets.every((item) => item.status === "succeeded")
          ? "succeeded"
          : "failed";
        op.revision++;
        opListeners.forEach((listener) => {
          listener(op);
        });
        return op;
      }
      if (request.kind === "setConsumers") {
        consumers.settle(request).forEach((settled, index) => {
          const result = op.targets[index];
          if (result) Object.assign(result, settled);
        });
        op.status = op.targets.every((item) => item.status === "succeeded")
          ? "succeeded"
          : "failed";
        op.revision++;
        opListeners.forEach((listener) => {
          listener(op);
        });
        return op;
      }
      for (const target of request.targets) {
        const ref = target.reference;
        try {
          if (request.kind === "stop" || request.kind === "stopProject") {
            find(ref);
            publish(without(ref));
          } else if (request.kind === "keep") {
            ignored.keep(ref);
          } else if (request.kind === "restore") {
            ignored.restore(ref);
          } else if (request.kind === "classify") {
            classify.set(ref, target.classification?.class ?? null);
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
            result.error = adoptionMessage(result.adoptionError, es);
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

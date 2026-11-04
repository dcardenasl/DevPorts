import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import type { Backend } from "./api";
import { App } from "./App";
import { mockAdoption } from "./mock-adoption";
import { mockClassify } from "./mock-classify";
import { mockConsumers } from "./mock-consumers";
import { mockIgnored } from "./mock-ignored";
import { mockPreferences } from "./mock-preferences";
import { mockSettings } from "./mock-settings";
import { sampleLog, sampleSnapshot } from "./sample";
import type {
  LanguagePreference,
  OperationRequest,
  OperationView,
  PanelState,
  Preferences,
  Snapshot,
} from "./types";

const hoisted = vi.hoisted(() => ({ current: undefined as Backend | undefined }));

vi.mock("./api", () => ({
  backend: () => Promise.resolve(hoisted.current),
}));

export function fakeBackend(
  snapshot: Snapshot,
  initialState?: PanelState,
  initialOperations: OperationView[] = [],
  savedPreferences?: Preferences,
): Backend & {
  calls: Record<string, unknown[][]>;
  getPersisted: () => PanelState;
  publishSnapshot: (next: Snapshot) => void;
} {
  const calls: Record<string, unknown[][]> = {};
  let persisted: PanelState = initialState ?? {
    version: 3,
    tab: "attention",
    openProjects: [],
    closedProjects: [],
    activityFilter: "all",
    draft: null,
  };
  const history: OperationView[] = [...initialOperations];
  const opListeners = new Set<(operation: OperationView) => void>();
  let currentSnapshot = snapshot;
  const listeners = new Set<(snapshot: Snapshot) => void>();
  const publish = (next: Snapshot): void => {
    currentSnapshot = { ...next, revision: currentSnapshot.revision + 1 };
    listeners.forEach((listener) => {
      listener(currentSnapshot);
    });
  };
  const adoption = mockAdoption(() => currentSnapshot, publish);
  const consumers = mockConsumers(() => currentSnapshot, publish);
  const ignored = mockIgnored(() => currentSnapshot, publish);
  const classify = mockClassify(() => currentSnapshot, publish);
  const settings = mockSettings();
  const preferences = mockPreferences(savedPreferences);
  const record =
    (name: string, result?: unknown) =>
    (...args: unknown[]) => {
      (calls[name] ??= []).push(args);
      return Promise.resolve(result);
    };
  return {
    calls,
    getPersisted: () => persisted,
    publishSnapshot: publish,
    snapshot: () => Promise.resolve(currentSnapshot),
    adoptionOptions: adoption.adoptionOptions,
    previewAdoption: (reference, target) => {
      void record("previewAdoption")(reference, target);
      return adoption.previewAdoption(reference, target);
    },
    validateAdoption: adoption.validateAdoption,
    sharedConsumers: consumers.sharedConsumers,
    rulePreview: ignored.rulePreview,
    ignoredRules: ignored.ignoredRules,
    preferences: preferences.preferences,
    setPreferences: (next) => {
      void record("setPreferences")(next);
      return preferences.setPreferences(next);
    },
    scanFolders: settings.scanFolders,
    previewScanFolders: settings.previewScanFolders,
    previewProjectStop: (project) => {
      void record("previewProjectStop")(project);
      return Promise.resolve(
        snapshot.instances.filter(
          (view) =>
            view.project === project &&
            view.kind === "app" &&
            !view.shared &&
            ["healthy", "unhealthy", "forgotten"].includes(view.state),
        ),
      );
    },
    panelState: () => Promise.resolve(persisted),
    savePanelState: (next) => {
      persisted = next;
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
      if (calls.operation === undefined) calls.operation = [];
      calls.operation.push([request]);
      const settled =
        request.kind === "setConsumers"
          ? consumers.settle(request)
          : request.kind === "setScanFolders"
            ? settings.settle(request)
            : [];
      for (const target of request.targets) {
        if (request.kind === "keep") ignored.keep(target.reference);
        if (request.kind === "restore") ignored.restore(target.reference);
        if (request.kind === "classify")
          classify.set(target.reference, target.classification?.class ?? null);
      }
      const op: OperationView = {
        id: request.id,
        kind: request.kind,
        status: settled.some((item) => item.status === "failed") ? "failed" : "succeeded",
        revision: 1,
        targets: request.targets.map((target, index) => ({
          reference: target.reference,
          adoptTarget: target.adoptTarget ?? null,
          stopExpectation: target.stopExpectation ?? null,
          status: "succeeded",
          error: null,
          result:
            request.kind === "adopt" && target.adoption
              ? adoption.apply(target.adoption.id, target.adoption.edits)
              : null,
          adoptionReview: target.adoption?.id ?? null,
          attempts: 1,
          ...settled[index],
        })),
      };
      history.unshift(op);
      opListeners.forEach((listener) => {
        listener(op);
      });
      return op;
    },
    retryOperation: async (id) => {
      calls.retryOperation = [[id]];
      await Promise.resolve();
      const operation = history.find((item) => item.id === id);
      if (!operation) throw new Error("No existe la operación");
      operation.status = "succeeded";
      operation.revision += 1;
      operation.targets.forEach((target) => {
        if (target.status === "failed") {
          target.status = "succeeded";
          target.error = null;
        }
      });
      opListeners.forEach((listener) => {
        listener(operation);
      });
      return operation;
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return Promise.resolve(() => {
        listeners.delete(listener);
      });
    },
    log: () => Promise.resolve(sampleLog()),
    openUrl: record("openUrl") as Backend["openUrl"],
    panelReady: record("panelReady") as Backend["panelReady"],
  };
}

/** The backend the mocked `api` module hands to the panel. */
export const state = hoisted;

export async function open(
  snapshot: Snapshot = sampleSnapshot(),
  language: LanguagePreference = "system",
) {
  // What the shell put in the URL is what the core has saved.
  const backend = fakeBackend(snapshot, undefined, [], { theme: "system", language });
  state.current = backend;
  const user = userEvent.setup();
  render(<App initialLanguage={language} />);
  await screen.findAllByRole("tab");
  return { backend, user };
}

export const tab = (name: RegExp) => screen.getByRole("tab", { name });

/** The reset every panel suite starts from: no backend yet, and jsdom has no layout to scroll. */
export function resetBackend(): void {
  state.current = undefined;
  Element.prototype.scrollIntoView = vi.fn();
}

// A fake core for developing the panel in a browser. Keeps its own state so the
// actions visibly work (stopping removes a row, extending adds time).

import type { Backend } from "./api";
import { sampleLog, sampleSnapshot } from "./sample";
import type { AdoptTarget, InstanceView, Snapshot } from "./types";

export function mockBackend(): Backend {
  let state: Snapshot = sampleSnapshot();
  const listeners = new Set<(snapshot: Snapshot) => void>();
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

  return {
    snapshot: () => Promise.resolve(state),
    subscribe: (listener) => {
      listeners.add(listener);
      return Promise.resolve(() => {
        listeners.delete(listener);
      });
    },
    stop: (ref) => {
      find(ref);
      publish(without(ref));
      return Promise.resolve();
    },
    extend: (ref) => {
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
      return Promise.resolve();
    },
    keep: (ref) => {
      find(ref);
      publish({
        ...state,
        instances: state.instances.map((candidate) =>
          candidate.ref === ref ? { ...candidate, ignored: true } : candidate,
        ),
        attention: state.attention.filter((item) => item.instance_ref !== ref),
      });
      return Promise.resolve();
    },
    adopt: (ref, target: AdoptTarget) => {
      find(ref);
      const project =
        target === "shared"
          ? "compartidos"
          : "project" in target
            ? target.project
            : (target.new ?? "proyecto-nuevo");
      publish(without(ref));
      return Promise.resolve({ project, service: "servicio" });
    },
    log: () => Promise.resolve(sampleLog()),
    openUrl: () => Promise.resolve(),
    panelReady: () => Promise.resolve(),
  };
}

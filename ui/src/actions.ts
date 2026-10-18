import type { InstanceView, Snapshot } from "./types";

/** What the views can ask for. `App` implements them against the core. */
export interface Actions {
  snapshot: Snapshot;
  now: number;
  /** Stops these exact instances after a single confirmation. */
  requestStop: (views: InstanceView[], invoker: HTMLButtonElement) => void;
  preparingProjectStop: boolean;
  requestProjectStop: (project: string, invoker: HTMLButtonElement) => void;
  extend: (view: InstanceView) => void;
  keep: (view: InstanceView) => void;
  openUrl: (url: string) => void;
  copy: (text: string) => void;
  showInProjects: (ref: string) => void;
  adopt: (view: InstanceView, invoker: HTMLButtonElement) => void;
  /** Opens the same reviewed declaration flow for a shared container. */
  declareShared: (view: InstanceView, invoker: HTMLButtonElement) => void;
}

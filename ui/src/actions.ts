import type { InstanceView, Snapshot } from "./types";

/** What the views can ask for. `App` implements them against the core. */
export interface Actions {
  snapshot: Snapshot;
  now: number;
  /** Stops these exact instances after a single confirmation. */
  requestStop: (views: InstanceView[], invoker: HTMLButtonElement) => void;
  extend: (view: InstanceView) => void;
  keep: (view: InstanceView) => void;
  openUrl: (url: string) => void;
  copy: (text: string) => void;
  showInProjects: (ref: string) => void;
  adopt: (view: InstanceView, invoker: HTMLButtonElement) => void;
  /** Declares a container in `shared.yml` without asking where it goes. */
  declareShared: (view: InstanceView) => void;
}

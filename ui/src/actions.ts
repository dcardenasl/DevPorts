import type { InstanceView, Snapshot } from "./types";

/** What the views can ask for. `App` implements them against the core. */
export interface Actions {
  snapshot: Snapshot;
  now: number;
  /** Stops it after the person confirms (always asks first). */
  requestStop: (view: InstanceView) => void;
  /** Stops without asking: duplicates and Temporals, which the card already offers. */
  stopNow: (views: InstanceView[]) => void;
  extend: (view: InstanceView) => void;
  keep: (view: InstanceView) => void;
  openUrl: (url: string) => void;
  copy: (text: string) => void;
  showInProjects: (ref: string) => void;
  adopt: (view: InstanceView) => void;
  /** Declares a container in `shared.yml` without asking where it goes. */
  declareShared: (view: InstanceView) => void;
}

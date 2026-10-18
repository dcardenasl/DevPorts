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
  /** Shows the exact rule and, once confirmed, ignores this service. */
  ignore: (view: InstanceView, invoker: HTMLButtonElement) => void;
  /** Lets the person correct where this process is listed, with the exact scope shown. */
  classify: (view: InstanceView, invoker: HTMLButtonElement) => void;
  /** Opens the scan-folder editor. */
  openSettings: (invoker: HTMLButtonElement) => void;
  /** Lists the saved ignore rules so each can be restored. */
  showIgnored: (invoker: HTMLButtonElement) => void;
  openUrl: (url: string) => void;
  copy: (text: string) => void;
  showInProjects: (ref: string) => void;
  adopt: (view: InstanceView, invoker: HTMLButtonElement) => void;
  /** Opens the same reviewed declaration flow for a shared container. */
  declareShared: (view: InstanceView, invoker: HTMLButtonElement) => void;
  /** Opens the consumers sheet for one shared service, optionally focused on a project. */
  manageConsumers: (shared: string, invoker: HTMLButtonElement, focus?: string) => void;
}

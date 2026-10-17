// The shapes the core sends are generated from the Rust types (`bindings.ts`,
// checked by `crates/devports-core/tests/bindings.rs`); only what the panel sends
// back to the core is written here.

export type {
  AttentionItem,
  AttentionKind,
  ChainLink,
  InstanceRole,
  InstanceState,
  InstanceView,
  LogEntry,
  ProjectSummary,
  RepoSummary,
  ServiceSummary,
  Snapshot,
} from "./bindings";

/** Where an adoption goes: an existing project, a new one, or `shared.yml`. */
export type AdoptTarget = { project: string } | { new: string | null } | "shared";

export type PanelTab = "attention" | "projects" | "shared" | "orphans" | "activity";
export type ActivityFilter = "all" | "rejections" | "temporals" | "alerts";
export type AdoptMode = "options" | "others" | "new";
export interface AdoptDraft {
  kind: "adopt";
  reference: string;
  mode: AdoptMode;
  name: string;
  lastStep: "others" | "new" | null;
}
export interface PanelState {
  version: 1;
  tab: PanelTab;
  expandedProject: string | null;
  expandedProjectSet: boolean;
  activityFilter: ActivityFilter;
  draft: AdoptDraft | null;
}

export type OperationKind = "stop" | "extend" | "keep" | "adopt";
export type OperationStatus = "queued" | "running" | "succeeded" | "partial" | "failed";
export type OperationTargetStatus = "queued" | "running" | "succeeded" | "failed";
export interface OperationTargetRequest {
  reference: string;
  adoptTarget?: AdoptTarget;
  stopExpectation?: StopExpectation;
}
export interface StopExpectation {
  targetIdentity: string | null;
  pid: number | null;
  container: string | null;
  port: number | null;
  project: string | null;
  service: string | null;
  kind: string;
  role: import("./bindings").InstanceRole;
  shared: boolean;
  startedAt: string | null;
}
export interface OperationRequest {
  id: string;
  kind: OperationKind;
  targets: OperationTargetRequest[];
}
export interface OperationTargetResult {
  reference: string;
  adoptTarget: AdoptTarget | null;
  stopExpectation?: StopExpectation | null;
  status: OperationTargetStatus;
  error: string | null;
  result: { project: string; service: string } | null;
  attempts: number;
}
export interface OperationView {
  id: string;
  kind: OperationKind;
  status: OperationStatus;
  revision: number;
  targets: OperationTargetResult[];
}

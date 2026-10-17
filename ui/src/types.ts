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

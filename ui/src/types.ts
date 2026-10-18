// The shapes the core sends are generated from the Rust types (`bindings.ts`,
// checked by `crates/devports-core/tests/bindings.rs`); only what the panel sends
// back to the core is written here.

export type {
  AdoptionDestination,
  AdoptionError,
  AdoptionOptions,
  AdoptionPreview,
  Classification,
  RootProblem,
  ScanFolders,
  ScanPreview,
  ScanRoot,
  SettingsError,
  ConsumerChoice,
  IgnoreRule,
  ProcessClass,
  ConsumerOptions,
  ServiceEdits,
  UseRequest,
  UsesError,
  AttentionItem,
  AttentionKind,
  ChainLink,
  InstanceRole,
  InstanceState,
  InstanceView,
  LogEntry,
  LogError,
  LanguagePreference,
  LogFacts,
  LogTopic,
  Preferences,
  ProjectSummary,
  RepoSummary,
  ServiceSummary,
  SharedSummary,
  Snapshot,
  ThemePreference,
} from "./bindings";

/** Where an adoption goes: an existing project, a new one, or `shared.yml`. */
export type AdoptTarget = import("./bindings").AdoptionTarget;

export type PanelTab = "attention" | "projects" | "shared" | "orphans" | "activity";
export type ActivityFilter = "all" | "rejections" | "temporals" | "alerts";
export type AdoptMode = "options" | "others" | "new" | "review";
export interface AdoptDraft {
  kind: "adopt";
  reference: string;
  mode: AdoptMode;
  name: string;
  lastStep: "others" | "new" | null;
  review?: {
    id: string;
    target: AdoptTarget;
    edits: import("./bindings").ServiceEdits;
    context: Pick<
      import("./bindings").AdoptionPreview,
      "root" | "manifest" | "project" | "createsManifest" | "observedPort" | "warnings"
    >;
  } | null;
  operationId?: string | null;
}
/** Projects ticked to consume one shared service; manifests are read again on restore. */
export interface ConsumersDraft {
  kind: "consumers";
  shared: string;
  selected: string[];
  reviewing: boolean;
  focus: string | null;
  operationId?: string | null;
}
/** The scan-folder list being edited; the files and their versions are read again on restore. */
export interface SettingsDraft {
  kind: "settings";
  roots: string[];
  reviewing: boolean;
  operationId?: string | null;
}
export type PanelDraft = AdoptDraft | ConsumersDraft | SettingsDraft;
export interface PanelState {
  version: 2;
  tab: PanelTab;
  /** Projects the person opened by hand. */
  openProjects: string[];
  /** Projects the person folded by hand. */
  closedProjects: string[];
  activityFilter: ActivityFilter;
  draft: PanelDraft | null;
}

export type OperationKind =
  | "stop"
  | "stopProject"
  | "extend"
  | "keep"
  | "adopt"
  | "setConsumers"
  | "restore"
  | "classify"
  | "setScanFolders";
export type OperationStatus = "queued" | "running" | "succeeded" | "partial" | "failed";
export type OperationTargetStatus = "queued" | "running" | "succeeded" | "failed";
export interface OperationTargetRequest {
  reference: string;
  adoptTarget?: AdoptTarget;
  adoption?: { id: string; edits: import("./bindings").ServiceEdits };
  stopExpectation?: StopExpectation;
  /** Consumer change: `reference` is the project name; `base` comes from `ConsumerChoice`. */
  consumers?: { shared: string; include: boolean; base: string };
  /** Classification change: `class: null` goes back to the automatic guess. */
  classification?: { class: import("./bindings").ProcessClass | null };
  /** Scan-folder change: `reference` is `config`; `base` comes from `ScanFolders`. */
  scanFolders?: { roots: string[]; base: string };
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
  adoptionReview?: string | null;
  adoptionError?: import("./bindings").AdoptionError | null;
  consumers?: { shared: string; include: boolean } | null;
  usesError?: import("./bindings").UsesError | null;
  classification?: { class: import("./bindings").ProcessClass | null } | null;
  scanFolderCount?: number | null;
  settingsError?: import("./bindings").SettingsError | null;
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

// The shapes the core sends. They mirror `devports-proto` and `runtime::Snapshot`;
// `contract.test.ts` checks them against a fixture the Rust tests also verify.

export type InstanceRole = "principal" | "temporal" | "unregistered";
export type InstanceState =
  "healthy" | "unhealthy" | "down" | "starting" | "stopping" | "forgotten";

export interface ChainLink {
  pid: number;
  name: string;
  label: string | null;
}

export interface InstanceView {
  ref: string;
  project: string | null;
  service: string | null;
  role: InstanceRole;
  state: InstanceState;
  kind: string;
  port: number | null;
  url: string | null;
  pid: number | null;
  container: string | null;
  branch: string | null;
  worktree: string | null;
  cwd: string | null;
  started_at: string | null;
  expires_at: string | null;
  reason: string | null;
  registered: boolean;
  launcher_chain: ChainLink[];
  duplicate_of: string | null;
  stale: string | null;
  shared: boolean;
  compose: string | null;
  argv: string[];
  image: string | null;
  ignored: boolean;
}

export interface RepoSummary {
  name: string;
  path: string;
  branch: string | null;
}

export interface ServiceSummary {
  name: string;
  repo: string;
  port: number | null;
  kind: string;
}

export interface ProjectSummary {
  name: string;
  root: string;
  uses: string[];
  repos: RepoSummary[];
  services: ServiceSummary[];
}

export type AttentionKind = "duplicate" | "expiring" | "hung";

export interface AttentionItem {
  kind: AttentionKind;
  instance_ref: string;
}

export interface Snapshot {
  revision: number;
  instances: InstanceView[];
  docker_reachable: boolean;
  docker_error: string | null;
  problems: string[];
  projects: ProjectSummary[];
  attention: AttentionItem[];
  extend_minutes: number;
}

export interface LogEntry {
  at: string;
  kind: string;
  project: string | null;
  service: string | null;
  summary: string;
}

export type AdoptTarget = { project: string } | { new: string | null };

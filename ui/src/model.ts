// What the panel shows, derived from the core's snapshot. No rules live here
// beyond layout: which things need a decision is decided by the core
// (`snapshot.attention`).

import { isActiveInstance } from "./instance";

import type {
  AttentionItem,
  InstanceView,
  PanelDraft,
  ProjectSummary,
  RepoSummary,
  ServiceSummary,
  Snapshot,
} from "./types";

export interface ServiceNode {
  name: string;
  port: number | null;
  kind: string;
  /** The Principal, or a "down" placeholder when nothing runs. */
  principal: InstanceView | undefined;
  temporals: InstanceView[];
  duplicates: InstanceView[];
}

export interface RepoNode {
  repo: RepoSummary;
  services: ServiceNode[];
}

export interface ProjectNode {
  summary: ProjectSummary;
  repos: RepoNode[];
  serviceCount: number;
  runningCount: number;
  unknownCount: number;
}

export interface SharedNode {
  name: string;
  principal: InstanceView;
  users: string[];
}

export interface DuplicateGroup {
  project: string;
  service: string;
  official: InstanceView | undefined;
  duplicates: InstanceView[];
}

export interface AttentionModel {
  hung: InstanceView[];
  expiring: InstanceView[];
  groups: DuplicateGroup[];
  /** Principals shown as fixed context above the alerts. */
  official: InstanceView[];
}

const isRunning = isActiveInstance;

export function indexByRef(instances: readonly InstanceView[]): Map<string, InstanceView> {
  return new Map(instances.map((view) => [view.ref, view]));
}

function principalOf(
  instances: readonly InstanceView[],
  project: string | null,
  service: string,
): InstanceView | undefined {
  return instances.find(
    (view) => view.role === "principal" && view.project === project && view.service === service,
  );
}

export function buildProjects(snapshot: Snapshot): ProjectNode[] {
  return snapshot.projects.map((summary) => {
    const repos: RepoNode[] = summary.repos.map((repo) => ({
      repo,
      services: summary.services
        .filter((service) => service.repo === repo.path)
        .map((service) => serviceNode(snapshot.instances, summary.name, service)),
    }));
    const nodes = repos.flatMap((repo) => repo.services);
    return {
      summary,
      repos,
      serviceCount: nodes.length,
      unknownCount: snapshot.instances.filter(
        (view) => view.project === summary.name && view.state === "unknown",
      ).length,
      runningCount: nodes.filter(
        (node) =>
          isRunning(node.principal) ||
          node.temporals.some(isRunning) ||
          node.duplicates.some(isRunning),
      ).length,
    };
  });
}

function serviceNode(
  instances: readonly InstanceView[],
  project: string,
  service: ServiceSummary,
): ServiceNode {
  const mine = instances.filter(
    (view) => view.project === project && view.service === service.name,
  );
  return {
    name: service.name,
    port: service.port,
    kind: service.kind,
    principal: principalOf(instances, project, service.name),
    temporals: mine.filter((view) => view.role === "temporal"),
    duplicates: mine.filter((view) => view.role === "unregistered"),
  };
}

export interface PortChip {
  port: number;
  tone: "green" | "coral" | "amber";
}

/** Ports a project occupies right now, ascending, with the health of whoever holds them. */
export function occupiedPorts(node: ProjectNode): PortChip[] {
  const chips = new Map<number, PortChip["tone"]>();
  for (const service of node.repos.flatMap((repo) => repo.services)) {
    const holders: [InstanceView | undefined, boolean][] = [
      [service.principal, false],
      ...service.temporals.map((view): [InstanceView, boolean] => [view, false]),
      ...service.duplicates.map((view): [InstanceView, boolean] => [view, true]),
    ];
    for (const [view, duplicate] of holders) {
      if (view === undefined || view.port === null || !isRunning(view) || view.ignored) continue;
      // A duplicate is coral whatever its health: it is the thing to deal with.
      chips.set(
        view.port,
        duplicate || view.state === "unhealthy"
          ? "coral"
          : view.state === "healthy"
            ? "green"
            : "amber",
      );
    }
  }
  return [...chips].sort(([a], [b]) => a - b).map(([port, tone]) => ({ port, tone }));
}

/** Projects with something running (or unconfirmed) first, then by name; stable inside each. */
export function orderProjects(nodes: readonly ProjectNode[]): ProjectNode[] {
  const active = (node: ProjectNode): number =>
    node.runningCount > 0 || node.unknownCount > 0 ? 0 : 1;
  return [...nodes].sort(
    (a, b) => active(a) - active(b) || a.summary.name.localeCompare(b.summary.name),
  );
}

export function buildShared(snapshot: Snapshot): SharedNode[] {
  return snapshot.instances
    .filter((view) => view.shared && view.role === "principal" && view.service !== null)
    .map((principal) => ({
      name: principal.service ?? "",
      principal,
      users: snapshot.projects
        .filter((project) => project.uses.includes(principal.service ?? ""))
        .map((project) => project.name),
    }));
}

const isLoose = (view: InstanceView): boolean =>
  view.role === "unregistered" && view.service === null && !view.ignored;

/** Running databases and caches nobody declared in `shared.yml` yet. */
export function buildUndeclaredData(snapshot: Snapshot): InstanceView[] {
  return snapshot.instances.filter(
    (view) => isLoose(view) && view.container !== null && view.kind === "data",
  );
}

/** Loose processes by where the panel lists them. Only the core's evidence classifies. */
export interface OrphanGroups {
  project: InstanceView[];
  unclassified: InstanceView[];
  auxiliary: InstanceView[];
}

/** Splits the loose processes by class; one with no classification stays unclassified. */
export function groupOrphans(snapshot: Snapshot): OrphanGroups {
  const byRef = new Map(snapshot.classifications.map((item) => [item.instanceRef, item]));
  const groups: OrphanGroups = { project: [], unclassified: [], auxiliary: [] };
  for (const view of buildOrphans(snapshot)) {
    groups[byRef.get(view.ref)?.class ?? "unclassified"].push(view);
  }
  return groups;
}

/** Things that listen on a port and belong to no declared service. */
export function buildOrphans(snapshot: Snapshot): InstanceView[] {
  const undeclared = new Set(buildUndeclaredData(snapshot).map((view) => view.ref));
  return snapshot.instances.filter((view) => isLoose(view) && !undeclared.has(view.ref));
}

export function buildAttention(snapshot: Snapshot): AttentionModel {
  const byRef = indexByRef(snapshot.instances);
  const pick = (kind: AttentionItem["kind"]): InstanceView[] =>
    snapshot.attention
      .filter((item) => item.kind === kind)
      .flatMap((item) => byRef.get(item.instance_ref) ?? []);

  const hung = pick("hung");
  const expiring = pick("expiring");
  const groups = new Map<string, DuplicateGroup>();
  for (const duplicate of pick("duplicate")) {
    const project = duplicate.project ?? "";
    const service = duplicate.service ?? "";
    const key = `${project}\u0000${service}`;
    const group = groups.get(key) ?? {
      project,
      service,
      official: duplicate.duplicate_of === null ? undefined : byRef.get(duplicate.duplicate_of),
      duplicates: [],
    };
    group.duplicates.push(duplicate);
    groups.set(key, group);
  }

  const official = new Map<string, InstanceView>();
  for (const group of groups.values()) {
    if (group.official !== undefined) official.set(group.official.ref, group.official);
  }
  for (const temporal of expiring) {
    const principal =
      temporal.service === null
        ? undefined
        : principalOf(snapshot.instances, temporal.project, temporal.service);
    if (principal !== undefined && isRunning(principal)) official.set(principal.ref, principal);
  }
  return { hung, expiring, groups: [...groups.values()], official: [...official.values()] };
}

/** Projects that stop working when `view` is stopped. */
export function dependentsOf(view: InstanceView, snapshot: Snapshot): string[] {
  if (view.shared && view.service !== null) {
    return snapshot.projects
      .filter((project) => project.uses.includes(view.service ?? ""))
      .map((project) => project.name);
  }
  if (view.kind === "data" && view.project !== null) {
    const project = snapshot.projects.find((candidate) => candidate.name === view.project);
    return (
      project?.services
        .filter((service) => service.kind !== "data" && service.name !== view.service)
        .map((service) => `${project.name} · ${service.name}`) ?? []
    );
  }
  return [];
}

/** The best command to start a service that is not running. */
export function startCommand(project: ProjectSummary | undefined, service: string): string {
  const where = project === undefined ? "" : `cd ${shellQuote(project.root)} && `;
  return `${where}devports run ${service}`;
}

function shellQuote(path: string): string {
  return /^[\w@%+=:,./-]+$/.test(path) ? path : `'${path.replaceAll("'", "'\\''")}'`;
}

export function whoAsked(view: InstanceView): string | null {
  const link = view.launcher_chain.find((candidate) => candidate.label !== null);
  return link?.label ?? view.launcher_chain[0]?.name ?? null;
}

/**
 * Whether what a draft refers to is still there. A draft with a sent operation is
 * kept either way: its result matters more than its subject.
 */
export function draftTargetExists(draft: PanelDraft, snapshot: Snapshot): boolean {
  if (draft.operationId != null || draft.kind === "settings") return true;
  return draft.kind === "adopt"
    ? snapshot.instances.some((view) => view.ref === draft.reference)
    : snapshot.shared.some((item) => item.name === draft.shared);
}

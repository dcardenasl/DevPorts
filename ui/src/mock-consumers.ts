// Browser-only fixture for consumer changes. Manifest validation stays the Rust core's job;
// this keeps the catalog and snapshot coherent so the panel can be developed without Tauri.
import { usesMessage } from "./consumers";
import type {
  ConsumerOptions,
  OperationRequest,
  OperationTargetResult,
  Snapshot,
  UseRequest,
  UsesError,
} from "./types";

export type ConsumerResult = { ok: true } | { ok: false; error: UsesError };
type Settled = Pick<OperationTargetResult, "consumers" | "status" | "error" | "usesError">;

const version = (uses: string[]): string => JSON.stringify(uses);

export function mockConsumers(get: () => Snapshot, publish: (next: Snapshot) => void) {
  const defined = (shared: string): boolean =>
    get().instances.some((view) => view.shared && view.service === shared);
  const validate = (shared: string, request: UseRequest): UsesError | undefined => {
    const project = get().projects.find((candidate) => candidate.name === request.project);
    if (project === undefined) return { code: "unknownProject", project: request.project };
    if (request.include && !defined(shared)) return { code: "unknownShared", name: shared };
    // Already in the requested state: nothing to write, whatever the version.
    if (project.uses.includes(shared) === request.include) return undefined;
    if (request.base !== version(project.uses))
      return { code: "documentChanged", path: `${project.root}/.devports.yml` };
    return undefined;
  };
  /** Validates every request first; writes nothing when any fails. */
  const apply = (shared: string, requests: UseRequest[]): ConsumerResult[] => {
    const errors = requests.map((request) => validate(shared, request));
    if (errors.some((error) => error !== undefined))
      return errors.map((error) => ({ ok: false, error: error ?? { code: "blocked" } }));
    const state = get();
    publish({
      ...state,
      projects: state.projects.map((project) => {
        const request = requests.find((candidate) => candidate.project === project.name);
        if (request === undefined) return project;
        const rest = project.uses.filter((name) => name !== shared);
        return { ...project, uses: request.include ? [...rest, shared] : rest };
      }),
    });
    return requests.map(() => ({ ok: true }));
  };
  return {
    sharedConsumers: async (shared: string): Promise<ConsumerOptions> => {
      await Promise.resolve();
      return {
        shared,
        defined: defined(shared),
        consumers: [...get().projects]
          .sort((a, b) => a.name.localeCompare(b.name))
          .map((project) => ({
            project: project.name,
            root: project.root,
            manifest: `${project.root}/.devports.yml`,
            declared: project.uses.includes(shared),
            eligible: true,
            reason: null,
            base: version(project.uses),
          })),
      };
    },
    apply,
    /**
     * Runs a whole `setConsumers` request the way the shell does: one all-or-nothing
     * batch, one result per target.
     */
    settle: (request: OperationRequest): Settled[] => {
      const shared = request.targets[0]?.consumers?.shared ?? "";
      const outcomes = apply(
        shared,
        request.targets.map((target) => ({
          project: target.reference,
          include: target.consumers?.include ?? false,
          base: target.consumers?.base ?? "",
        })),
      );
      return request.targets.map((target, index): Settled => {
        const outcome = outcomes[index];
        const consumers = target.consumers
          ? { shared: target.consumers.shared, include: target.consumers.include }
          : null;
        return outcome?.ok === false
          ? {
              consumers,
              status: "failed",
              error: usesMessage(outcome.error),
              usesError: outcome.error,
            }
          : { consumers, status: "succeeded", error: null, usesError: null };
      });
    },
  };
}

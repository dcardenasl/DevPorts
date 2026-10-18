// Browser-only fixture for consumer changes. Manifest validation stays the Rust core's job;
// this keeps the catalog and snapshot coherent so the panel can be developed without Tauri.
import type { ConsumerOptions, Snapshot, UseRequest, UsesError } from "./types";

export type ConsumerResult = { ok: true } | { ok: false; error: UsesError };

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
    /** Validates every request first; writes nothing when any fails. */
    apply: (shared: string, requests: UseRequest[]): ConsumerResult[] => {
      const errors = requests.map((request) => validate(shared, request));
      if (errors.some((error) => error !== undefined))
        return errors.map((error) => ({
          ok: false,
          error: error ?? { code: "blocked" },
        }));
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
    },
  };
}

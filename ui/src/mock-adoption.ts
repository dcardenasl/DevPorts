// Browser-only adoption fixture. Filesystem validation remains the Rust core's job.
import { adoptionFailure, adoptionMessage } from "./adoption";
import { es } from "./i18n/es";
import type {
  AdoptTarget,
  AdoptionError,
  AdoptionOptions,
  AdoptionPreview,
  Unsuitability,
  InstanceView,
  ServiceEdits,
  Snapshot,
} from "./types";

const slug = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 63) || "servicio";
const within = (cwd: string | null, root: string): boolean =>
  cwd !== null && (cwd === root || cwd.startsWith(`${root}/`));
/** Error instance carrying the same typed payload as Tauri's rejected JSON. */
function adoptionException(error: AdoptionError): Error & AdoptionError {
  return Object.assign(new Error(adoptionMessage(error, es)), error);
}
const fail = (reason: Unsuitability): never => {
  throw adoptionException({ code: "unsuitable", reason });
};

export function mockAdoption(get: () => Snapshot, publish: (next: Snapshot) => void) {
  const reviews = new Map<
    string,
    {
      preview: AdoptionPreview;
      identity: string | null;
      applied?: { project: string; service: string };
      fields?: string;
    }
  >();
  const find = (reference: string): InstanceView =>
    get().instances.find((view) => view.ref === reference) ?? fail({ code: "instanceUnavailable" });
  const prepare = (reference: string, target: AdoptTarget): AdoptionPreview => {
    const state = get();
    const view = find(reference);
    if (view.service !== null || view.state === "down" || view.state === "unknown")
      fail({ code: "instanceUnavailable" });
    const shared = target === "shared";
    if (shared && view.container === null) fail({ code: "sharedNeedsContainer" });
    const existing =
      typeof target === "object" && "project" in target
        ? (state.projects.find((project) => project.name === target.project) ??
          fail({ code: "projectNotInCatalog" }))
        : undefined;
    const root = shared
      ? "/Users/demo/.config/devports"
      : (existing?.root ?? view.cwd ?? fail({ code: "noKnownFolder" }));
    if (!shared && view.container === null && !within(view.cwd, root))
      fail({ code: "outsideProjectRoot" });
    const project = shared
      ? "compartidos"
      : (existing?.name ??
        (typeof target === "object" && "new" in target ? target.new : null) ??
        slug(root.split("/").pop() ?? "proyecto"));
    if (!/^[a-z][a-z0-9-]{0,62}$/.test(project))
      fail({ code: "other", detail: "El nombre de proyecto no es válido" });
    const taken = new Set(
      shared
        ? state.instances
            .filter((candidate) => candidate.shared)
            .map((candidate) => candidate.service)
        : existing?.services.map((service) => service.name),
    );
    const base = slug(view.container ?? "servidor");
    let name = base;
    for (let number = 2; taken.has(name); number++)
      name = `${base.slice(0, 63 - String(number).length - 1)}-${number}`;
    const start =
      view.container !== null
        ? `docker start '${view.container.replaceAll("'", "'\\''")}'`
        : view.argv.length === 0
          ? null
          : view.argv.map((arg) => `'${arg.replaceAll("'", "'\\''")}'`).join(" ");
    return {
      id: crypto.randomUUID(),
      reference,
      target,
      root,
      manifest: `${root}/${shared ? "shared.yml" : ".devports.yml"}`,
      project,
      createsManifest: shared ? false : existing === undefined || existing.services.length === 0,
      observedPort: view.port,
      service: {
        name,
        cwd:
          shared || view.container !== null
            ? null
            : view.cwd === root
              ? "."
              : `.${view.cwd?.slice(root.length) ?? ""}`,
        docker: view.container,
        port: view.port,
        start,
        matchText: view.argv.length > 0 ? view.argv.join(" ") : null,
        isData: view.kind === "data",
        review: null,
      },
      warnings: start === null ? [{ code: "noStartCommand" }] : [],
      yaml: "",
    };
  };
  const validate = (id: string, edits: ServiceEdits): AdoptionPreview => {
    const stored = reviews.get(id);
    if (!stored) throw adoptionException({ code: "previewExpired" });
    if (stored.applied) throw adoptionException({ code: "requestChanged" });
    if (!/^[a-z][a-z0-9-]{0,62}$/.test(edits.name))
      throw adoptionException({
        code: "invalidField",
        field: "name",
        problem: "invalidName",
      });
    if (
      edits.start !== null &&
      (edits.start.trim() === "" ||
        /[\n\r\0]/.test(edits.start) ||
        new TextEncoder().encode(edits.start).length > 16_384)
    )
      throw adoptionException({
        code: "invalidField",
        field: "start",
        problem: "invalidStart",
      });
    const preview = stored.preview;
    const taken =
      preview.target === "shared"
        ? get().instances.some((view) => view.shared && view.service === edits.name)
        : get()
            .projects.find((project) => project.name === preview.project)
            ?.services.some((service) => service.name === edits.name);
    if (taken)
      throw adoptionException({
        code: "invalidField",
        field: "name",
        problem: "nameTaken",
      });
    const view = find(preview.reference);
    if (view.target_identity !== stored.identity || view.service !== null)
      throw adoptionException({ code: "targetChanged" });
    return {
      ...preview,
      service: { ...preview.service, ...edits },
      warnings: edits.start === null ? [{ code: "noStartCommand" }] : preview.warnings,
      yaml: `version: 1\nproject: ${JSON.stringify(preview.project)}\nservices:\n  ${JSON.stringify(edits.name)}:\n    start: ${JSON.stringify(edits.start)}\n`,
    };
  };
  return {
    adoptionOptions: async (reference: string): Promise<AdoptionOptions> => {
      await Promise.resolve();
      return {
        reference,
        destinations: [
          ...get().projects.map((project) => ({ project: project.name })),
          { new: null },
          "shared" as const,
        ].map((target) => {
          try {
            const preview = prepare(reference, target);
            return {
              target,
              eligible: true,
              reason: null,
              root: preview.root,
              manifest: preview.manifest,
              createsManifest: preview.createsManifest,
            };
          } catch (error) {
            return {
              target,
              eligible: false,
              reason: adoptionFailure(error),
              root: null,
              manifest: null,
              createsManifest: false,
            };
          }
        }),
      };
    },
    previewAdoption: async (reference: string, target: AdoptTarget): Promise<AdoptionPreview> => {
      await Promise.resolve();
      const preview = prepare(reference, target);
      reviews.set(preview.id, { preview, identity: find(reference).target_identity });
      if (reviews.size > 8) reviews.delete(reviews.keys().next().value ?? "");
      return validate(preview.id, { name: preview.service.name, start: preview.service.start });
    },
    validateAdoption: async (id: string, edits: ServiceEdits) => {
      await Promise.resolve();
      return validate(id, edits);
    },
    apply: (id: string, edits: ServiceEdits): { project: string; service: string } => {
      const stored = reviews.get(id);
      if (!stored) throw adoptionException({ code: "previewExpired" });
      if (stored.applied) {
        if (stored.fields !== JSON.stringify(edits))
          throw adoptionException({ code: "requestChanged" });
        return stored.applied;
      }
      const preview = validate(id, edits);
      const shared = preview.target === "shared";
      const state = get();
      const service = {
        name: edits.name,
        repo: preview.root,
        port: preview.observedPort,
        kind: preview.service.isData ? "data" : "app",
      };
      const existing = state.projects.find((project) => project.name === preview.project);
      publish({
        ...state,
        instances: state.instances.map((view) =>
          view.ref === preview.reference
            ? {
                ...view,
                project: shared ? null : preview.project,
                service: edits.name,
                shared,
                role: "principal",
                kind: service.kind,
              }
            : view,
        ),
        attention: state.attention.filter((item) => item.instance_ref !== preview.reference),
        projects: shared
          ? state.projects
          : existing
            ? state.projects.map((project) =>
                project === existing
                  ? { ...project, services: [...project.services, service] }
                  : project,
              )
            : [
                ...state.projects,
                {
                  name: preview.project,
                  root: preview.root,
                  uses: [],
                  repos: [
                    {
                      name: preview.root.split("/").pop() ?? preview.project,
                      path: preview.root,
                      branch: null,
                    },
                  ],
                  services: [service],
                },
              ],
      });
      stored.fields = JSON.stringify(edits);
      stored.applied = { project: preview.project, service: edits.name };
      return stored.applied;
    },
  };
}

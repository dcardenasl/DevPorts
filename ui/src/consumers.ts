// Pure helpers for the consumers sheet: what changes, what is sent, how failures read.
import type {
  ConsumerOptions,
  ConsumersDraft,
  OperationRequest,
  OperationTargetResult,
  Snapshot,
  UsesError,
} from "./types";

export interface ConsumerChange {
  project: string;
  include: boolean;
  base: string;
  manifest: string;
}

/** The draft for managing `shared`, starting from what the manifests declare now. */
export function consumersDraft(
  snapshot: Snapshot,
  shared: string,
  focus: string | null,
): ConsumersDraft {
  return {
    kind: "consumers",
    shared,
    selected: snapshot.projects
      .filter((project) => project.uses.includes(shared))
      .map((project) => project.name),
    reviewing: false,
    focus,
    operationId: null,
  };
}

/**
 * Projects whose declaration differs from the selection. Projects that cannot be
 * edited never produce a change: they stay listed with their reason.
 */
export function consumerChanges(
  options: ConsumerOptions,
  selected: readonly string[],
): ConsumerChange[] {
  return options.consumers.flatMap((choice) =>
    choice.eligible && choice.base !== null && selected.includes(choice.project) !== choice.declared
      ? [
          {
            project: choice.project,
            include: !choice.declared,
            base: choice.base,
            manifest: choice.manifest,
          },
        ]
      : [],
  );
}

export function consumersRequest(
  id: string,
  shared: string,
  changes: readonly ConsumerChange[],
): OperationRequest {
  return {
    id,
    kind: "setConsumers",
    targets: changes.map((change) => ({
      reference: change.project,
      consumers: { shared, include: change.include, base: change.base },
    })),
  };
}

/** Failures that only a fresh read of the manifests can resolve. */
export function needsFreshRead(error: UsesError | null | undefined): boolean {
  return error?.code === "documentChanged" || error?.code === "unknownProject";
}

export function usesMessage(error: UsesError): string {
  switch (error.code) {
    case "unknownShared":
      return `«${error.name}» ya no está definido en shared.yml. Decláralo antes de asociarlo.`;
    case "unknownProject":
      return `El proyecto «${error.project}» ya no existe.`;
    case "documentChanged":
      return `${error.path} cambió desde que lo revisaste. Consulta de nuevo antes de guardar.`;
    case "blocked":
      return "No se escribió nada: otro proyecto de esta selección no pasó la validación.";
    default:
      return error.message;
  }
}

/** The failure to show for one target: typed first, the shell's text otherwise. */
export function targetFailure(target: OperationTargetResult): string | undefined {
  if (target.status !== "failed") return undefined;
  return target.usesError ? usesMessage(target.usesError) : (target.error ?? undefined);
}

const USES_CODES = [
  "unknownShared",
  "unknownProject",
  "ineligible",
  "documentChanged",
  "unsupported",
  "blocked",
  "invalidRequest",
  "io",
];

/** A rejected call as text: typed core errors keep their meaning, anything else its message. */
export function failureMessage(error: unknown): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    USES_CODES.includes(error.code)
  )
    return usesMessage(error as UsesError);
  return error instanceof Error ? error.message : String(error);
}

// Pure helpers for the consumers sheet: what changes, what is sent, how failures read.
import type { Catalog } from "./i18n/es";
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

export function usesMessage(error: UsesError, text: Catalog): string {
  switch (error.code) {
    case "unknownShared":
      return text.errors.uses.unknownShared(error.name);
    case "unknownProject":
      return text.errors.uses.unknownProject(error.project);
    case "documentChanged":
      return text.errors.uses.documentChanged(error.path);
    case "blocked":
      return text.errors.uses.blocked;
    default:
      return error.message;
  }
}

/** The failure to show for one target: typed first, the shell's text otherwise. */
export function targetFailure(target: OperationTargetResult, text: Catalog): string | undefined {
  if (target.status !== "failed") return undefined;
  return target.usesError ? usesMessage(target.usesError, text) : (target.error ?? undefined);
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
export function failureMessage(error: unknown, text: Catalog): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    USES_CODES.includes(error.code)
  )
    return usesMessage(error as UsesError, text);
  return error instanceof Error ? error.message : String(error);
}

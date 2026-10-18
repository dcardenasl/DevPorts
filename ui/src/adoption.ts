import type { Catalog } from "./i18n/es";
import { fieldProblemText, limitationText, unsuitabilityText } from "./reasons";
import type { AdoptDraft, AdoptionError, AdoptionPreview } from "./types";

/** Display-only context; the core owns every fact used to authorize the write. */
export function reviewDraft(preview: AdoptionPreview): NonNullable<AdoptDraft["review"]> {
  return {
    id: preview.id,
    target: preview.target,
    edits: { name: preview.service.name, start: preview.service.start },
    context: {
      root: preview.root,
      manifest: preview.manifest,
      project: preview.project,
      createsManifest: preview.createsManifest,
      observedPort: preview.observedPort,
      warnings: preview.warnings,
    },
  };
}

const ADOPTION_CODES = [
  "unsuitable",
  "invalidField",
  "documentChanged",
  "targetChanged",
  "previewExpired",
  "requestChanged",
  "atCapacity",
  "unsupported",
  "closing",
  "io",
];

/** A rejected call as a typed error when the core sent one, else as an input/output failure. */
export function adoptionFailure(failure: unknown): AdoptionError {
  if (
    typeof failure === "object" &&
    failure !== null &&
    "code" in failure &&
    typeof failure.code === "string" &&
    ADOPTION_CODES.includes(failure.code)
  )
    return failure as AdoptionError;
  return { code: "io", detail: failure instanceof Error ? failure.message : String(failure) };
}

export function needsNewReview(error: AdoptionError | undefined): boolean {
  return (
    error !== undefined &&
    ["documentChanged", "targetChanged", "previewExpired", "requestChanged"].includes(error.code)
  );
}

export function adoptionMessage(error: AdoptionError, text: Catalog): string {
  switch (error.code) {
    case "unsuitable":
      return unsuitabilityText(error.reason, text);
    case "invalidField":
      return fieldProblemText(error.problem, text);
    case "documentChanged":
      return text.errors.adoption.documentChanged(error.path);
    case "targetChanged":
    case "previewExpired":
    case "requestChanged":
    case "atCapacity":
      return text.errors.adoption[error.code];
    case "unsupported":
      return limitationText(error.limitation, text);
    case "closing":
      return text.reasons.closing;
    case "io":
      return text.reasons.io(error.detail);
  }
}

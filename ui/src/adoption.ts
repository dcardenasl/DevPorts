import type { Catalog } from "./i18n/es";
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

export function adoptionFailure(failure: unknown): AdoptionError {
  if (typeof failure === "object" && failure !== null && "code" in failure) {
    const code = failure.code;
    if (
      (code === "invalidField" &&
        "field" in failure &&
        typeof failure.field === "string" &&
        "message" in failure &&
        typeof failure.message === "string") ||
      (code === "documentChanged" && "path" in failure && typeof failure.path === "string") ||
      (["unsuitable", "unsupported", "io"].includes(String(code)) &&
        "message" in failure &&
        typeof failure.message === "string") ||
      ["targetChanged", "previewExpired", "requestChanged", "atCapacity"].includes(String(code))
    ) {
      return failure as AdoptionError;
    }
  }
  return { code: "io", message: failure instanceof Error ? failure.message : String(failure) };
}

export function needsNewReview(error: AdoptionError | undefined): boolean {
  return (
    error !== undefined &&
    ["documentChanged", "targetChanged", "previewExpired", "requestChanged"].includes(error.code)
  );
}

export function adoptionMessage(error: AdoptionError, text: Catalog): string {
  switch (error.code) {
    case "documentChanged":
      return text.errors.adoption.documentChanged(error.path);
    case "targetChanged":
    case "previewExpired":
    case "requestChanged":
    case "atCapacity":
      return text.errors.adoption[error.code];
    default:
      return error.message;
  }
}

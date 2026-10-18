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

export function adoptionMessage(error: AdoptionError): string {
  switch (error.code) {
    case "documentChanged":
      return `${error.path} cambió. Prepara una nueva revisión antes de guardar.`;
    case "targetChanged":
      return "La instancia o su destino cambiaron. Vuelve a revisar la propuesta.";
    case "previewExpired":
      return "La revisión caducó. Tus campos se conservan; prepara otra revisión.";
    case "requestChanged":
      return "Esta revisión ya pertenece a otro guardado. Consulta su resultado o prepara otra revisión.";
    case "atCapacity":
      return "Hay demasiadas operaciones en curso. Espera y vuelve a intentarlo.";
    default:
      return error.message;
  }
}

/** Error instance carrying the same typed payload as Tauri's rejected JSON. */
export function adoptionException(error: AdoptionError): Error & AdoptionError {
  return Object.assign(new Error(adoptionMessage(error)), error);
}

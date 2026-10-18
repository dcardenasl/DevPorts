// Browser-only fixture for classification corrections. Which evidence classifies stays
// the Rust core's job; this keeps the snapshot coherent so the panel works without Tauri.
import type { Classification, ProcessClass, Snapshot } from "./types";

export function mockClassify(get: () => Snapshot, publish: (next: Snapshot) => void) {
  return {
    /** Applies a correction, or goes back to the detected class with `null`. */
    set: (reference: string, choice: ProcessClass | null): void => {
      const state = get();
      if (!state.instances.some((view) => view.ref === reference))
        throw new Error(`no existe la instancia «${reference}»`);
      const known = state.classifications.find((item) => item.instanceRef === reference);
      const next: Classification = {
        instanceRef: reference,
        class: "unclassified",
        detected: "unclassified",
        program: null,
        origin: null,
        signals: [],
        ...known,
        corrected: choice !== null,
      };
      next.class = choice ?? next.detected;
      publish({
        ...state,
        classifications: [
          ...state.classifications.filter((item) => item.instanceRef !== reference),
          next,
        ],
      });
    },
  };
}

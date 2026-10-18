// Browser-only fixture for the saved preferences: an in-memory copy, so the panel can
// change its theme and language without Tauri. Persistence is the Rust core's job.
import type { Preferences } from "./types";

export function mockPreferences(initial: Preferences = { theme: "system", language: "system" }) {
  let current = initial;
  return {
    preferences: (): Promise<Preferences> => Promise.resolve(current),
    setPreferences: (next: Preferences): Promise<Preferences> => {
      current = next;
      return Promise.resolve(current);
    },
  };
}

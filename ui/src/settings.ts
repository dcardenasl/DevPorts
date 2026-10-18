// Pure helpers for the settings sheet: how failures read and whether the list changed.
import type { Catalog } from "./i18n/es";
import { ineligibilityText, limitationText, listProblemText } from "./reasons";
import type { ScanFolders, SettingsError } from "./types";

export function settingsMessage(error: SettingsError, text: Catalog): string {
  switch (error.code) {
    case "ineligible":
      return ineligibilityText(error.reason, text);
    case "invalidRoots":
      return text.errors.settings.invalidRoots;
    case "invalidList":
      return listProblemText(error.reason, text);
    case "documentChanged":
      return text.errors.settings.documentChanged(error.path);
    case "unsupported":
      return limitationText(error.limitation, text);
    case "io":
      return text.reasons.io(error.detail);
  }
}

const CODES = ["ineligible", "invalidRoots", "invalidList", "documentChanged", "unsupported", "io"];

/** A rejected call as a typed error when the core sent one, else as plain text. */
export function settingsFailure(error: unknown): SettingsError {
  if (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    typeof error.code === "string" &&
    CODES.includes(error.code)
  )
    return error as SettingsError;
  return { code: "io", detail: error instanceof Error ? error.message : String(error) };
}

/** The folders as written in `config.yml`, to start editing from. */
export function configuredRoots(folders: ScanFolders): string[] {
  return folders.roots.map((root) => root.configured);
}

/** Whether the typed list differs from what `config.yml` says now. */
export function rootsChanged(folders: ScanFolders, typed: readonly string[]): boolean {
  const current = configuredRoots(folders);
  return (
    typed.length !== current.length || typed.some((root, index) => root.trim() !== current[index])
  );
}

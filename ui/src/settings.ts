// Pure helpers for the settings sheet: how failures read and whether the list changed.
import type { ScanFolders, SettingsError } from "./types";

export function settingsMessage(error: SettingsError): string {
  switch (error.code) {
    case "invalidRoots":
      return "Hay carpetas que no se pueden usar. Corrígelas y vuelve a revisar.";
    case "documentChanged":
      return `${error.path} cambió desde que lo revisaste. Vuelve a leer la configuración antes de guardar.`;
    default:
      return error.message;
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
  return { code: "io", message: error instanceof Error ? error.message : String(error) };
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

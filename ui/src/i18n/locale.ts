// Which language the panel speaks (ADR-041). The preference is `system`, `es` or `en`;
// what the catalogs see is always one `Locale`.
import type { LanguagePreference } from "../types";

export type Locale = "es" | "en";

export const LOCALES: readonly Locale[] = ["es", "en"];

/** What the shell puts in the panel URL; anything else, or nothing, is `system`. */
export function parseLanguagePreference(value: string | null): LanguagePreference {
  return value === "es" || value === "en" ? value : "system";
}

/**
 * `system` is the first of the system's preferred languages this build has a catalog for
 * (`es-CL` is Spanish, `en-GB` is English); none of them → English. The Rust side applies the
 * same rule to the tray (`crates/devports-app/src/i18n.rs`); both are tested with the same
 * table.
 */
export function resolveLocale(
  preference: LanguagePreference,
  systemTags: readonly string[],
): Locale {
  if (preference !== "system") return preference;
  for (const tag of systemTags) {
    const language = tag.split(/[-_]/)[0]?.toLowerCase();
    const known = LOCALES.find((locale) => locale === language);
    if (known !== undefined) return known;
  }
  return "en";
}

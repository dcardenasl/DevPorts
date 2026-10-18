// Which theme the panel wears (ADR-040). The preference is `system`, `light` or `dark`;
// what the CSS sees is always one explicit `data-theme`, so there is one set of tokens per
// theme and no media query to keep in step with it.
import type { ThemePreference } from "./types";

export type ResolvedTheme = "light" | "dark";

/** What the shell puts in the panel URL; anything else, or nothing, is `system`. */
export function parseThemePreference(value: string | null): ThemePreference {
  return value === "light" || value === "dark" ? value : "system";
}

export function resolveTheme(preference: ThemePreference, systemIsDark: boolean): ResolvedTheme {
  if (preference === "system") return systemIsDark ? "dark" : "light";
  return preference;
}

/** The part of `MediaQueryList` the controller needs, so a test can stand in for it. */
export type SystemAppearance = Pick<
  MediaQueryList,
  "matches" | "addEventListener" | "removeEventListener"
>;

export interface ThemeController {
  /** Wears the theme `preference` asks for and, for `system`, follows the system. */
  set(preference: ThemePreference): void;
  readonly preference: ThemePreference;
  readonly resolved: ResolvedTheme;
}

/**
 * Applies the theme to `root`. Without a system appearance to ask (no `matchMedia`) the
 * system counts as dark, which is also what the stylesheet falls back to.
 */
export function createThemeController(
  root: HTMLElement,
  system: SystemAppearance | null,
): ThemeController {
  let preference: ThemePreference = "system";
  let stopFollowing: (() => void) | undefined;
  const resolved = (): ResolvedTheme => resolveTheme(preference, system?.matches ?? true);
  const apply = (): void => {
    root.dataset.theme = resolved();
  };
  return {
    set(next) {
      preference = next;
      stopFollowing?.();
      stopFollowing = undefined;
      if (next === "system" && system !== null) {
        system.addEventListener("change", apply);
        stopFollowing = () => {
          system.removeEventListener("change", apply);
        };
      }
      apply();
    },
    get preference() {
      return preference;
    },
    get resolved() {
      return resolved();
    },
  };
}

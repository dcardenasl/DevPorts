// Runs before React mounts (imported first by `main.tsx`): the panel window stays
// transparent until the page reports its first paint, so the theme the shell asked for in
// the URL is on the page before anyone can see it.
import { createThemeController, parseThemePreference } from "./theme";

export const appTheme = createThemeController(
  document.documentElement,
  typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null,
);

appTheme.set(parseThemePreference(new URLSearchParams(window.location.search).get("theme")));

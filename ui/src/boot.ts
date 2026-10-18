// Runs before React mounts (imported first by `main.tsx`): the panel window stays
// transparent until the page reports its first paint, so the theme the shell asked for in
// the URL is on the page before anyone can see it. The language it asked for is read here
// too and handed to the application.
import { parseLanguagePreference } from "./i18n/locale";
import { createThemeController, parseThemePreference } from "./theme";

export const appTheme = createThemeController(
  document.documentElement,
  typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null,
);

const params = new URLSearchParams(window.location.search);
appTheme.set(parseThemePreference(params.get("theme")));

export const bootLanguage = parseLanguagePreference(params.get("lang"));

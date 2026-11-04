import { useCallback, useEffect, useRef, useState } from "react";
import { type Backend } from "../api";
import type { ThemeController } from "../theme";
import type { LanguagePreference, Preferences } from "../types";

const SYSTEM_PREFERENCES: Preferences = { theme: "system", language: "system" };

/**
 * The saved theme and language, applied the moment one is chosen and saved behind it.
 * The shell already put them in the URL, so this only follows the saved value (they can
 * differ in a browser) and the person's changes. A failed save puts back what was there;
 * only the latest change's outcome counts, so a slow save never undoes a newer choice.
 */
export function usePreferences(
  api: Backend | undefined,
  theme: ThemeController,
  onLanguage: (language: LanguagePreference) => void,
): { preferences: Preferences; failed: boolean; change: (next: Preferences) => void } {
  const [preferences, setPreferences] = useState(SYSTEM_PREFERENCES);
  const [failed, setFailed] = useState(false);
  const latest = useRef(0);
  // The last value the core confirmed: what a failed save goes back to.
  const confirmed = useRef(SYSTEM_PREFERENCES);
  const onLanguageRef = useRef(onLanguage);
  useEffect(() => {
    onLanguageRef.current = onLanguage;
  });

  const apply = useCallback(
    (next: Preferences) => {
      setPreferences(next);
      theme.set(next.theme);
      onLanguageRef.current(next.language);
    },
    [theme],
  );

  useEffect(() => {
    if (api === undefined) return;
    let active = true;
    api.preferences().then(
      (saved) => {
        if (!active || latest.current !== 0) return;
        confirmed.current = saved;
        apply(saved);
      },
      () => undefined,
    );
    return () => {
      active = false;
    };
  }, [api, apply]);

  const change = useCallback(
    (next: Preferences) => {
      if (api === undefined) return;
      const mine = ++latest.current;
      setFailed(false);
      apply(next);
      api.setPreferences(next).then(
        (saved) => {
          confirmed.current = saved;
          if (mine === latest.current) apply(saved);
        },
        () => {
          if (mine !== latest.current) return;
          apply(confirmed.current);
          setFailed(true);
        },
      );
    },
    [api, apply],
  );

  return { preferences, failed, change };
}

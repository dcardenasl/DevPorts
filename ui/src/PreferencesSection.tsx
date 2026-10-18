import { type ReactElement } from "react";

import { useI18n } from "./i18n/provider";
import type { LanguagePreference, Preferences, ThemePreference } from "./types";

const THEMES: readonly ThemePreference[] = ["system", "light", "dark"];
const LANGUAGES: readonly LanguagePreference[] = ["system", "es", "en"];

/** One group of native radio buttons: the browser gives the arrow keys and the semantics. */
function Choice<T extends string>({
  legend,
  name,
  values,
  value,
  label,
  onChange,
}: {
  legend: string;
  name: string;
  values: readonly T[];
  value: T;
  label: (value: T) => string;
  onChange: (value: T) => void;
}): ReactElement {
  return (
    <fieldset className="choice">
      <legend className="label soft">{legend}</legend>
      <div className="choice-group">
        {values.map((option) => (
          <label key={option} className={option === value ? "segment on" : "segment"}>
            <input
              type="radio"
              name={name}
              value={option}
              checked={option === value}
              onChange={() => {
                onChange(option);
              }}
            />
            <span>{label(option)}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

/** The appearance and language of the panel, applied as soon as one is chosen (ADR-040, ADR-041). */
export function PreferencesSection({
  preferences,
  failed,
  onChange,
}: {
  preferences: Preferences;
  failed: boolean;
  onChange: (next: Preferences) => void;
}): ReactElement {
  const { text } = useI18n();
  const t = text.preferences;
  return (
    <section className="stack" aria-label={t.appearance}>
      <Choice
        legend={t.theme}
        name="theme"
        values={THEMES}
        value={preferences.theme}
        label={(value) => t.themes[value]}
        onChange={(theme) => {
          onChange({ ...preferences, theme });
        }}
      />
      <Choice
        legend={t.language}
        name="language"
        values={LANGUAGES}
        value={preferences.language}
        label={(value) => t.languages[value]}
        onChange={(language) => {
          onChange({ ...preferences, language });
        }}
      />
      {failed && (
        <p className="form-error" role="alert">
          {t.saveFailed}
        </p>
      )}
    </section>
  );
}

// The catalog the panel reads, for the language in force (ADR-041).
import {
  createContext,
  type ReactElement,
  type ReactNode,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import type { LanguagePreference } from "../types";
import { en } from "./en";
import { type Catalog, es } from "./es";
import { type Locale, resolveLocale } from "./locale";

const CATALOGS: Record<Locale, Catalog> = { es, en };

export interface I18n {
  readonly locale: Locale;
  /** Every string the person reads, in `locale`. */
  readonly text: Catalog;
}

/**
 * Without a provider a component speaks Spanish, the source catalog: a component can be
 * tested alone, and the application always mounts the provider (`App`).
 */
const I18nContext = createContext<I18n>({ locale: "es", text: es });

function systemTags(): readonly string[] {
  return typeof navigator === "undefined" ? [] : navigator.languages;
}

/** Follows `preference`; for `system` it follows the system's languages too, live. */
export function I18nProvider({
  preference,
  children,
}: {
  preference: LanguagePreference;
  children: ReactNode;
}): ReactElement {
  const [tags, setTags] = useState(systemTags);
  useEffect(() => {
    if (preference !== "system") return;
    const update = (): void => {
      setTags(systemTags());
    };
    window.addEventListener("languagechange", update);
    return () => {
      window.removeEventListener("languagechange", update);
    };
  }, [preference]);
  const locale = resolveLocale(preference, tags);
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);
  const value = useMemo<I18n>(() => ({ locale, text: CATALOGS[locale] }), [locale]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}

// Language-aware formatting, all through `Intl` so the rules are the platform's, not ours.
import type { Locale } from "./locale";

/**
 * Plural forms for one language. The catalogs build one per language and use it for
 * every counted phrase, so «1 proyecto» and «2 proyectos» are never an `if` in a template.
 */
export function pluralizer(locale: Locale) {
  const rules = new Intl.PluralRules(locale);
  const pick = (count: number, one: string, other: string): string =>
    rules.select(count) === "one" ? one : other;
  return {
    /** `one` for 1, `other` for everything else the language says is not 1. */
    pick,
    /** The count and its noun: `1 proyecto`, `3 proyectos`. */
    counted: (count: number, one: string, other: string): string =>
      `${count} ${pick(count, one, other)}`,
  };
}

/** `ahora`, `hace 41 min`, `hace 2 h`, `hace 3 d` (`now`, `41 min. ago`, …). */
export function relativeTime(minutesAgo: number, locale: Locale): string {
  const format = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "short" });
  if (minutesAgo < 1) return format.format(0, "second");
  if (minutesAgo < 60) return format.format(-minutesAgo, "minute");
  const hours = Math.floor(minutesAgo / 60);
  if (hours < 48) return format.format(-hours, "hour");
  return format.format(-Math.floor(hours / 24), "day");
}

/** The time of day in the person's own time zone: `09:12`, `9:12 AM`. */
export function timeOfDay(moment: Date, locale: Locale): string {
  return moment.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit" });
}

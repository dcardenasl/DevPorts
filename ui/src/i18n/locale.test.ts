import { describe, expect, it } from "vitest";

import { relativeTime, timeOfDay } from "./format";
import { parseLanguagePreference, resolveLocale } from "./locale";

// covers: UXR-9.3
describe("which language the panel speaks", () => {
  it.each([
    ["es", "es"],
    ["en", "en"],
    ["system", "system"],
    ["fr", "system"],
    ["", "system"],
    [null, "system"],
  ] as const)("reads %j from the URL as %s", (value, expected) => {
    expect(parseLanguagePreference(value)).toBe(expected);
  });

  // The Rust side tests this same table (crates/devports-app/src/i18n.rs).
  it.each([
    [["es-CL"], "es"],
    [["es_MX"], "es"],
    [["en-GB"], "en"],
    [["EN-us"], "en"],
    [["fr-FR", "es-ES"], "es"],
    [["fr-FR", "en-GB", "es"], "en"],
    [["fr-FR", "de"], "en"],
    [[], "en"],
  ] as const)("follows the system languages %j as %s", (tags, expected) => {
    expect(resolveLocale("system", tags)).toBe(expected);
  });

  it("ignores the system when a language is chosen", () => {
    expect(resolveLocale("es", ["en-US"])).toBe("es");
    expect(resolveLocale("en", ["es-CL"])).toBe("en");
  });
});

// covers: UXR-9.3
describe("how time reads", () => {
  it.each([
    [0, "es", "ahora"],
    [41, "es", "hace 41 min"],
    [120, "es", "hace 2 h"],
    [60 * 72, "es", "hace 3 d"],
    [0, "en", "now"],
    [41, "en", "41 min. ago"],
    [60 * 72, "en", "3 days ago"],
  ] as const)("%i minutes ago in %s is «%s»", (minutes, locale, expected) => {
    expect(relativeTime(minutes, locale)).toBe(expected);
  });

  it("writes the time of day the way the language does", () => {
    const moment = new Date(2026, 9, 18, 1, 22);
    expect(timeOfDay(moment, "es")).toBe("01:22");
    expect(timeOfDay(moment, "en")).toMatch(/^0?1:22\s?AM$/);
  });
});

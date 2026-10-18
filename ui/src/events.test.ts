import { describe, expect, it } from "vitest";

import { eventSentence } from "./events";
import { en } from "./i18n/en";
import { es } from "./i18n/es";
import type { LogEntry, LogFacts } from "./types";

const facts = (given: Partial<LogFacts> = {}): LogFacts => ({
  port: null,
  reason: null,
  launcher: null,
  existing_url: null,
  expires_at: null,
  ...given,
});

const entry = (kind: string, given: Partial<LogEntry> = {}): LogEntry => ({
  at: "2026-10-18T10:00:00-03:00",
  kind,
  project: "tienda-demo",
  service: "api",
  summary: "una frase del núcleo",
  topics: [],
  facts: facts({ port: 8080 }),
  ...given,
});

// covers: UXR-9.4
describe("the sentences of the activity log", () => {
  it("are written in the language in force from what the event carries", () => {
    const rejected = entry("run_rejected", {
      facts: facts({ launcher: "Cursor > zsh", existing_url: "http://localhost:8080" }),
    });
    expect(eventSentence(rejected, es, "es")).toBe(
      "un agente (Cursor > zsh) intentó levantar otra instancia de api; la Principal sigue en http://localhost:8080",
    );
    expect(eventSentence(rejected, en, "en")).toBe(
      "An agent (Cursor > zsh) tried to start another instance of api; the Primary is still at http://localhost:8080",
    );
  });

  it("tell a temporary run from a Principal one by the topic the core attached", () => {
    const base = { facts: facts({ port: 52814, reason: "QA login" }) };
    const temporal = entry("run_accepted", { ...base, topics: ["temporal"] });
    expect(eventSentence(temporal, es, "es")).toBe("Prueba de api :52814 para «QA login»");
    expect(eventSentence(temporal, en, "en")).toBe("Test of api :52814 for “QA login”");
    expect(eventSentence(entry("run_accepted"), es, "es")).toBe(
      "api registrada como Principal :8080",
    );
    expect(eventSentence(entry("run_accepted"), en, "en")).toBe("api registered as Primary :8080");
  });

  it("write what is absent as nothing, never as a gap", () => {
    expect(eventSentence(entry("stopped", { facts: facts() }), es, "es")).toBe("api se detuvo");
    expect(eventSentence(entry("stopped", { service: null, facts: facts() }), en, "en")).toBe(
      "a service was stopped",
    );
    expect(eventSentence(entry("port_stolen", { facts: facts() }), es, "es")).toBe(
      "Otro proceso tomó el puerto ? reservado para api",
    );
  });

  it("show the time a lease ends on the person's clock", () => {
    const extended = entry("lease_extended", {
      facts: facts({ port: 8080, expires_at: "2026-10-18T10:30:00-03:00" }),
    });
    expect(eventSentence(extended, es, "es")).toMatch(
      /^La prueba de api :8080 se extendió; vence \d{2}:\d{2}$/,
    );
    expect(eventSentence(extended, en, "en")).toMatch(
      /^The test of api :8080 was extended; it expires \d{1,2}:\d{2}\s?[AP]M$/,
    );
    expect(eventSentence(entry("lease_extended", { facts: facts() }), en, "en")).toBe(
      "The test of api was extended; it expires later",
    );
  });

  it("fall back to the core's own sentence for a kind this panel does not know", () => {
    expect(eventSentence(entry("something_new"), en, "en")).toBe("una frase del núcleo");
    // Names that exist on every object are not event kinds.
    expect(eventSentence(entry("constructor"), en, "en")).toBe("una frase del núcleo");
    expect(eventSentence(entry("toString"), en, "en")).toBe("una frase del núcleo");
  });

  it("still read an entry from an older core that sends no facts", () => {
    const older: LogEntry = {
      at: "2026-10-18T10:00:00-03:00",
      kind: "stopped",
      project: null,
      service: "api",
      summary: "api se detuvo",
    };
    expect(eventSentence(older, en, "en")).toBe("api was stopped");
  });
});

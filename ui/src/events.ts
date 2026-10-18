// The sentences of the activity log, written here from what the core says happened
// (`LogEntry.kind` and `facts`), so they come out in the language in force. The sentence the
// core also sends is Spanish and is only the fallback for a kind this panel does not know,
// which is what a newer core may send.
import type { Catalog, EventContext } from "./i18n/es";
import type { Locale } from "./i18n/locale";
import { timeOfDay } from "./i18n/format";
import type { LogEntry } from "./types";

export function eventSentence(entry: LogEntry, text: Catalog, locale: Locale): string {
  const writers: Record<string, ((context: EventContext) => string) | undefined> =
    text.activity.events;
  const write = Object.hasOwn(writers, entry.kind) ? writers[entry.kind] : undefined;
  if (write === undefined) return entry.summary;
  const facts = entry.facts;
  const expires = facts?.expires_at == null ? null : new Date(facts.expires_at);
  const port = facts?.port ?? null;
  const sentence = write({
    service: entry.service ?? text.activity.unnamedService,
    port: port === null ? "" : `:${port}`,
    portNumber: port,
    reason: facts?.reason ?? null,
    launcher: facts?.launcher ?? null,
    url: facts?.existing_url ?? null,
    expires:
      expires === null || Number.isNaN(expires.getTime()) ? null : timeOfDay(expires, locale),
    temporal: (entry.topics ?? []).includes("temporal"),
  });
  // An absent detail leaves a double space behind; a sentence never keeps one.
  return sentence.split(/\s+/).filter(Boolean).join(" ");
}

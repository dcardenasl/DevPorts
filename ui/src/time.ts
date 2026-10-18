import { relativeTime, timeOfDay } from "./i18n/format";
import type { Locale } from "./i18n/locale";

/** Seconds from `now` until `iso`, never negative. */
export function secondsUntil(iso: string | null, nowMs: number): number | null {
  if (iso === null) return null;
  const end = Date.parse(iso);
  if (Number.isNaN(end)) return null;
  return Math.max(0, Math.round((end - nowMs) / 1000));
}

/** `24:12` or `1:05:09`. */
export function countdown(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const two = (value: number) => String(value).padStart(2, "0");
  return hours > 0 ? `${hours}:${two(minutes)}:${two(seconds)}` : `${minutes}:${two(seconds)}`;
}

/** `hace 2 h`, `hace 41 min` (`2 hr. ago`, `41 min. ago`). */
export function ago(iso: string | null, nowMs: number, locale: Locale): string {
  if (iso === null) return "";
  const started = Date.parse(iso);
  if (Number.isNaN(started)) return "";
  return relativeTime(Math.max(0, Math.floor((nowMs - started) / 60_000)), locale);
}

/** `09:12` in the person's own time zone. */
export function clock(iso: string | null, locale: Locale): string {
  if (iso === null) return "";
  const moment = new Date(iso);
  if (Number.isNaN(moment.getTime())) return "";
  return timeOfDay(moment, locale);
}

/** `hace 2 h` for a moment given in milliseconds since the epoch. */
export function agoFrom(ms: number | null, nowMs: number, locale: Locale): string {
  return ms === null ? "" : ago(new Date(ms).toISOString(), nowMs, locale);
}

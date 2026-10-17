import { text } from "./text";

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

/** `hace 2 h`, `hace 41 min`. */
export function ago(iso: string | null, nowMs: number): string {
  if (iso === null) return "";
  const started = Date.parse(iso);
  if (Number.isNaN(started)) return "";
  const minutes = Math.max(0, Math.floor((nowMs - started) / 60_000));
  if (minutes < 1) return text.relative.now;
  if (minutes < 60) return text.relative.minutes(minutes);
  const hours = Math.floor(minutes / 60);
  if (hours < 48) return text.relative.hours(hours);
  return text.relative.days(Math.floor(hours / 24));
}

/** `09:12` in the person's own time zone. */
export function clock(iso: string | null): string {
  if (iso === null) return "";
  const moment = new Date(iso);
  if (Number.isNaN(moment.getTime())) return "";
  return moment.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit", hour12: false });
}

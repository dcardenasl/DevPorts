// The link between the panel and the core as a small state machine, independent of React.
//
// connecting ──ok──▶ ready ──poll or call fails──▶ reconnecting ──ok──▶ ready
//     │                                                 │
//     └──────────── attempts exhausted ──▶ failed ◀─────┘        failed ──retry()──▶ connecting
//
// Retries are bounded and spaced out; `retry()` is the manual way out of `failed` and
// also skips the wait while reconnecting. Nothing here resends a confirmed operation:
// it only reads state (snapshot, panel context) and follows the snapshot stream.

import type { Backend } from "./api";
import type { PanelState, Snapshot } from "./types";

/** Waits before each automatic attempt after a failure; its length bounds the attempts. */
export const RETRY_DELAYS_MS: readonly number[] = [1_000, 2_000, 4_000, 8_000, 15_000];
/** How often a healthy link asks for the snapshot again, to notice a core that went away. */
export const POLL_MS = 10_000;

export type Phase = "connecting" | "ready" | "reconnecting" | "failed";

/** What the panel shows about the link. */
export interface Link {
  phase: Phase;
  /** Automatic attempts made since the last success (0 while ready). */
  attempt: number;
  /** Automatic attempts allowed before `failed`. */
  attempts: number;
  /** Why the last attempt failed, as text. */
  error: string | null;
  /** When the snapshot on screen was received, to say how stale it is. */
  receivedAt: number | null;
  /** Counts successful (re)connections, so dependants can read their state again. */
  epoch: number;
}

export interface Timers {
  setTimeout(run: () => void, ms: number): number;
  clearTimeout(id: number): void;
  now(): number;
}

export const browserTimers: Timers = {
  setTimeout: (run, ms) => window.setTimeout(run, ms),
  clearTimeout: (id) => {
    window.clearTimeout(id);
  },
  now: () => Date.now(),
};

/** What the first successful connection brings: the context saved by the shell. */
export interface FirstConnection {
  api: Backend;
  snapshot: Snapshot;
  stored: PanelState;
}

export interface ConnectorOptions {
  connect: () => Promise<Backend>;
  timers: Timers;
  onLink: (link: Link) => void;
  /** Called once, on the first success. Later reconnections keep drafts and context. */
  onFirst: (first: FirstConnection) => void;
  /** Called for every snapshot that is newer than the one before it. */
  onSnapshot: (snapshot: Snapshot) => void;
}

const message = (failure: unknown): string =>
  failure instanceof Error ? failure.message : String(failure);

export class Connector {
  private run = 0;
  private stopped = false;
  private first = true;
  private link: Link = {
    phase: "connecting",
    attempt: 0,
    attempts: RETRY_DELAYS_MS.length,
    error: null,
    receivedAt: null,
    epoch: 0,
  };
  private timer: number | undefined;
  private unsubscribe: (() => void) | undefined;
  private api: Backend | undefined;
  private latest = -1;

  constructor(private readonly options: ConnectorOptions) {}

  start(): void {
    void this.attempt(0);
  }

  /** Manual retry: also skips the wait of an automatic one. */
  retry(): void {
    if (this.stopped || this.link.phase === "connecting" || this.link.phase === "ready") return;
    this.clearTimer();
    void this.attempt(0);
  }

  stop(): void {
    this.stopped = true;
    this.run += 1;
    this.clearTimer();
    this.drop();
  }

  private emit(change: Partial<Link>): void {
    this.link = { ...this.link, ...change };
    this.options.onLink(this.link);
  }

  private clearTimer(): void {
    if (this.timer !== undefined) this.options.timers.clearTimeout(this.timer);
    this.timer = undefined;
  }

  private drop(): void {
    this.unsubscribe?.();
    this.unsubscribe = undefined;
  }

  private current(run: number): boolean {
    return !this.stopped && run === this.run;
  }

  /** `done` automatic attempts have already failed. */
  private async attempt(done: number): Promise<void> {
    this.run += 1;
    const run = this.run;
    this.clearTimer();
    this.drop();
    const reconnecting = this.link.receivedAt !== null;
    this.emit({
      phase: reconnecting || done > 0 ? "reconnecting" : "connecting",
      attempt: done,
      ...(done === 0 && !reconnecting ? { error: null } : {}),
    });
    try {
      const api = await this.options.connect();
      const [snapshot, stored] = await Promise.all([
        api.snapshot(),
        this.first ? api.panelState() : Promise.resolve(undefined),
      ]);
      if (!this.current(run)) return;
      this.latest = snapshot.revision;
      // Subscribe once per connection, and only after the previous one was released.
      const stop = await api.subscribe((next) => {
        if (!this.current(run) || next.revision < this.latest) return;
        this.latest = next.revision;
        this.receive(next);
      });
      if (!this.current(run)) {
        stop();
        return;
      }
      this.unsubscribe = stop;
      this.api = api;
      if (this.first && stored !== undefined) {
        this.first = false;
        this.options.onFirst({ api, snapshot, stored });
      } else {
        this.options.onSnapshot(snapshot);
      }
      this.emit({
        phase: "ready",
        attempt: 0,
        error: null,
        receivedAt: this.options.timers.now(),
        epoch: this.link.epoch + 1,
      });
      this.schedulePoll(run);
    } catch (failure) {
      if (!this.current(run)) return;
      this.fail(done, message(failure), run);
    }
  }

  private receive(snapshot: Snapshot): void {
    this.emit({ receivedAt: this.options.timers.now() });
    this.options.onSnapshot(snapshot);
  }

  private fail(done: number, error: string, run: number): void {
    const delay = RETRY_DELAYS_MS[done];
    if (delay === undefined) {
      this.emit({ phase: "failed", attempt: done, error });
      return;
    }
    this.emit({
      phase: this.link.receivedAt === null && done === 0 ? "connecting" : "reconnecting",
      attempt: done + 1,
      error,
    });
    this.timer = this.options.timers.setTimeout(() => {
      if (this.current(run)) void this.attempt(done + 1);
    }, delay);
  }

  /** A healthy link asks again from time to time: that is how a lost core is noticed. */
  private schedulePoll(run: number): void {
    this.timer = this.options.timers.setTimeout(() => {
      void this.poll(run);
    }, POLL_MS);
  }

  private async poll(run: number): Promise<void> {
    if (!this.current(run) || this.api === undefined) return;
    try {
      const snapshot = await this.api.snapshot();
      if (!this.current(run)) return;
      if (snapshot.revision > this.latest) {
        this.latest = snapshot.revision;
        this.receive(snapshot);
      } else {
        this.emit({ receivedAt: this.options.timers.now() });
      }
      this.schedulePoll(run);
    } catch (failure) {
      if (!this.current(run)) return;
      this.drop();
      this.fail(0, message(failure), run);
    }
  }
}

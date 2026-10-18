import { describe, expect, it } from "vitest";

import type { Backend } from "./api";
import {
  Connector,
  POLL_MS,
  RETRY_DELAYS_MS,
  type FirstConnection,
  type Link,
  type Timers,
} from "./connection";
import { sampleSnapshot } from "./sample";
import type { PanelState, Snapshot } from "./types";

/** Deterministic timers: nothing runs until the test advances the clock. */
function fakeTimers(): Timers & { advance: (ms: number) => Promise<void>; pending: () => number } {
  let now = 1_000_000;
  let next = 1;
  const queue = new Map<number, { at: number; run: () => void }>();
  return {
    setTimeout: (run, ms) => {
      const id = next++;
      queue.set(id, { at: now + ms, run });
      return id;
    },
    clearTimeout: (id) => {
      queue.delete(id);
    },
    now: () => now,
    pending: () => queue.size,
    advance: async (ms) => {
      const end = now + ms;
      for (;;) {
        const due = [...queue.entries()]
          .filter(([, timer]) => timer.at <= end)
          .sort((a, b) => a[1].at - b[1].at)[0];
        if (due === undefined) break;
        queue.delete(due[0]);
        now = due[1].at;
        due[1].run();
        await flush();
      }
      now = end;
    },
  };
}

const flush = async (): Promise<void> => {
  for (let i = 0; i < 20; i++) await Promise.resolve();
};

const stored: PanelState = {
  version: 1,
  tab: "attention",
  expandedProject: null,
  expandedProjectSet: true,
  activityFilter: "all",
  draft: null,
};

interface Harness {
  connector: Connector;
  links: Link[];
  snapshots: Snapshot[];
  firsts: FirstConnection[];
  timers: ReturnType<typeof fakeTimers>;
  control: {
    failSnapshot: number;
    subscribed: number;
    unsubscribed: number;
    panelStateCalls: number;
    push: (snapshot: Snapshot) => void;
    revision: number;
  };
}

function harness(failures = 0): Harness {
  const timers = fakeTimers();
  const links: Link[] = [];
  const snapshots: Snapshot[] = [];
  const firsts: FirstConnection[] = [];
  let listener: ((snapshot: Snapshot) => void) | undefined;
  const control = {
    failSnapshot: failures,
    subscribed: 0,
    unsubscribed: 0,
    panelStateCalls: 0,
    revision: 5,
    push: (snapshot: Snapshot) => {
      listener?.(snapshot);
    },
  };
  const api = {
    snapshot: () => {
      if (control.failSnapshot > 0) {
        control.failSnapshot--;
        return Promise.reject(new Error("el núcleo no responde"));
      }
      return Promise.resolve({ ...sampleSnapshot(), revision: control.revision });
    },
    panelState: () => {
      control.panelStateCalls++;
      return Promise.resolve(stored);
    },
    subscribe: (next: (snapshot: Snapshot) => void) => {
      control.subscribed++;
      listener = next;
      return Promise.resolve(() => {
        control.unsubscribed++;
        if (listener === next) listener = undefined;
      });
    },
  } as unknown as Backend;
  const connector = new Connector({
    connect: () => Promise.resolve(api),
    timers,
    onLink: (link) => links.push(link),
    onFirst: (first) => firsts.push(first),
    onSnapshot: (snapshot) => snapshots.push(snapshot),
  });
  return { connector, links, snapshots, firsts, timers, control };
}

const last = (h: Harness): Link => h.links[h.links.length - 1] as Link;

// covers: UXR-6.2
describe("the connection to the core", () => {
  it("connects once, hydrates the shell context once and follows snapshots", async () => {
    const h = harness();
    h.connector.start();
    await flush();
    expect(last(h)).toMatchObject({ phase: "ready", attempt: 0, epoch: 1 });
    expect(h.firsts).toHaveLength(1);
    expect(h.firsts[0]?.stored).toBe(stored);
    expect(h.control.subscribed).toBe(1);
    h.control.push({ ...sampleSnapshot(), revision: 6 });
    expect(h.snapshots.map((s) => s.revision)).toEqual([6]);
    h.connector.stop();
  });

  it("drops an old snapshot instead of letting it overwrite a newer one", async () => {
    const h = harness();
    h.connector.start();
    await flush();
    h.control.push({ ...sampleSnapshot(), revision: 9 });
    h.control.push({ ...sampleSnapshot(), revision: 7 });
    h.control.push({ ...sampleSnapshot(), revision: 9 });
    expect(h.snapshots.map((s) => s.revision)).toEqual([9, 9]);
    h.connector.stop();
  });

  it("recovers from an initial rejection without restarting, hydrating exactly once", async () => {
    const h = harness(2);
    h.connector.start();
    await flush();
    expect(last(h)).toMatchObject({
      phase: "connecting",
      attempt: 1,
      error: "el núcleo no responde",
    });
    await h.timers.advance(RETRY_DELAYS_MS[0] ?? 0);
    expect(last(h)).toMatchObject({ phase: "reconnecting", attempt: 2 });
    await h.timers.advance(RETRY_DELAYS_MS[1] ?? 0);
    expect(last(h)).toMatchObject({ phase: "ready", attempt: 0, error: null });
    expect(h.firsts).toHaveLength(1);
    expect(h.control.panelStateCalls).toBe(3);
    h.connector.stop();
  });

  it("gives up after the bounded attempts and a manual retry starts a fresh round", async () => {
    const h = harness(100);
    h.connector.start();
    await flush();
    for (const delay of RETRY_DELAYS_MS) await h.timers.advance(delay);
    expect(last(h)).toMatchObject({ phase: "failed", error: "el núcleo no responde" });
    expect(h.timers.pending()).toBe(0);
    const attemptsMade = h.control.panelStateCalls;
    await h.timers.advance(60_000);
    expect(h.control.panelStateCalls).toBe(attemptsMade);

    h.control.failSnapshot = 0;
    h.connector.retry();
    await flush();
    expect(last(h)).toMatchObject({ phase: "ready", epoch: 1 });
    expect(h.firsts).toHaveLength(1);
    h.connector.stop();
  });

  it("notices a lost core by polling, keeps the old data and reconnects without a second subscription", async () => {
    const h = harness();
    h.connector.start();
    await flush();
    const received = last(h).receivedAt;
    h.control.failSnapshot = 2;
    await h.timers.advance(POLL_MS);
    expect(last(h)).toMatchObject({ phase: "reconnecting", attempt: 1, receivedAt: received });
    expect(h.control.unsubscribed).toBe(1);
    await h.timers.advance(RETRY_DELAYS_MS[0] ?? 0);
    expect(last(h)).toMatchObject({ phase: "reconnecting", attempt: 2 });
    await h.timers.advance(RETRY_DELAYS_MS[1] ?? 0);
    expect(last(h)).toMatchObject({ phase: "ready", epoch: 2 });
    expect(h.control.subscribed - h.control.unsubscribed).toBe(1);
    expect(h.firsts).toHaveLength(1);
    expect(h.control.panelStateCalls).toBe(1);
    h.connector.stop();
  });

  it("a healthy poll adopts a newer snapshot and ignores an older one", async () => {
    const h = harness();
    h.connector.start();
    await flush();
    h.control.revision = 12;
    await h.timers.advance(POLL_MS);
    expect(h.snapshots.map((s) => s.revision)).toEqual([12]);
    h.control.revision = 3;
    await h.timers.advance(POLL_MS);
    expect(h.snapshots).toHaveLength(1);
    expect(last(h).phase).toBe("ready");
    h.connector.stop();
  });

  it("retry skips the wait of an automatic attempt while reconnecting", async () => {
    const h = harness();
    h.connector.start();
    await flush();
    h.control.failSnapshot = 1;
    await h.timers.advance(POLL_MS);
    expect(last(h).phase).toBe("reconnecting");
    h.connector.retry();
    await flush();
    expect(last(h)).toMatchObject({ phase: "ready", epoch: 2 });
    expect(h.timers.pending()).toBe(1);
    h.connector.stop();
  });

  it("stop() releases the subscription and cancels every pending timer and late result", async () => {
    const h = harness();
    h.connector.start();
    await flush();
    h.connector.stop();
    expect(h.control.unsubscribed).toBe(1);
    expect(h.timers.pending()).toBe(0);
    const before = h.links.length;
    h.control.push({ ...sampleSnapshot(), revision: 50 });
    await h.timers.advance(POLL_MS * 3);
    expect(h.links).toHaveLength(before);
    expect(h.snapshots).toHaveLength(0);

    const pending = harness(1);
    pending.connector.start();
    await flush();
    pending.connector.stop();
    await pending.timers.advance(60_000);
    expect(pending.control.subscribed).toBe(0);
    expect(pending.timers.pending()).toBe(0);
  });
});

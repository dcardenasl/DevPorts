import { pruned } from "./projectFolds";
import { useCallback, useEffect, useRef, useState } from "react";

import { backend, type Backend } from "./api";
import { Connector, RETRY_DELAYS_MS, browserTimers, type Link } from "./connection";
import { draftTargetExists } from "./model";
import type { OperationRequest, OperationView, PanelState, Snapshot } from "./types";

/** Milliseconds since the epoch, refreshed every `everyMs`. */
export function useNow(everyMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => {
      setNow(Date.now());
    }, everyMs);
    return () => {
      window.clearInterval(id);
    };
  }, [everyMs]);
  return now;
}

export interface Connection {
  api: Backend | undefined;
  snapshot: Snapshot | undefined;
  /** The link to the core: connecting, ready, reconnecting or failed, with the way out. */
  link: Link;
  /** Tries again at once; only meaningful while reconnecting or failed. */
  retry: () => void;
  panelState: PanelState | undefined;
  missingDraft: boolean;
}

const STARTING: Link = {
  phase: "connecting",
  attempt: 0,
  attempts: RETRY_DELAYS_MS.length,
  error: null,
  receivedAt: null,
  epoch: 0,
};

/** Connects to the core, follows its snapshots, recovers from transient failures and reports the first paint. */
export function useConnection(): Connection {
  const [api, setApi] = useState<Backend>();
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [link, setLink] = useState<Link>(STARTING);
  const [panelState, setPanelState] = useState<PanelState>();
  const [missingDraft, setMissingDraft] = useState(false);
  const connector = useRef<Connector | undefined>(undefined);
  const reported = useRef(false);

  useEffect(() => {
    const next = new Connector({
      connect: backend,
      timers: browserTimers,
      onLink: setLink,
      onFirst: ({ api: connected, snapshot: first, stored }) => {
        const folds = pruned(
          { open: stored.openProjects, closed: stored.closedProjects },
          first.projects.map((project) => project.name),
        );
        const draftExists = stored.draft === null || draftTargetExists(stored.draft, first);
        const restored: PanelState = {
          ...stored,
          openProjects: [...folds.open],
          closedProjects: [...folds.closed],
          draft: draftExists ? stored.draft : null,
        };
        const requestedTab = new URLSearchParams(window.location.search).get("tab");
        const validTab = ["attention", "projects", "shared", "orphans", "activity"].find(
          (tab) => tab === requestedTab,
        );
        if (validTab !== undefined) restored.tab = validTab as PanelState["tab"];
        setMissingDraft(!draftExists);
        setPanelState(restored);
        if (
          restored.openProjects.length !== stored.openProjects.length ||
          restored.closedProjects.length !== stored.closedProjects.length ||
          !draftExists
        ) {
          void connected.savePanelState(restored);
        }
        setApi(connected);
        setSnapshot(first);
      },
      onSnapshot: setSnapshot,
    });
    connector.current = next;
    next.start();
    return () => {
      next.stop();
      connector.current = undefined;
    };
  }, []);

  const retry = useCallback(() => {
    connector.current?.retry();
  }, []);

  // Two animation frames guarantee the browser has painted before the shell
  // makes the window visible (WebKit does not paint a window nobody sees).
  useEffect(() => {
    if (api === undefined || reported.current) return;
    reported.current = true;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        void api.panelReady();
      });
    });
  }, [api, snapshot]);

  return { api, snapshot, link, retry, panelState, missingDraft };
}

/** Stable predicate that is true while the calling component is mounted. */
export function useMounted(): () => boolean {
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  return useCallback(() => mounted.current, []);
}

/** Runs one async job at a time; calls made while one is running are ignored. */
export function useSingleFlight(): (job: () => Promise<void>) => Promise<void> {
  const running = useRef(false);
  return useCallback(async (job) => {
    if (running.current) return;
    running.current = true;
    try {
      await job();
    } finally {
      running.current = false;
    }
  }, []);
}

/**
 * Panel state mirrored to the shell. `persist` always merges onto the latest
 * value, so concurrent changes never overwrite each other with a stale copy.
 */
export function usePanelStore(
  api: Backend | undefined,
  hydrated: PanelState | undefined,
): {
  panelState: PanelState | undefined;
  persist: (change: Partial<PanelState>) => Promise<void>;
} {
  const [panelState, setPanelState] = useState<PanelState | undefined>(hydrated);
  const [previousHydrated, setPreviousHydrated] = useState(hydrated);
  const latest = useRef(hydrated);
  if (hydrated !== previousHydrated) {
    setPreviousHydrated(hydrated);
    setPanelState(hydrated);
  }
  useEffect(() => {
    latest.current = hydrated;
  }, [hydrated]);
  const persist = useCallback(
    async (change: Partial<PanelState>): Promise<void> => {
      const current = latest.current;
      if (current === undefined || api === undefined) return;
      const next = { ...current, ...change };
      latest.current = next;
      setPanelState(next);
      await api.savePanelState(next);
    },
    [api],
  );
  return { panelState, persist };
}

/**
 * Follows one shell operation: its stored state first, then every update. `operation`
 * is `undefined` while the shell is being asked, `null` when it has no such operation
 * (it expired or was never sent) and the latest view otherwise. A stale revision never
 * replaces a newer one. `refresh` asks again and bumps `generation` so a sheet can
 * reload what it reads next to the operation.
 */
export function useOperationWatch(
  api: Backend,
  operationId: string | undefined,
): {
  operation: OperationView | null | undefined;
  record: (id: string, next: OperationView | null) => void;
  error: string | undefined;
  clearError: () => void;
  refresh: () => void;
  generation: number;
} {
  const [observed, setObserved] = useState<{ id: string; value: OperationView | null }>();
  const [error, setError] = useState<string>();
  const [generation, setGeneration] = useState(0);
  const record = useCallback((id: string, next: OperationView | null) => {
    setObserved((prior) => {
      if (
        prior?.id === id &&
        prior.value !== null &&
        (next === null || prior.value.revision > next.revision)
      )
        return prior;
      return { id, value: next };
    });
  }, []);
  useEffect(() => {
    if (operationId === undefined) return;
    const state = { active: true };
    const active = (): boolean => state.active;
    let unsubscribe: (() => void) | undefined;
    const merge = (next: OperationView): void => {
      if (state.active && next.id === operationId) record(operationId, next);
    };
    void (async () => {
      unsubscribe = await api.subscribeOperations(merge);
      if (!state.active) {
        unsubscribe();
        return;
      }
      const stored = await api.operation(operationId);
      if (!active()) return;
      if (stored === null) record(operationId, null);
      else merge(stored);
    })().catch((failure: unknown) => {
      if (state.active) setError(failure instanceof Error ? failure.message : String(failure));
    });
    return () => {
      state.active = false;
      unsubscribe?.();
    };
  }, [api, operationId, generation, record]);
  const clearError = useCallback(() => {
    setError(undefined);
  }, []);
  const refresh = useCallback(() => {
    setError(undefined);
    setGeneration((value) => value + 1);
  }, []);
  return {
    operation: observed?.id === operationId ? observed?.value : undefined,
    record,
    error,
    clearError,
    refresh,
    generation,
  };
}

/** A message that disappears by itself. */
export function useToast(): [string | undefined, (message: string) => void] {
  const [message, setMessage] = useState<string>();
  const timer = useRef<number | undefined>(undefined);
  const show = useCallback((next: string) => {
    setMessage(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      setMessage(undefined);
    }, 2200);
  }, []);
  useEffect(
    () => () => {
      window.clearTimeout(timer.current);
    },
    [],
  );
  return [message, show];
}

/** Tracks shell-owned operations so panel remounts recover their actual state. */
export function useOperations(
  api: Backend | undefined,
  epoch = 0,
): {
  operations: OperationView[];
  /** Starts the operation, or returns the equal one still waiting for a retry. */
  start: (request: Omit<OperationRequest, "id">) => Promise<OperationView | undefined>;
  retry: (id: string) => Promise<void>;
} {
  const [operations, setOperations] = useState<OperationView[]>([]);
  const busy = useRef(new Set<string>());
  const merge = useCallback((next: OperationView) => {
    setOperations((current) => {
      const prior = current.find((item) => item.id === next.id);
      if (prior && prior.revision > next.revision) return current;
      return [next, ...current.filter((item) => item.id !== next.id)].slice(0, 48);
    });
  }, []);
  useEffect(() => {
    if (!api) return;
    const life = { active: true };
    let unsubscribe: (() => void) | undefined;
    void (async () => {
      const stop = await api.subscribeOperations((item) => {
        if (life.active) merge(item);
      });
      if (!life.active) {
        stop();
        return;
      }
      unsubscribe = stop;
      const all = await api.operations();
      all.forEach(merge);
    })().catch(() => undefined);
    return () => {
      life.active = false;
      unsubscribe?.();
    };
  }, [api, merge, epoch]);
  const start = useCallback(
    async (request: Omit<OperationRequest, "id">) => {
      if (!api) return undefined;
      const key = JSON.stringify(request);
      if (busy.current.has(key)) return undefined;
      const existing = operations.find(
        (item) =>
          item.kind === request.kind &&
          JSON.stringify(
            item.targets.map(({ reference, adoptTarget, stopExpectation }) => ({
              reference,
              adoptTarget: adoptTarget ?? null,
              stopExpectation: stopExpectation ?? null,
            })),
          ) ===
            JSON.stringify(
              request.targets.map(({ reference, adoptTarget, stopExpectation }) => ({
                reference,
                adoptTarget: adoptTarget ?? null,
                stopExpectation: stopExpectation ?? null,
              })),
            ),
      );
      if (existing && existing.status !== "succeeded") return existing;
      busy.current.add(key);
      try {
        const operation = await api.startOperation({ ...request, id: crypto.randomUUID() });
        merge(operation);
        return operation;
      } finally {
        busy.current.delete(key);
      }
    },
    [api, merge, operations],
  );
  const retry = useCallback(
    async (id: string) => {
      if (!api) return;
      merge(await api.retryOperation(id));
    },
    [api, merge],
  );
  return { operations, start, retry };
}

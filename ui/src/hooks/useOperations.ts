import { useCallback, useEffect, useRef, useState } from "react";
import { type Backend } from "../api";
import type { OperationRequest, OperationView } from "../types";

/** How long a finished operation stays on screen; failures and pending work stay until settled. */
export const RECENT_SUCCESS_MS = 6000;

/** Tracks shell-owned operations so panel remounts recover their actual state. */
export function useOperations(
  api: Backend | undefined,
  epoch = 0,
): {
  operations: OperationView[];
  /** What the panel shows: pending and failed work, and a success only for a moment. */
  visible: OperationView[];
  /** Starts the operation, or returns the equal one still waiting for a retry. */
  start: (request: Omit<OperationRequest, "id">) => Promise<OperationView | undefined>;
  retry: (id: string) => Promise<void>;
} {
  const [operations, setOperations] = useState<OperationView[]>([]);
  const [retired, setRetired] = useState<ReadonlySet<string>>(new Set());
  const busy = useRef(new Set<string>());
  const timers = useRef(new Map<string, number>());
  const live = useRef(new Set<string>());
  const retire = useCallback((id: string) => {
    setRetired((current) => new Set(current).add(id));
  }, []);
  const settle = useCallback(
    (next: OperationView, fromHistory: boolean) => {
      const timer = timers.current.get(next.id);
      if (next.status !== "succeeded") {
        // Work that came back (a retry) is shown again until it succeeds anew.
        if (timer !== undefined) window.clearTimeout(timer);
        timers.current.delete(next.id);
        setRetired((current) => {
          if (!current.has(next.id)) return current;
          const rest = new Set(current);
          rest.delete(next.id);
          return rest;
        });
        return;
      }
      if (timer !== undefined) return;
      if (fromHistory && !live.current.has(next.id)) {
        retire(next.id);
        return;
      }
      timers.current.set(
        next.id,
        window.setTimeout(() => {
          timers.current.delete(next.id);
          retire(next.id);
        }, RECENT_SUCCESS_MS),
      );
    },
    [retire],
  );
  const merge = useCallback(
    (next: OperationView, fromHistory = false) => {
      if (!fromHistory) live.current.add(next.id);
      setOperations((current) => {
        const prior = current.find((item) => item.id === next.id);
        if (prior && prior.revision > next.revision) return current;
        return [next, ...current.filter((item) => item.id !== next.id)].slice(0, 48);
      });
      settle(next, fromHistory);
    },
    [settle],
  );
  useEffect(() => {
    const pending = timers.current;
    return () => {
      pending.forEach((timer) => {
        window.clearTimeout(timer);
      });
      pending.clear();
    };
  }, []);
  useEffect(() => {
    if (!api) return;
    const life = { active: true };
    const isActive = (): boolean => life.active;
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
      if (isActive())
        all.forEach((item) => {
          merge(item, true);
        });
    })().catch(() => undefined);
    return () => {
      life.active = false;
      unsubscribe?.();
    };
  }, [api, merge, epoch]);
  const visible = operations.filter(
    (item) => !(item.status === "succeeded" && retired.has(item.id)),
  );
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
  return { operations, visible, start, retry };
}

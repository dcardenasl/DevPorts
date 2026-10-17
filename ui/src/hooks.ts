import { useCallback, useEffect, useRef, useState } from "react";

import { backend, type Backend } from "./api";
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
  error: string | undefined;
  panelState: PanelState | undefined;
  missingDraft: boolean;
}

/** Connects to the core, follows its snapshots and reports the first paint. */
export function useConnection(): Connection {
  const [api, setApi] = useState<Backend>();
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [error, setError] = useState<string>();
  const [panelState, setPanelState] = useState<PanelState>();
  const [missingDraft, setMissingDraft] = useState(false);
  const reported = useRef(false);

  useEffect(() => {
    let unsubscribe: (() => void) | undefined;
    const life = { cancelled: false };
    const gone = (): boolean => life.cancelled;
    void (async () => {
      try {
        const connected = await backend();
        const [first, stored] = await Promise.all([connected.snapshot(), connected.panelState()]);
        if (gone()) return;
        const expandedProject = stored.expandedProjectSet
          ? first.projects.some((project) => project.name === stored.expandedProject)
            ? stored.expandedProject
            : null
          : (first.projects[0]?.name ?? null);
        const draftExists =
          stored.draft === null ||
          first.instances.some((view) => view.ref === stored.draft?.reference);
        const restored: PanelState = {
          ...stored,
          expandedProject,
          expandedProjectSet: true,
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
          restored.expandedProject !== stored.expandedProject ||
          !stored.expandedProjectSet ||
          !draftExists
        ) {
          void connected.savePanelState(restored);
        }
        setApi(connected);
        setSnapshot(first);
        const stop = await connected.subscribe((next) => {
          setSnapshot((current) =>
            current === undefined || next.revision >= current.revision ? next : current,
          );
        });
        if (gone()) stop();
        else unsubscribe = stop;
      } catch (failure) {
        if (!gone()) setError(failure instanceof Error ? failure.message : String(failure));
      }
    })();
    return () => {
      life.cancelled = true;
      unsubscribe?.();
    };
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

  return { api, snapshot, error, panelState, missingDraft };
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
export function useOperations(api: Backend | undefined): {
  operations: OperationView[];
  start: (request: Omit<OperationRequest, "id">) => Promise<void>;
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
  }, [api, merge]);
  const start = useCallback(
    async (request: Omit<OperationRequest, "id">) => {
      if (!api) return;
      const key = JSON.stringify(request);
      if (busy.current.has(key)) return;
      const existing = operations.find(
        (item) =>
          item.kind === request.kind &&
          JSON.stringify(
            item.targets.map(({ reference, adoptTarget }) => ({ reference, adoptTarget })),
          ) ===
            JSON.stringify(
              request.targets.map(({ reference, adoptTarget }) => ({
                reference,
                adoptTarget: adoptTarget ?? null,
              })),
            ),
      );
      if (existing && existing.status !== "succeeded") return;
      busy.current.add(key);
      try {
        const operation = await api.startOperation({ ...request, id: crypto.randomUUID() });
        merge(operation);
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

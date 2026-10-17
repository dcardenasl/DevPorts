import { useCallback, useEffect, useRef, useState } from "react";

import { backend, type Backend } from "./api";
import type { PanelState, Snapshot } from "./types";

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

import { pruned } from "../projectFolds";
import { useCallback, useEffect, useRef, useState } from "react";
import { backend, type Backend } from "../api";
import { Connector, RETRY_DELAYS_MS, browserTimers, type Link } from "../connection";
import { draftTargetExists } from "../model";
import type { PanelState, Snapshot } from "../types";

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
  const [earlyApi, setEarlyApi] = useState<Backend>();
  const [panelState, setPanelState] = useState<PanelState>();
  const [missingDraft, setMissingDraft] = useState(false);
  const connector = useRef<Connector | undefined>(undefined);
  const reported = useRef(false);

  useEffect(() => {
    const next = new Connector({
      connect: backend,
      timers: browserTimers,
      onLink: setLink,
      onBackend: setEarlyApi,
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
  // makes the window visible (WebKit does not paint a window nobody sees). It does not
  // wait for the data: the loading screen is what the first paint shows.
  useEffect(() => {
    if (earlyApi === undefined || reported.current) return;
    reported.current = true;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        void earlyApi.panelReady();
      });
    });
  }, [earlyApi]);

  return { api, snapshot, link, retry, panelState, missingDraft };
}

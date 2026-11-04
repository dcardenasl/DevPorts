import { useCallback, useEffect, useRef, useState } from "react";
import { type Backend } from "../api";
import type { PanelState } from "../types";

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

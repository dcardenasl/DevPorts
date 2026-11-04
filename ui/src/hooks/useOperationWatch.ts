import { useCallback, useEffect, useState } from "react";
import { type Backend } from "../api";
import type { OperationView } from "../types";

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
  error: { cause: unknown } | undefined;
  clearError: () => void;
  refresh: () => void;
  generation: number;
} {
  const [observed, setObserved] = useState<{ id: string; value: OperationView | null }>();
  // The cause, not its text: the message is written where it is shown, in the language then in force.
  const [error, setError] = useState<{ cause: unknown }>();
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
      if (state.active) setError({ cause: failure });
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

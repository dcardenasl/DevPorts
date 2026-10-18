import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Backend } from "./api";
import { RECENT_SUCCESS_MS, useOperations } from "./hooks";
import type { OperationStatus, OperationView } from "./types";

function operation(id: string, status: OperationStatus, revision = 1): OperationView {
  return { id, kind: "keep", status, revision, targets: [] };
}

/** The slice of the backend the hook reads, with a way to push a live update. */
function backendWith(history: OperationView[]): {
  api: Backend;
  push: (item: OperationView) => void;
  listeners: () => number;
} {
  const subscribers = new Set<(item: OperationView) => void>();
  const api = {
    operations: () => Promise.resolve(history),
    subscribeOperations: (listener: (item: OperationView) => void) => {
      subscribers.add(listener);
      return Promise.resolve(() => {
        subscribers.delete(listener);
      });
    },
  } as unknown as Backend;
  return {
    api,
    push: (item) => {
      subscribers.forEach((listener) => {
        listener(item);
      });
    },
    listeners: () => subscribers.size,
  };
}

const settleEffects = async (): Promise<void> => {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
};

describe("operations shown in the panel", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  // covers: UXR-11.2
  it("never shows a success that was already in the history when the panel opened", async () => {
    const { api } = backendWith([operation("old", "succeeded")]);
    const { result } = renderHook(() => useOperations(api));
    await settleEffects();
    expect(result.current.operations).toHaveLength(1);
    expect(result.current.visible).toHaveLength(0);
  });

  // covers: UXR-11.2
  it("keeps pending and failed work from the history until it is settled", async () => {
    const { api } = backendWith([
      operation("a", "running"),
      operation("b", "failed"),
      operation("c", "partial"),
      operation("d", "queued"),
    ]);
    const { result } = renderHook(() => useOperations(api));
    await settleEffects();
    expect(result.current.visible.map((item) => item.id).sort()).toEqual(["a", "b", "c", "d"]);
  });

  // covers: UXR-11.2
  it("shows a success that arrives live for a few seconds and then retires it", async () => {
    const { api, push } = backendWith([]);
    const { result } = renderHook(() => useOperations(api));
    await settleEffects();
    act(() => {
      push(operation("live", "succeeded"));
    });
    expect(result.current.visible).toHaveLength(1);
    act(() => {
      vi.advanceTimersByTime(RECENT_SUCCESS_MS - 1);
    });
    expect(result.current.visible).toHaveLength(1);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current.visible).toHaveLength(0);
    expect(result.current.operations).toHaveLength(1);
  });

  // covers: UXR-11.2
  it("shows a failure again after it was retired and stops its timer when it fails anew", async () => {
    const { api, push } = backendWith([]);
    const { result } = renderHook(() => useOperations(api));
    await settleEffects();
    act(() => {
      push(operation("x", "succeeded", 1));
    });
    act(() => {
      push(operation("x", "failed", 2));
    });
    act(() => {
      vi.advanceTimersByTime(RECENT_SUCCESS_MS * 2);
    });
    expect(result.current.visible.map((item) => item.status)).toEqual(["failed"]);
  });

  // covers: UXR-11.2
  it("leaves no timer behind when the panel closes", async () => {
    const { api, push, listeners } = backendWith([]);
    const { unmount } = renderHook(() => useOperations(api));
    await settleEffects();
    act(() => {
      push(operation("live", "succeeded"));
    });
    expect(vi.getTimerCount()).toBe(1);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
    expect(listeners()).toBe(0);
  });
});

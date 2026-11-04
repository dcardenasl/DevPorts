import { useCallback, useRef } from "react";

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

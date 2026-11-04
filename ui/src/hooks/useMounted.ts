import { useCallback, useEffect, useRef } from "react";

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

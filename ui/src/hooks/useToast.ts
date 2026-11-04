import { useCallback, useEffect, useRef, useState } from "react";

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

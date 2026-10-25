import { useEffect, useRef } from "react";

/** A window keydown listener while `active`; the latest handler is used. */
export function useWindowKeys(active: boolean, onKey: (e: KeyboardEvent) => void) {
  const ref = useRef(onKey);
  ref.current = onKey;
  useEffect(() => {
    if (!active) return;
    const listener = (e: KeyboardEvent) => ref.current(e);
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [active]);
}

import { type DependencyList, useEffect } from "react";

/** Runs `fn` once `deps` have stopped changing for `ms`. */
export function useDebouncedEffect(fn: () => void, deps: DependencyList, ms: number) {
  useEffect(() => {
    const t = setTimeout(fn, ms);
    return () => clearTimeout(t);
  }, deps);
}

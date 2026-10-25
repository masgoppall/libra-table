import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";

/** Listens to a Rust event for the component's lifetime; the latest handler is used. */
export function useTauriEvent<T>(name: string, handler: (payload: T) => void) {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => {
    const un = listen<T>(name, (e) => ref.current(e.payload));
    return () => {
      un.then((f) => f()).catch(() => undefined);
    };
  }, [name]);
}
